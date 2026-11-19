//! A window for a [`morfo`] interface, in one call:
//!
//! ```no_run
//! let mut clicks = 0;
//! morfo_winit::run(morfo_winit::Options::new("Hello"), move |ui, _| {
//!     ui.heading("Hello");
//!     if ui.button("more", "Click me").clicked {
//!         clicks += 1;
//!     }
//!     ui.label(&format!("{clicks} clicks"));
//! })
//! .unwrap();
//! ```
//!
//! [`run`] opens the window and drives it: it paints with the GPU when
//! there is one and the CPU otherwise, only while something moves (at rest
//! the program sleeps), and wires up what morfo cannot do by itself: the
//! pointer, touch, keys and text, input methods, the clipboard, screen
//! readers (AccessKit), HiDPI, the system's "reduce motion" setting, and
//! saving the usage profile between runs.
//!
//! Hosts with needs of their own (several windows, a live reload, an engine
//! of their own) can build on the pieces instead: [`Painter`], [`Input`],
//! [`access`], [`Chrome`] and [`system`].

pub mod access;
mod chrome;
mod input;
mod painter;
pub mod system;

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use morfo::{Locale, Profile, Theme, TitleBar, Ui, Vec2};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

pub use chrome::{cursor, direction, Chrome};
pub use input::{key_of, Chord, Handled, Input};
pub use painter::{Backend, Fonts, Painter};
pub use winit;

/// How the window and the interface start.
pub struct Options {
    pub title: String,
    /// Inner size, in logical pixels.
    pub size: Vec2,
    pub theme: Theme,
    pub locale: Locale,
    pub backend: Backend,
    /// The fonts to draw with (`None`: the system's).
    pub fonts: Option<Fonts>,
    /// The window manager's title bar and borders. Turn them off to draw
    /// your own with [`Ui::title_bar`] and hand its result to
    /// [`Context::title_bar`].
    pub decorations: bool,
    /// Start from the system's "reduce motion" setting.
    pub system_motion: bool,
    /// Keep what the interface learns about its user between runs, in this
    /// application's settings folder ([`system::config_dir`]) under this
    /// name. `None`: forget it on exit.
    pub app_id: Option<String>,
    /// Most frames per second while something moves (`None`: as many as
    /// the display shows). An interface at rest draws nothing either way.
    pub max_fps: Option<f32>,
    /// On battery, animate at no more than 30 frames per second: motion
    /// stays smooth enough and the processor sleeps half the time.
    pub battery_saver: bool,
}

impl Options {
    /// A decorated 900×700 window titled `title`, in the dark theme.
    pub fn new(title: &str) -> Options {
        Options {
            title: title.to_string(),
            size: Vec2::new(900.0, 700.0),
            theme: Theme::dark(),
            locale: Locale::ENGLISH,
            backend: Backend::Auto,
            fonts: None,
            decorations: true,
            system_motion: true,
            app_id: None,
            max_fps: None,
            battery_saver: true,
        }
    }
}

/// Why the window could not run.
#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// What the application can ask of the window while building a frame.
pub struct Context<'a> {
    window: &'a Window,
    chord: Option<Chord>,
    quit: bool,
    bar: Option<TitleBar>,
}

impl Context<'_> {
    /// The window, for whatever winit can do with it.
    pub fn window(&self) -> &Window {
        self.window
    }

    /// Closes the window once this frame is done.
    pub fn quit(&mut self) {
        self.quit = true;
    }

    /// What the application's own title bar asked for this frame (moving,
    /// resizing, minimizing, maximizing, closing): it is carried out.
    pub fn title_bar(&mut self, bar: TitleBar) {
        self.bar = Some(bar);
    }

    /// A Ctrl+key (in lower case) pressed since the last frame that was not
    /// an editing shortcut: for the application's own (Ctrl+K…).
    pub fn chord(&self) -> Option<&str> {
        self.chord.as_ref().map(|c| c.0.as_str())
    }
}

/// Wall-clock seconds: a saved profile keeps decaying between runs.
pub fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

struct Gfx {
    window: Arc<Window>,
    painter: Painter,
    access: accesskit_winit::Adapter,
}

struct Host<F> {
    ui: Ui,
    app: F,
    options: Options,
    fonts: Fonts,
    gfx: Option<Gfx>,
    proxy: EventLoopProxy<accesskit_winit::Event>,
    input: Input,
    chrome: Chrome,
    chord: Option<Chord>,
    wake: Option<Instant>,
    profile: Option<PathBuf>,
    error: Option<Error>,
    /// Whether the machine runs on battery, and when that was last asked.
    battery: (bool, Option<Instant>),
}

impl<F: FnMut(&mut Ui, &mut Context)> Host<F> {
    fn redraw(&mut self, el: &ActiveEventLoop) {
        let Some(gfx) = self.gfx.as_mut() else { return };
        let size = gfx.window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        let scale = gfx.window.scale_factor() as f32;
        let viewport = Vec2::new(size.width as f32 / scale, size.height as f32 / scale);
        let started = Instant::now();

        self.ui.begin(now(), viewport);
        let mut cx = Context { window: &gfx.window, chord: self.chord.take(), quit: false, bar: None };
        (self.app)(&mut self.ui, &mut cx);
        let (quit, bar) = (cx.quit, cx.bar);
        let frame = self.ui.end();

        let close = bar.is_some_and(|bar| self.chrome.apply(&mut self.ui, &gfx.window, bar));
        self.input.after_frame(&mut self.ui, &gfx.window);
        let (ui, s, title) = (&self.ui, gfx.window.scale_factor(), &self.options.title);
        gfx.access.update_if_active(|| access::tree(ui.access_nodes(), ui.focused(), s, title));
        gfx.painter.paint(&self.ui, w, h, scale);

        self.wake = None;
        let cap = if frame.animating { self.fps_cap(started) } else { None };
        let Some(gfx) = self.gfx.as_mut() else { return };
        if frame.animating {
            match cap {
                // The next frame when its turn comes, not as soon as possible.
                Some(fps) => self.wake = Some(started + Duration::from_secs_f32(1.0 / fps)),
                None => gfx.window.request_redraw(),
            }
        } else {
            self.wake = frame.wake_in.map(|s| Instant::now() + Duration::from_secs_f32(s));
        }
        if quit || close {
            self.exit(el);
        }
    }

    /// The frame rate to keep animations to, if any.
    fn fps_cap(&mut self, now: Instant) -> Option<f32> {
        let mut cap = self.options.max_fps.filter(|f| *f > 0.0);
        if self.options.battery_saver {
            // Asked every ten seconds: it is a file read or a command.
            if self.battery.1.is_none_or(|t| now.duration_since(t) > Duration::from_secs(10)) {
                self.battery = (system::on_battery(), Some(now));
            }
            if self.battery.0 {
                cap = Some(cap.map_or(30.0, |c| c.min(30.0)));
            }
        }
        cap
    }

    fn exit(&mut self, el: &ActiveEventLoop) {
        if let Some(path) = &self.profile {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = std::fs::write(path, self.ui.profile().encode()) {
                eprintln!("morfo: could not save the profile to {}: {e}", path.display());
            }
        }
        el.exit();
    }
}

impl<F: FnMut(&mut Ui, &mut Context)> ApplicationHandler<accesskit_winit::Event> for Host<F> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        // Created hidden: AccessKit must be attached before it is shown.
        let attrs = Window::default_attributes()
            .with_title(self.options.title.clone())
            .with_inner_size(LogicalSize::new(self.options.size.x, self.options.size.y))
            .with_decorations(self.options.decorations)
            .with_visible(false);
        let window = match el.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.error = Some(Error(format!("could not open a window: {e}")));
                return el.exit();
            }
        };
        let access = accesskit_winit::Adapter::with_event_loop_proxy(el, &window, self.proxy.clone());
        window.set_ime_allowed(true);
        window.set_visible(true);
        let painter = match Painter::new(&window, self.options.backend, &self.fonts) {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(Error(format!("could not paint: {e}")));
                return el.exit();
            }
        };
        self.ui.set_text_measure(painter.measure());
        self.gfx = Some(Gfx { window, painter, access });
    }

    fn user_event(&mut self, _: &ActiveEventLoop, event: accesskit_winit::Event) {
        access::handle(&mut self.ui, event.window_event);
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
            WindowEvent::CloseRequested => self.exit(el),
            WindowEvent::RedrawRequested => self.redraw(el),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } | WindowEvent::Occluded(false) => {
                gfx.painter.invalidate();
                window.request_redraw();
            }
            event => {
                let handled = self.input.handle(&mut self.ui, &window, &event);
                if handled.chord.is_some() {
                    self.chord = handled.chord;
                }
                if handled.redraw {
                    window.request_redraw();
                }
            }
        }
    }
}

/// Opens a window and runs `app` in it until it is closed. `app(ui, cx)`
/// builds each frame; `cx` reaches the window. Returns when the window
/// closes.
pub fn run(options: Options, app: impl FnMut(&mut Ui, &mut Context)) -> Result<(), Error> {
    let fonts = match options.fonts.clone() {
        Some(f) => f,
        None => Fonts::system().ok_or_else(|| Error("no system font found; set MORFO_FONT to a .ttf file".into()))?,
    };
    let mut ui = Ui::new(options.theme);
    ui.set_locale(options.locale);
    if options.system_motion {
        ui.set_motion(system::motion_preference());
    }
    let profile = options.app_id.as_ref().map(|id| system::config_dir(id).join("profile.txt"));
    if let Some(saved) = profile.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| Profile::decode(&t)) {
        ui.set_profile(saved);
    }
    ui.profile_mut().time_of_day = Some(system::utc_offset());

    let event_loop = EventLoop::<accesskit_winit::Event>::with_user_event().build().map_err(|e| Error(e.to_string()))?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut host = Host {
        ui,
        app,
        proxy: event_loop.create_proxy(),
        options,
        fonts,
        gfx: None,
        input: Input::default(),
        chrome: Chrome::default(),
        chord: None,
        wake: None,
        profile,
        error: None,
        battery: (false, None),
    };
    event_loop.run_app(&mut host).map_err(|e| Error(e.to_string()))?;
    host.error.map_or(Ok(()), Err)
}
