//! What the operating system knows and the interface should follow: its
//! fonts, whether animations are wanted, the local time zone, and where
//! applications keep their settings.

use std::path::PathBuf;
use std::process::Command;

use morfo::MotionPreference;

/// Where interface fonts usually are.
pub(crate) const SANS: &[&str] = &[
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "/Library/Fonts/Arial.ttf",
    "C:\\Windows\\Fonts\\segoeui.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
];

/// Where monospaced fonts usually are.
pub(crate) const MONO: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoMono-Regular.ttf",
    "/System/Library/Fonts/Menlo.ttc",
    "C:\\Windows\\Fonts\\consola.ttf",
];

/// Fonts tried, after the interface's own faces, for characters they lack.
/// Scripts that need shaping (Arabic, Indic) are left out: their letters
/// would come out unjoined.
const FALLBACKS: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansHebrew-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansGreek-Regular.ttf",
    "/System/Library/Fonts/Apple Symbols.ttf",
    "C:\\Windows\\Fonts\\seguisym.ttf",
];

/// Where the bold, italic and bold italic of a font at `path` usually are,
/// by the common naming schemes (`-Regular` → `-Bold`, `DejaVuSans` →
/// `DejaVuSans-Oblique`, `arial` → `arialbd`).
fn siblings(path: &str) -> [Vec<String>; 3] {
    let (stem, ext) = path.rsplit_once('.').unwrap_or((path, "ttf"));
    let base = stem.strip_suffix("-Regular").unwrap_or(stem);
    let named = |names: &[&str]| -> Vec<String> {
        let mut v: Vec<String> = names.iter().map(|n| format!("{base}-{n}.{ext}")).collect();
        // Windows: arial → arialbd, ariali, arialbi; segoeui → segoeuib...
        let short = match names[0] {
            "Bold" => ["bd", "b"],
            "Italic" => ["i", "i"],
            _ => ["bi", "z"],
        };
        v.extend(short.iter().map(|s| format!("{stem}{s}.{ext}")));
        v
    };
    [named(&["Bold"]), named(&["Italic", "Oblique"]), named(&["BoldItalic", "BoldOblique"])]
}

/// The system's fonts, as [`Fonts::system`](crate::Fonts::system) says.
pub(crate) fn fonts() -> Option<crate::Fonts> {
    let env = std::env::var("MORFO_FONT").ok();
    let path = env.iter().map(String::as_str).chain(SANS.iter().copied()).find(|p| std::path::Path::new(p).is_file())?;
    let main = std::fs::read(path).ok()?;
    let [bold, italic, bold_italic] = siblings(path).map(|names| names.iter().find_map(|p| std::fs::read(p).ok()));
    let mut fallbacks: Vec<Vec<u8>> = std::env::var_os("MORFO_FALLBACK_FONTS")
        .map(|v| std::env::split_paths(&v).filter_map(|p| std::fs::read(p).ok()).collect())
        .unwrap_or_default();
    // The main font itself is no fallback.
    fallbacks.extend(FALLBACKS.iter().filter(|p| **p != path).filter_map(|p| std::fs::read(p).ok()));
    Some(crate::Fonts { main, mono: font("MORFO_MONO_FONT", MONO), bold, italic, bold_italic, fallbacks })
}

/// The font named by the environment variable `env`, or the first of
/// `candidates` that exists.
pub(crate) fn font(env: &str, candidates: &[&str]) -> Option<Vec<u8>> {
    std::env::var(env).ok().into_iter().chain(candidates.iter().map(|s| s.to_string())).find_map(|p| std::fs::read(p).ok())
}

/// Runs a small command and returns what it printed, trimmed.
fn ask(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// How much the interface may move, as the system is set: `Reduced` where
/// the person asked for less motion ("Reduce motion" on macOS, animations
/// off on Windows or GNOME), `Full` otherwise or when it cannot be known.
/// `MORFO_MOTION=full|reduced|off` overrides it.
pub fn motion_preference() -> MotionPreference {
    if let Ok(v) = std::env::var("MORFO_MOTION") {
        match v.to_ascii_lowercase().as_str() {
            "off" | "none" => return MotionPreference::Off,
            "reduced" | "reduce" => return MotionPreference::Reduced,
            "full" => return MotionPreference::Full,
            _ => {}
        }
    }
    let reduced = if cfg!(target_os = "macos") {
        ask("defaults", &["read", "com.apple.universalaccess", "reduceMotion"]).is_some_and(|v| v == "1")
    } else if cfg!(windows) {
        // "Show animations in Windows" off sets MinAnimate to 0.
        ask("reg", &["query", r"HKCU\Control Panel\Desktop\WindowMetrics", "/v", "MinAnimate"]).is_some_and(|v| v.trim_end().ends_with('0'))
    } else {
        ask("gsettings", &["get", "org.gnome.desktop.interface", "enable-animations"]).is_some_and(|v| v == "false")
    };
    if reduced {
        MotionPreference::Reduced
    } else {
        MotionPreference::Full
    }
}

/// Whether the computer runs on its battery now (no charger plugged in).
/// `false` when it cannot be known, and on desktops. `MORFO_BATTERY=1|0`
/// overrides it.
pub fn on_battery() -> bool {
    if let Ok(v) = std::env::var("MORFO_BATTERY") {
        return v == "1" || v.eq_ignore_ascii_case("true");
    }
    if cfg!(target_os = "linux") {
        // Any mains supply online means plugged in; a battery discharging
        // with none means on battery.
        let Ok(dir) = std::fs::read_dir("/sys/class/power_supply") else { return false };
        let (mut mains, mut discharging) = (false, false);
        for e in dir.flatten() {
            let p = e.path();
            let read = |f: &str| std::fs::read_to_string(p.join(f)).map(|s| s.trim().to_string()).unwrap_or_default();
            match read("type").as_str() {
                "Mains" | "USB" => mains |= read("online") == "1",
                "Battery" => discharging |= read("status") == "Discharging",
                _ => {}
            }
        }
        discharging && !mains
    } else if cfg!(target_os = "macos") {
        ask("pmset", &["-g", "batt"]).is_some_and(|s| s.contains("Battery Power"))
    } else {
        false
    }
}

/// The local time zone's offset from UTC, in seconds (for
/// [`Profile::time_of_day`](morfo::Profile::time_of_day) and dates).
pub fn utc_offset() -> f64 {
    if cfg!(windows) {
        let seconds = "[TimeZoneInfo]::Local.GetUtcOffset([DateTime]::Now).TotalSeconds";
        return ask("powershell", &["-NoProfile", "-Command", seconds]).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    }
    let Some(text) = ask("date", &["+%z"]) else { return 0.0 };
    let (sign, digits) = match text.split_at_checked(1) {
        Some(("-", d)) => (-1.0, d),
        Some((_, d)) => (1.0, d),
        None => return 0.0,
    };
    let n: f64 = digits.parse().unwrap_or(0.0);
    sign * ((n / 100.0).floor() * 3600.0 + (n % 100.0) * 60.0)
}

/// The folder where `app` keeps its settings: `$XDG_CONFIG_HOME/app`,
/// `~/.config/app`, `~/Library/Application Support/app` or `%APPDATA%\app`.
pub fn config_dir(app: &str) -> PathBuf {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let base = if cfg!(windows) {
        env("APPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library").join("Application Support"))
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))
    };
    base.unwrap_or_else(std::env::temp_dir).join(app)
}
