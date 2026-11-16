//! # morfo-live
//!
//! Live reload for an app under development: save a source file and the
//! running window picks the change up by itself, on the same screen, with
//! the same state.
//!
//! Rust is compiled, so "live" means: a background thread watches the source
//! files the binary was built from, runs the same `cargo build` that
//! produced it, and — when the build succeeds — the app saves its state in a
//! [`Snapshot`] and is replaced in place by the new binary, which reads the
//! state back. Because the whole program is swapped, *any* change takes
//! effect (your interface, the library, a renderer, a struct's fields), and
//! a build that fails changes nothing: the previous build keeps running and
//! [`Live::status`] carries the compiler's first error, to show it in the
//! window.
//!
//! It turns itself on only when the app was started with `cargo run`, so a
//! shipped binary is unaffected. `MORFO_LIVE=0` turns it off.
//!
//! ```no_run
//! use morfo_live::{Live, Snapshot, Status};
//!
//! // On start: the state the previous build left, if this is a reload.
//! let mut counter = 0u32;
//! if let Some(snap) = morfo_live::restored() {
//!     snap.load("counter", &mut counter);
//! }
//!
//! // `wake` runs on another thread whenever the status changes: use it to
//! // wake your event loop up.
//! let live = Live::start(|| { /* proxy.send_event(...) */ });
//!
//! // In the event loop, when woken:
//! if let Some(live) = &live {
//!     match live.status() {
//!         Status::Building => { /* show "rebuilding…" */ }
//!         Status::Failed(error) => { /* show the error */ }
//!         Status::Ready => {
//!             let mut snap = Snapshot::new();
//!             snap.set("counter", &counter);
//!             // Only returns if the new build could not be started.
//!             let error = live.restart(&snap);
//!             eprintln!("{error}");
//!         }
//!         Status::Watching => {}
//!     }
//! }
//! ```
//!
//! How long a reload takes is how long the incremental build takes: a few
//! seconds in the `dev` profile, much longer in a `release` profile with
//! LTO. [`Live::start_with_profile`] or `MORFO_LIVE_PROFILE=dev` make
//! reloads use another profile than the one the app was started with, and
//! `MORFO_LIVE_CARGO_ARGS` adds arguments to the build (`--features …`).
//!
//! Unix only for now (Linux, macOS): elsewhere [`Live::start`] returns `None`.
#![forbid(unsafe_code)]

mod snapshot;

pub use snapshot::{Snapshot, Value};

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, BufReader, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant, SystemTime};
use std::{env, fs, thread};

/// Names the file with the saved state for the build being started.
const STATE_ENV: &str = "MORFO_LIVE_STATE";
/// How often the source files are checked.
const POLL: Duration = Duration::from_millis(250);
/// Editors save in several steps: a change must stay put this long.
const SETTLE: Duration = Duration::from_millis(80);
/// A rebuild slower than this gets a hint about faster profiles.
const SLOW: Duration = Duration::from_secs(20);

/// Where the reload stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Up to date, waiting for a source file to change.
    Watching,
    /// A change was saved and the app is being rebuilt.
    Building,
    /// The build failed; the previous one keeps running. Holds the first
    /// compiler error and its location (the full output is in the terminal).
    Failed(String),
    /// The new build is ready: save the state and call [`Live::restart`].
    Ready,
}

/// The live-reload watcher of this process.
pub struct Live {
    plan: Arc<Plan>,
    status: Arc<Mutex<Status>>,
}

impl Live {
    /// Starts watching the sources of the running binary, if it was launched
    /// with `cargo run` (and `MORFO_LIVE` is not `0`); `None` otherwise.
    ///
    /// `wake` is called from the watcher's thread every time the
    /// [status](Self::status) changes.
    pub fn start(wake: impl Fn() + Send + 'static) -> Option<Live> {
        Live::begin(None, wake)
    }

    /// Like [`start`](Self::start), but reloads always build the cargo
    /// profile `profile`, whatever profile the app was started with: started
    /// with `--release`, it becomes a `"dev"` build at the first reload and
    /// every reload takes seconds. `MORFO_LIVE_PROFILE` still has the last
    /// word.
    pub fn start_with_profile(profile: &str, wake: impl Fn() + Send + 'static) -> Option<Live> {
        Live::begin(Some(profile), wake)
    }

    fn begin(profile: Option<&str>, wake: impl Fn() + Send + 'static) -> Option<Live> {
        let plan = Arc::new(Plan::from_env(profile)?);
        let status = Arc::new(Mutex::new(Status::Watching));
        let (watched, weak) = (plan.clone(), Arc::downgrade(&status));
        eprintln!("[morfo-live] watching {} source files; saving one runs `{}` and reloads", plan.files().len(), plan.command());
        if !plan.exe.exists() {
            // Another profile than the running one, never built: its
            // dependencies are compiled from scratch, once.
            eprintln!("[morfo-live] that profile has not been built yet: the first reload will take a while");
        }
        thread::Builder::new().name("morfo-live".into()).spawn(move || watch(&watched, &weak, &wake)).ok()?;
        Some(Live { plan, status })
    }

    pub fn status(&self) -> Status {
        self.status.lock().map_or(Status::Watching, |s| s.clone())
    }

    /// Replaces this process with the new build, which gets `state` from
    /// [`restored`]. Call it when the status is [`Status::Ready`], after
    /// saving anything else that must outlive the process: nothing runs
    /// after it, not even destructors. It only returns if the new build
    /// could not be started, with the reason.
    pub fn restart(&self, state: &Snapshot) -> io::Error {
        let path = env::temp_dir().join(format!("morfo-live-{}.state", std::process::id()));
        if let Err(e) = write_private(&path, &state.encode()) {
            return e;
        }
        let mut next = Command::new(&self.plan.exe);
        next.args(env::args_os().skip(1)).env(STATE_ENV, &path);
        exec(next)
    }
}

/// The state the previous build saved before reloading into this one;
/// `None` on a normal start. It can be read once.
pub fn restored() -> Option<Snapshot> {
    let path = env::var_os(STATE_ENV)?;
    let text = fs::read_to_string(&path).ok()?;
    let _ = fs::remove_file(&path);
    Snapshot::decode(&text)
}

#[cfg(unix)]
fn exec(mut next: Command) -> io::Error {
    use std::os::unix::process::CommandExt;
    next.exec()
}

#[cfg(not(unix))]
fn exec(_: Command) -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, "morfo-live can only relaunch on Unix")
}

/// The state may hold what was typed into the app: nobody else reads it.
fn write_private(path: &Path, text: &str) -> io::Result<()> {
    let _ = fs::remove_file(path);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(text.as_bytes())
}

/// How to rebuild the running binary, and what it is built from.
struct Plan {
    cargo: OsString,
    args: Vec<OsString>,
    /// The binary the build produces: the one to relaunch.
    exe: PathBuf,
    /// Cargo's list of the files `exe` was compiled from.
    dep_info: PathBuf,
    manifest_dir: PathBuf,
    /// Reloads build an optimized profile: worth a hint when they are slow.
    optimized: bool,
}

impl Plan {
    /// `None` unless this process was started by `cargo run`. Reloads build
    /// `MORFO_LIVE_PROFILE`, else `profile`, else the profile it runs in.
    fn from_env(profile: Option<&str>) -> Option<Plan> {
        if !cfg!(unix) || env::var_os("MORFO_LIVE").is_some_and(|v| v == "0") {
            return None;
        }
        // `cargo run` hands these two to the program it starts.
        let cargo = env::var_os("CARGO")?;
        let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?);
        let exe = env::current_exe().ok()?;
        let chosen = env::var("MORFO_LIVE_PROFILE").ok();
        let extra = env::var("MORFO_LIVE_CARGO_ARGS").unwrap_or_default();
        Plan::new(cargo, manifest_dir, &exe, chosen.as_deref().or(profile), &extra)
    }

    /// Reads the build back from where cargo put the binary:
    /// `<target>/<profile>/<bin>` or `<target>/<profile>/examples/<example>`.
    fn new(cargo: OsString, manifest_dir: PathBuf, exe: &Path, profile: Option<&str>, extra: &str) -> Option<Plan> {
        let name = exe.file_name()?;
        let dir = exe.parent()?;
        let example = dir.file_name()? == "examples";
        let profile_dir = if example { dir.parent()? } else { dir };
        let launched = match profile_dir.file_name()?.to_str()? {
            "debug" => "dev",
            other => other,
        };
        let profile = profile.unwrap_or(launched);
        let mut out = profile_dir.parent()?.join(if profile == "dev" { "debug" } else { profile });
        if example {
            out.push("examples");
        }
        let exe = out.join(name);
        let mut dep_info = exe.clone().into_os_string();
        dep_info.push(".d");

        let mut args: Vec<OsString> = vec!["build".into(), "--manifest-path".into(), manifest_dir.join("Cargo.toml").into()];
        args.extend([if example { "--example" } else { "--bin" }.into(), name.to_owned(), "--profile".into(), profile.into()]);
        args.extend(extra.split_whitespace().map(OsString::from));
        Some(Plan { cargo, args, exe, dep_info: dep_info.into(), manifest_dir, optimized: profile != "dev" })
    }

    /// The build command, for people.
    fn command(&self) -> String {
        let args: Vec<_> = self.args.iter().map(|a| a.to_string_lossy()).collect();
        format!("cargo {}", args.join(" "))
    }

    /// Every file whose change should trigger a rebuild: what cargo says the
    /// binary was built from, any other `.rs` next to those (a module about
    /// to be added), and the manifests.
    fn files(&self) -> Vec<PathBuf> {
        let deps = fs::read_to_string(&self.dep_info).map(|t| parse_dep_info(&t)).unwrap_or_default();
        let mut roots: Vec<PathBuf> =
            deps.iter().filter(|p| p.extension() == Some(OsStr::new("rs"))).filter_map(|p| p.parent()).map(PathBuf::from).collect();
        if roots.is_empty() {
            // Nothing from cargo to go by: the package itself.
            roots.push(self.manifest_dir.clone());
        }
        roots.sort();
        let mut top: Vec<PathBuf> = Vec::new();
        for root in roots {
            if !top.iter().any(|t| root.starts_with(t)) {
                top.push(root);
            }
        }

        let mut files = BTreeSet::new();
        let mut budget = 20_000;
        for root in &top {
            walk(root, 0, &mut budget, &mut files);
            // A crate's manifest sits next to its `src`.
            files.extend(root.ancestors().take(2).map(|d| d.join("Cargo.toml")).filter(|m| m.is_file()));
        }
        files.extend(deps);
        // The package's manifest and, above it, the workspace's.
        for dir in self.manifest_dir.ancestors() {
            files.extend(Some(dir.join("Cargo.toml")).filter(|m| m.is_file()));
            if dir.join("Cargo.lock").is_file() {
                break;
            }
        }
        files.into_iter().collect()
    }

    /// Runs the build, echoing cargo's output to the terminal. On failure,
    /// returns the first error.
    fn build(&self) -> Result<(), String> {
        let mut cargo = Command::new(&self.cargo);
        cargo.args(&self.args).stdin(Stdio::null()).stderr(Stdio::piped());
        // Through a pipe cargo would drop the colors the terminal can show.
        if io::stderr().is_terminal() && env::var_os("CARGO_TERM_COLOR").is_none() {
            cargo.env("CARGO_TERM_COLOR", "always");
        }
        let mut child = cargo.spawn().map_err(|e| format!("could not run cargo: {e}"))?;
        let mut output = String::new();
        if let Some(pipe) = child.stderr.take() {
            let mut pipe = BufReader::new(pipe);
            let mut line = Vec::new();
            while pipe.read_until(b'\n', &mut line).is_ok_and(|n| n > 0) {
                let _ = io::stderr().write_all(&line);
                output.push_str(&String::from_utf8_lossy(&line));
                line.clear();
            }
        }
        match child.wait() {
            Ok(exit) if exit.success() => Ok(()),
            Ok(_) => Err(first_error(&output)),
            Err(e) => Err(format!("could not run cargo: {e}")),
        }
    }
}

/// The watcher thread: runs until the [`Live`] it belongs to is dropped.
fn watch(plan: &Plan, status: &Weak<Mutex<Status>>, wake: &dyn Fn()) {
    let set = |new: Status| -> bool {
        let Some(status) = status.upgrade() else { return false };
        match status.lock() {
            Ok(mut s) => *s = new,
            Err(_) => return false,
        }
        wake();
        true
    };
    let mut seen = stamp(&plan.files());
    loop {
        thread::sleep(POLL);
        if status.strong_count() == 0 {
            return;
        }
        let files = plan.files();
        let mut now = stamp(&files);
        if now == seen {
            continue;
        }
        // Wait for the save to finish (write, rename, format on save...).
        loop {
            thread::sleep(SETTLE);
            let again = stamp(&files);
            if again == now {
                break;
            }
            now = again;
        }
        seen = now;

        eprintln!("[morfo-live] change saved, rebuilding…");
        if !set(Status::Building) {
            return;
        }
        let (built, started) = (modified(&plan.exe), Instant::now());
        let result = plan.build();
        if stamp(&files) != seen {
            // Saved again meanwhile: this build is already stale. Go round.
            continue;
        }
        // The build may have changed what there is to watch.
        seen = stamp(&plan.files());
        let took = started.elapsed();
        let next = match result {
            Err(error) => {
                eprintln!("[morfo-live] the build failed; still running the previous one");
                Status::Failed(error)
            }
            Ok(()) if modified(&plan.exe) == built => Status::Watching,
            Ok(()) => {
                eprintln!("[morfo-live] rebuilt in {:.1}s, reloading", took.as_secs_f32());
                if plan.optimized && took > SLOW {
                    eprintln!(
                        "[morfo-live] hint: an unoptimized build reloads much faster (`cargo run` without `--release`, or MORFO_LIVE_PROFILE=dev)"
                    );
                }
                Status::Ready
            }
        };
        if !set(next) {
            return;
        }
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Fingerprint of the state of `files` on disk: it changes when any of them
/// is written, created or removed.
fn stamp(files: &[PathBuf]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for path in files {
        path.hash(&mut h);
        match fs::metadata(path) {
            Ok(m) => (m.len(), m.modified().ok()).hash(&mut h),
            Err(_) => 0u8.hash(&mut h),
        }
    }
    h.finish()
}

/// Collects the `.rs` files under `dir`, skipping hidden entries and build
/// output. `budget` bounds the work if `dir` turns out to be huge.
fn walk(dir: &Path, depth: u32, budget: &mut u32, out: &mut BTreeSet<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        let (name, path) = (entry.file_name(), entry.path());
        if name.as_encoded_bytes().starts_with(b".") {
            continue;
        }
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            if depth < 12 && name != "target" {
                walk(&path, depth + 1, budget, out);
            }
        } else if path.extension() == Some(OsStr::new("rs")) {
            out.insert(path);
        }
    }
}

/// The prerequisites in a cargo `.d` file (`target: file file ...`, with
/// spaces and backslashes in paths escaped by a backslash).
fn parse_dep_info(text: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for line in text.lines() {
        let Some((_, deps)) = line.split_once(": ") else { continue };
        let mut file = String::new();
        let mut chars = deps.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => file.extend(chars.next()),
                ' ' => files.extend((!file.is_empty()).then(|| PathBuf::from(std::mem::take(&mut file)))),
                c => file.push(c),
            }
        }
        files.extend((!file.is_empty()).then(|| PathBuf::from(file)));
    }
    files
}

/// The first error in cargo's output and where it points to, without colors.
fn first_error(output: &str) -> String {
    let clean = strip_ansi(output);
    let lines: Vec<&str> = clean.lines().collect();
    let closing = |l: &str| l.starts_with("error: could not compile") || l.starts_with("error: aborting due to");
    match lines.iter().position(|l| l.starts_with("error") && !closing(l)) {
        Some(i) => {
            let mut error = lines[i].to_string();
            if let Some(at) = lines[i + 1..].iter().take(2).find_map(|l| l.trim_start().strip_prefix("--> ")) {
                error.push('\n');
                error.push_str(at.trim());
            }
            error
        }
        None => lines.iter().rev().find(|l| !l.trim().is_empty()).map_or("the build failed".to_string(), |l| l.trim().to_string()),
    }
}

/// Drops terminal escape sequences (colors, bold).
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
        } else if chars.next_if_eq(&'[').is_some() {
            // Parameters, then one final letter.
            while chars.next().is_some_and(|c| !('\u{40}'..='\u{7e}').contains(&c)) {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(exe: &str, profile: Option<&str>, extra: &str) -> Plan {
        Plan::new("cargo".into(), "/work/demo".into(), Path::new(exe), profile, extra).unwrap()
    }

    #[test]
    fn rebuilds_what_was_launched() {
        let p = plan("/work/target/release/docs", None, "");
        assert_eq!(p.command(), "cargo build --manifest-path /work/demo/Cargo.toml --bin docs --profile release");
        assert_eq!(p.exe, Path::new("/work/target/release/docs"));
        assert_eq!(p.dep_info, Path::new("/work/target/release/docs.d"));
        assert!(p.optimized);

        let p = plan("/work/target/debug/examples/my.app", None, "--features extra");
        assert_eq!(p.command(), "cargo build --manifest-path /work/demo/Cargo.toml --example my.app --profile dev --features extra");
        assert_eq!(p.dep_info, Path::new("/work/target/debug/examples/my.app.d"));
        assert!(!p.optimized);
    }

    #[test]
    fn reloads_can_use_another_profile() {
        let p = plan("/work/target/release/docs", Some("dev"), "");
        assert!(p.command().ends_with("--bin docs --profile dev"));
        assert_eq!(p.exe, Path::new("/work/target/debug/docs"));

        let p = plan("/work/target/debug/examples/demo", Some("fast"), "");
        assert_eq!(p.exe, Path::new("/work/target/fast/examples/demo"));
    }

    #[test]
    fn reads_cargo_dep_info() {
        let text = "/t/release/docs: /w/src/lib.rs /w/my\\ files/a.rs /w/shader.wgsl\n\n# env-dep:FOO\n";
        let files = parse_dep_info(text);
        assert_eq!(files, [Path::new("/w/src/lib.rs"), Path::new("/w/my files/a.rs"), Path::new("/w/shader.wgsl")]);
        assert!(parse_dep_info("").is_empty());
    }

    #[test]
    fn picks_the_first_error() {
        let output = "\u{1b}[1m\u{1b}[32m   Compiling\u{1b}[0m demo v0.1.0\n\
            warning: unused variable: `x`\n --> demo/src/a.rs:1:1\n\
            \u{1b}[1m\u{1b}[31merror[E0425]\u{1b}[0m: cannot find value `nope` in this scope\n\
            \u{1b}[34m  --> \u{1b}[0mdemo/src/pages.rs:12:9\n   |\n\
            error: second one\n  --> demo/src/pages.rs:20:1\n\
            error: could not compile `demo` (lib) due to 2 previous errors\n";
        assert_eq!(first_error(output), "error[E0425]: cannot find value `nope` in this scope\ndemo/src/pages.rs:12:9");
        assert_eq!(first_error("error: linking with `cc` failed\n  = note: ld: boom\n"), "error: linking with `cc` failed");
        assert_eq!(first_error("something odd\n\n"), "something odd");
        assert_eq!(first_error(""), "the build failed");
    }

    #[test]
    fn notices_changes_and_new_modules() {
        let dir = env::temp_dir().join(format!("morfo-live-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let src = dir.join("pkg/src");
        fs::create_dir_all(src.join("deep")).unwrap();
        fs::create_dir_all(dir.join("target/debug")).unwrap();
        fs::write(dir.join("Cargo.lock"), "").unwrap();
        fs::write(dir.join("Cargo.toml"), "[workspace]").unwrap();
        fs::write(dir.join("pkg/Cargo.toml"), "[package]").unwrap();
        fs::write(src.join("main.rs"), "fn main() {}").unwrap();
        fs::write(src.join("notes.txt"), "not code").unwrap();
        fs::write(src.join(".hidden.rs"), "").unwrap();
        fs::write(dir.join("target/debug/app.d"), format!("{}: {}\n", dir.join("target/debug/app").display(), src.join("main.rs").display()))
            .unwrap();

        let p = Plan::new("cargo".into(), dir.join("pkg"), &dir.join("target/debug/app"), None, "").unwrap();
        let files = p.files();
        assert_eq!(files, [dir.join("Cargo.toml"), dir.join("pkg/Cargo.toml"), src.join("main.rs")]);

        let before = stamp(&files);
        assert_eq!(before, stamp(&p.files()));
        fs::write(src.join("main.rs"), "fn main() { println!(); }").unwrap();
        assert_ne!(before, stamp(&p.files()), "an edit is noticed");

        let before = stamp(&p.files());
        fs::write(src.join("deep/new.rs"), "").unwrap();
        assert_ne!(before, stamp(&p.files()), "a module not yet built is noticed");
        fs::remove_dir_all(&dir).unwrap();
    }
}
