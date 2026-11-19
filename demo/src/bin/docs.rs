//! The interactive documentation in a window.
//!
//! It draws with the GPU (wgpu) when there is one, and falls back to the
//! CPU renderer (softbuffer) otherwise; `--cpu` forces the CPU. The event
//! loop sleeps (`ControlFlow::Wait`) and only draws when there is input or
//! the UI reports it is still animating: at rest it uses no CPU.
//! The usage profile is saved on exit, so the interface you shaped comes back.
//!
//! It is built on the pieces of `morfo-winit` (painting, input, screen
//! readers, the title bar) rather than its `run`, because it adds what only
//! it needs: the live reload, and X11's focus manners.
//!
//! Started with `cargo run`, it reloads itself when a source file is saved
//! (morfo-live): it rebuilds in the background and relaunches in place, on
//! the same page, scroll and window. Reloads are `dev` builds (seconds)
//! whatever profile it was started with; `MORFO_LIVE_PROFILE=release` keeps
//! them optimized.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use morfo::{Profile, Theme, Ui, Vec2};
use morfo_demo::{build, theme_of, State, WINDOW};
use morfo_live::{Live, Snapshot, Status};
use morfo_winit::winit::application::ApplicationHandler;
use morfo_winit::winit::dpi::{LogicalSize, PhysicalPosition};
use morfo_winit::winit::event::{StartCause, WindowEvent};
use morfo_winit::winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use morfo_winit::winit::window::{Window, WindowId};
use morfo_winit::{access, now, system, Backend, Chrome, Fonts, Input, Painter};

/// What wakes the event loop up besides the window's own events.
enum Wake {
    /// A request from the screen reader.
    Access(accesskit_winit::Event),
    /// The live reload has news: building, failed, or ready to relaunch.
    Live,
}

impl From<accesskit_winit::Event> for Wake {
    fn from(event: accesskit_winit::Event) -> Wake {
        Wake::Access(event)
    }
}

struct Gfx {
    window: Arc<Window>,
    painter: Painter,
    access: accesskit_winit::Adapter,
}

struct App {
    ui: Ui,
    state: State,
    backend: Backend,
    fonts: Fonts,
    gfx: Option<Gfx>,
    proxy: EventLoopProxy<Wake>,
    /// Rebuilds and relaunches on save, when started with `cargo run`.
    live: Option<Live>,
    /// What the build this one replaced left: where its window was.
    restored: Option<Snapshot>,
    /// Bring the restored interface to rest before its first visible frame.
    settle: bool,
    /// Reloaded in the background: once shown, ask for no attention.
    calm: bool,
    input: Input,
    chrome: Chrome,
    /// When the UI asked to be woken with no input (tooltips, toasts, caret).
    wake: Option<Instant>,
    /// The title bar's close button was pressed.
    quit: bool,
}

/// X11 hands the keyboard to a window when it appears, and winit cannot ask
/// it not to. These two requests to the window manager let a window come
/// back from a live reload without disturbing whoever is typing elsewhere.
#[cfg(all(unix, not(target_os = "macos")))]
mod x11 {
    use morfo_winit::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use morfo_winit::winit::window::Window;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, PropMode};
    use x11rb::rust_connection::RustConnection;
    use x11rb::wrapper::ConnectionExt as _;

    type Done = Result<(), Box<dyn std::error::Error>>;

    /// Runs `f` with a connection to the X server, the id of `window` and
    /// the root window. Does nothing on Wayland, where a window never gets
    /// the focus by surprise.
    fn with(window: &Window, f: impl FnOnce(&RustConnection, u32, u32) -> Done) {
        let id = match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Xlib(h)) => h.window as u32,
            Ok(RawWindowHandle::Xcb(h)) => h.window.get(),
            _ => return,
        };
        let _ = x11rb::connect(None).map_err(Into::into).and_then(|(x, screen)| {
            let root = x.setup().roots[screen].root;
            f(&x, id, root)?;
            // Done before winit goes on through its own connection.
            x.sync()?;
            Ok::<(), Box<dyn std::error::Error>>(())
        });
    }

    /// Before the window is shown: a user time of zero says that nobody
    /// asked for it just now, so it does not take the keyboard.
    pub fn appear_unfocused(window: &Window) {
        with(window, |x, id, _| {
            let user_time = x.intern_atom(false, b"_NET_WM_USER_TIME")?.reply()?.atom;
            x.change_property32(PropMode::REPLACE, id, user_time, AtomEnum::CARDINAL, &[0])?;
            Ok(())
        });
    }

    /// Once shown: a window that was refused the focus is marked as wanting
    /// attention (it blinks in the task bar). This one wants none, and from
    /// here on it is a window like any other.
    pub fn calm(window: &Window) {
        with(window, |x, id, root| {
            let user_time = x.intern_atom(false, b"_NET_WM_USER_TIME")?.reply()?.atom;
            x.delete_property(id, user_time)?;
            let state = x.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
            let attention = x.intern_atom(false, b"_NET_WM_STATE_DEMANDS_ATTENTION")?.reply()?.atom;
            // 0 = remove the state; 1 = the request comes from an application.
            let event = ClientMessageEvent::new(32, id, state, [0, attention, 0, 1, 0]);
            x.send_event(false, root, EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY, event)?;
            Ok(())
        });
    }
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
mod x11 {
    pub fn appear_unfocused(_: &morfo_winit::winit::window::Window) {}
    pub fn calm(_: &morfo_winit::winit::window::Window) {}
}

fn profile_path() -> PathBuf {
    system::config_dir("morfo-demo").join("profile.txt")
}

impl App {
    fn redraw(&mut self) {
        let Some(gfx) = self.gfx.as_mut() else { return };
        let size = gfx.window.inner_size();
        let (Some(w), Some(h)) = (std::num::NonZeroU32::new(size.width), std::num::NonZeroU32::new(size.height)) else { return };
        let scale = gfx.window.scale_factor() as f32;

        self.state.maximized = gfx.window.is_maximized();
        let viewport = Vec2::new(size.width as f32 / scale, size.height as f32 / scale);
        if std::mem::take(&mut self.settle) {
            // Just reloaded: a few unpainted frames with no animation, so the
            // first one seen is already laid out, scrolled and still.
            self.ui.set_instant(true);
            for _ in 0..4 {
                self.ui.begin(now(), viewport);
                build(&mut self.ui, &mut self.state);
                self.ui.end();
            }
            self.ui.set_instant(false);
        }
        self.ui.begin(now(), viewport);
        build(&mut self.ui, &mut self.state);
        match self.live.as_ref().map(Live::status) {
            Some(Status::Building) => self.ui.notice("live", "Recompilando…"),
            Some(Status::Failed(error)) => self.ui.notice("live", &format!("No compila: sigue en marcha la versión anterior.\n{error}")),
            _ => {}
        }
        let frame = self.ui.end();

        // What the app's own title bar asked of the window.
        if self.chrome.apply(&mut self.ui, &gfx.window, self.state.chrome) {
            self.quit = true;
        }
        self.input.after_frame(&mut self.ui, &gfx.window);
        let (ui, s) = (&self.ui, gfx.window.scale_factor());
        gfx.access.update_if_active(|| access::tree(ui.access_nodes(), ui.focused(), s, "morfo · documentación"));

        gfx.painter.paint(&self.ui, w, h, scale);
        if std::mem::take(&mut self.calm) {
            x11::calm(&gfx.window);
        }

        self.wake = None;
        if frame.animating {
            gfx.window.request_redraw();
        } else {
            self.wake = frame.wake_in.map(|s| Instant::now() + Duration::from_secs_f32(s));
        }
    }

    fn save_profile(&self) {
        let path = profile_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&path, self.ui.profile().encode()) {
            eprintln!("could not save profile to {}: {e}", path.display());
        }
    }

    /// The new build is ready: hand it this one's state and become it.
    fn reload(&mut self, el: &ActiveEventLoop) {
        let Some(live) = self.live.as_ref() else { return };
        self.save_profile();
        let mut snap = Snapshot::new();
        self.state.save_live(&mut snap);
        snap.set("ui", &self.ui.save_state());
        if let Some(gfx) = self.gfx.as_ref() {
            let window = &gfx.window;
            snap.set("window.maximized", &window.is_maximized());
            snap.set("window.focused", &window.has_focus());
            if !window.is_maximized() {
                let size = window.inner_size().to_logical::<f32>(window.scale_factor());
                snap.set("window.size", &(size.width, size.height));
                // Not every platform says (Wayland does not).
                if let Ok(at) = window.outer_position() {
                    snap.set("window.position", &(at.x, at.y));
                }
            }
        }
        // This window closes before the new build opens its own.
        self.gfx = None;
        let error = live.restart(&snap);
        eprintln!("could not relaunch: {error}");
        el.exit();
    }
}

impl ApplicationHandler<Wake> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        // No window-manager decorations: the app draws its own title bar.
        // Created hidden: AccessKit must be attached before it is shown.
        let mut attrs = Window::default_attributes()
            .with_title("morfo · documentación")
            .with_inner_size(LogicalSize::new(WINDOW.x, WINDOW.y))
            .with_decorations(false)
            .with_visible(false);
        // After a live reload the window comes back where it was, and only
        // takes the keyboard if it had it: you may be typing in your editor.
        let mut unfocused = false;
        if let Some(snap) = self.restored.take() {
            unfocused = snap.get("window.focused") == Some(false);
            attrs = attrs.with_active(!unfocused);
            if let Some((w, h)) = snap.get::<(f32, f32)>("window.size") {
                attrs = attrs.with_inner_size(LogicalSize::new(w, h));
            }
            if let Some((x, y)) = snap.get::<(i32, i32)>("window.position") {
                attrs = attrs.with_position(PhysicalPosition::new(x, y));
            }
            attrs = attrs.with_maximized(snap.get("window.maximized") == Some(true));
        }
        let window = Arc::new(el.create_window(attrs).unwrap());
        let access = accesskit_winit::Adapter::with_event_loop_proxy(el, &window, self.proxy.clone());
        window.set_ime_allowed(true);
        if unfocused {
            x11::appear_unfocused(&window);
        }
        window.set_visible(true);
        self.calm = unfocused;
        let painter = Painter::new(&window, self.backend, &self.fonts).expect("no way to paint the window");
        eprintln!("drawing with the {}", painter.name());
        self.ui.set_text_measure(painter.measure());
        self.gfx = Some(Gfx { window, painter, access });
    }

    /// Requests from the screen reader, and news from the live reload.
    fn user_event(&mut self, el: &ActiveEventLoop, event: Wake) {
        match event {
            Wake::Live => {
                if self.live.as_ref().is_some_and(|live| live.status() == Status::Ready) {
                    return self.reload(el);
                }
            }
            Wake::Access(event) => access::handle(&mut self.ui, event.window_event),
        }
        if let Some(gfx) = self.gfx.as_ref() {
            gfx.window.request_redraw();
        }
    }

    fn new_events(&mut self, _: &ActiveEventLoop, cause: StartCause) {
        if let (StartCause::ResumeTimeReached { .. }, Some(gfx)) = (cause, self.gfx.as_ref()) {
            gfx.window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        el.set_control_flow(self.wake.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(gfx) = self.gfx.as_mut() else { return };
        gfx.access.process_event(&gfx.window, &event);
        let window = gfx.window.clone();
        match event {
            WindowEvent::CloseRequested => {
                self.save_profile();
                el.exit();
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
                if self.quit {
                    self.save_profile();
                    el.exit();
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } | WindowEvent::Occluded(false) => {
                gfx.painter.invalidate();
                window.request_redraw();
            }
            event => {
                let handled = self.input.handle(&mut self.ui, &window, &event);
                // Ctrl+K unfolds the search.
                if handled.chord.is_some_and(|c| c.0 == "k") {
                    self.state.finder = !self.state.finder;
                }
                if handled.redraw {
                    window.request_redraw();
                }
            }
        }
    }
}

fn main() {
    let backend = if std::env::args().any(|a| a == "--cpu") { Backend::Cpu } else { Backend::Auto };
    // After a live reload: what the build this one replaced was showing.
    let restored = morfo_live::restored();
    let mut state = restored.as_ref().map_or_else(State::default, State::restored);
    if restored.is_none() {
        // Today, in local time, for the date picker.
        state.date = morfo::Date::from_unix(now() + system::utc_offset());
    }
    let mut ui = Ui::new(if restored.is_some() { theme_of(&state) } else { Theme::dark() });
    if let Some(saved) = restored.as_ref().and_then(|snap| snap.get::<String>("ui")) {
        ui.restore_state(&saved);
    }
    ui.set_motion(system::motion_preference());
    match std::fs::read_to_string(profile_path()).ok().and_then(|t| Profile::decode(&t)) {
        Some(profile) => ui.set_profile(profile),
        // First run: a short memory, so the adaptation is visible in seconds.
        None => ui.profile_mut().half_life = 300.0,
    }
    // Habits by time of day, in local time.
    ui.profile_mut().time_of_day = Some(system::utc_offset());
    // The system's faces (bold, italic, fallbacks), else the demo's own font.
    let fonts = Fonts::system().unwrap_or_else(|| Fonts { mono: morfo_demo::load_mono_font(), ..Fonts::new(morfo_demo::load_font()) });

    let event_loop = EventLoop::<Wake>::with_user_event().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let waker = event_loop.create_proxy();
    // Reloads build the `dev` profile even when started with `--release`:
    // seconds instead of the minute and more an optimized build takes.
    let live = Live::start_with_profile("dev", move || {
        let _ = waker.send_event(Wake::Live);
    });
    let mut app = App {
        ui,
        state,
        backend,
        fonts,
        gfx: None,
        proxy: event_loop.create_proxy(),
        live,
        settle: restored.is_some(),
        calm: false,
        restored,
        input: Input::default(),
        chrome: Chrome::default(),
        wake: None,
        quit: false,
    };
    event_loop.run_app(&mut app).unwrap();
}
