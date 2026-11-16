//! The interactive documentation in a window (winit + softbuffer, CPU only).
//!
//! The event loop sleeps (`ControlFlow::Wait`) and only draws when there is
//! input or the UI reports it is still animating: at rest it uses no CPU.
//! The usage profile is saved on exit, so the interface you shaped comes back.

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use morfo::{Key, Profile, ResizeEdge, Theme, Ui, Vec2};
use morfo_demo::{build, State, WINDOW};
use morfo_skia::{tiny_skia::Pixmap, Renderer};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{CursorIcon, ResizeDirection, Window, WindowId};

struct Gfx {
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    pixmap: Pixmap,
}

struct App {
    ui: Ui,
    state: State,
    renderer: Renderer,
    gfx: Option<Gfx>,
    /// When the UI asked to be woken with no input (tooltips, toasts, caret).
    wake: Option<Instant>,
    /// Border under the pointer last frame, to change the cursor only when needed.
    edge: Option<ResizeEdge>,
    /// The title bar's close button was pressed.
    quit: bool,
    /// Ctrl is held (for Ctrl+K).
    ctrl: bool,
}

fn direction(e: ResizeEdge) -> ResizeDirection {
    match e {
        ResizeEdge::North => ResizeDirection::North,
        ResizeEdge::South => ResizeDirection::South,
        ResizeEdge::East => ResizeDirection::East,
        ResizeEdge::West => ResizeDirection::West,
        ResizeEdge::NorthEast => ResizeDirection::NorthEast,
        ResizeEdge::NorthWest => ResizeDirection::NorthWest,
        ResizeEdge::SouthEast => ResizeDirection::SouthEast,
        ResizeEdge::SouthWest => ResizeDirection::SouthWest,
    }
}

fn cursor(e: Option<ResizeEdge>) -> CursorIcon {
    match e {
        None => CursorIcon::Default,
        Some(ResizeEdge::North | ResizeEdge::South) => CursorIcon::NsResize,
        Some(ResizeEdge::East | ResizeEdge::West) => CursorIcon::EwResize,
        Some(ResizeEdge::NorthEast | ResizeEdge::SouthWest) => CursorIcon::NeswResize,
        Some(ResizeEdge::NorthWest | ResizeEdge::SouthEast) => CursorIcon::NwseResize,
    }
}

fn profile_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("morfo-demo").join("profile.txt")
}

/// Wall-clock seconds, so the saved profile keeps decaying between sessions.
fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

impl App {
    fn redraw(&mut self) {
        let Some(gfx) = self.gfx.as_mut() else { return };
        let size = gfx.window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        let scale = gfx.window.scale_factor() as f32;
        if gfx.pixmap.width() != w.get() || gfx.pixmap.height() != h.get() {
            gfx.pixmap = Pixmap::new(w.get(), h.get()).unwrap();
            gfx.surface.resize(w, h).unwrap();
        }

        self.state.maximized = gfx.window.is_maximized();
        self.ui.begin(now(), Vec2::new(size.width as f32 / scale, size.height as f32 / scale));
        build(&mut self.ui, &mut self.state);
        let frame = self.ui.end();

        // Carry out what the app's own title bar asked of the window.
        let bar = self.state.chrome;
        if bar.hover_edge != self.edge {
            gfx.window.set_cursor(cursor(bar.hover_edge));
            self.edge = bar.hover_edge;
        }
        if bar.drag || bar.resize.is_some() {
            let _ = match bar.resize {
                Some(edge) => gfx.window.drag_resize_window(direction(edge)),
                None => gfx.window.drag_window(),
            };
            // The window manager takes the pointer from here: the release
            // may never reach us.
            self.ui.pointer_released();
            self.ui.pointer_left();
        }
        if bar.minimize {
            gfx.window.set_minimized(true);
        }
        if bar.toggle_maximize {
            gfx.window.set_maximized(!gfx.window.is_maximized());
        }
        if bar.close {
            self.quit = true;
        }
        let damage = self.renderer.render(self.ui.draw_list(), &mut gfx.pixmap, scale, self.ui.background());

        // Copy to the window only the rows that were repainted, when the
        // window buffer still holds the previous frame.
        let mut buffer = gfx.surface.buffer_mut().unwrap();
        let rows = if buffer.age() == 1 { damage } else { Some((0, h.get())) };
        if let Some((first, end)) = rows {
            let (from, to) = (first as usize * w.get() as usize, end as usize * w.get() as usize);
            for (dst, src) in buffer[from..to].iter_mut().zip(gfx.pixmap.data().as_chunks::<4>().0[from..to].iter()) {
                *dst = (src[0] as u32) << 16 | (src[1] as u32) << 8 | src[2] as u32;
            }
        }
        buffer.present().unwrap();

        self.wake = None;
        if frame.animating {
            gfx.window.request_redraw();
        } else {
            self.wake = frame.wake_in.map(|s| Instant::now() + Duration::from_secs_f32(s));
        }
    }
}

impl App {
    fn save_profile(&self) {
        let path = profile_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&path, self.ui.profile().encode()) {
            eprintln!("could not save profile to {}: {e}", path.display());
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        // No window-manager decorations: the app draws its own title bar.
        let attrs = Window::default_attributes()
            .with_title("morfo · documentación")
            .with_inner_size(LogicalSize::new(WINDOW.x, WINDOW.y))
            .with_decorations(false);
        let window = Rc::new(el.create_window(attrs).unwrap());
        let context = softbuffer::Context::new(window.clone()).unwrap();
        let mut surface = softbuffer::Surface::new(&context, window.clone()).unwrap();
        let size = window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        surface.resize(NonZeroU32::new(w).unwrap(), NonZeroU32::new(h).unwrap()).unwrap();
        self.gfx = Some(Gfx { window, surface, pixmap: Pixmap::new(w, h).unwrap() });
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
        let Some(gfx) = self.gfx.as_ref() else { return };
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
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => window.request_redraw(),
            WindowEvent::CursorMoved { position, .. } => {
                let p = position.to_logical::<f32>(window.scale_factor());
                self.ui.pointer_moved(Vec2::new(p.x, p.y));
                window.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                self.ui.pointer_left();
                window.request_redraw();
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                match state {
                    ElementState::Pressed => self.ui.pointer_pressed(),
                    ElementState::Released => self.ui.pointer_released(),
                }
                window.request_redraw();
            }
            WindowEvent::ModifiersChanged(m) => self.ctrl = m.state().control_key(),
            WindowEvent::MouseWheel { delta, .. } => {
                self.ui.scroll_wheel(match delta {
                    MouseScrollDelta::LineDelta(_, lines) => lines * 48.0,
                    MouseScrollDelta::PixelDelta(p) => (p.y / window.scale_factor()) as f32,
                });
                window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let key = match &event.logical_key {
                    WinitKey::Named(NamedKey::Backspace) => Some(Key::Backspace),
                    WinitKey::Named(NamedKey::Delete) => Some(Key::Delete),
                    WinitKey::Named(NamedKey::ArrowLeft) => Some(Key::Left),
                    WinitKey::Named(NamedKey::ArrowRight) => Some(Key::Right),
                    WinitKey::Named(NamedKey::Home) => Some(Key::Home),
                    WinitKey::Named(NamedKey::End) => Some(Key::End),
                    WinitKey::Named(NamedKey::Enter) => Some(Key::Enter),
                    WinitKey::Named(NamedKey::Escape) => Some(Key::Escape),
                    WinitKey::Named(NamedKey::Tab) => Some(Key::Tab),
                    WinitKey::Named(NamedKey::ArrowUp) => Some(Key::Up),
                    WinitKey::Named(NamedKey::ArrowDown) => Some(Key::Down),
                    _ => None,
                };
                let ctrl_k = self.ctrl && matches!(&event.logical_key, WinitKey::Character(c) if c.eq_ignore_ascii_case("k"));
                match (key, &event.text) {
                    _ if ctrl_k => self.state.palette = !self.state.palette,
                    (Some(key), _) => self.ui.key_pressed(key),
                    // Text typed with Ctrl held is a shortcut, not text.
                    (None, Some(text)) if !self.ctrl => self.ui.text_input(text),
                    _ => {}
                }
                window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() {
    let renderer = morfo_demo::renderer();
    let mut ui = Ui::new(Theme::dark());
    ui.set_text_measure(Box::new(renderer.measure()));
    match std::fs::read_to_string(profile_path()).ok().and_then(|t| Profile::decode(&t)) {
        Some(profile) => ui.set_profile(profile),
        // First run: a short memory, so the adaptation is visible in seconds.
        None => ui.profile_mut().half_life = 300.0,
    }

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App { ui, state: State::default(), renderer, gfx: None, wake: None, edge: None, quit: false, ctrl: false };
    event_loop.run_app(&mut app).unwrap();
}
