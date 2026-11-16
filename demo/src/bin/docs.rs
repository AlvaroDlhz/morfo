//! The interactive documentation in a window.
//!
//! It draws with the GPU (wgpu) when there is one, and falls back to the
//! CPU renderer (softbuffer) otherwise; `--cpu` forces the CPU. The event
//! loop sleeps (`ControlFlow::Wait`) and only draws when there is input or
//! the UI reports it is still animating: at rest it uses no CPU.
//! The usage profile is saved on exit, so the interface you shaped comes back.
//!
//! It also shows how a host wires up what morfo cannot do by itself: the
//! system clipboard (arboard), input methods, screen readers (AccessKit) and
//! moving and resizing an undecorated window.

use std::collections::HashSet;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use accesskit::{Action, Node, NodeId, Toggled, Tree, TreeUpdate};
use accesskit_winit::{Adapter, Event as AccessEvent, WindowEvent as AccessWindowEvent};
use morfo::{AccessNode, Id, Key, Modifiers, Profile, ResizeEdge, Role, Theme, Ui, Vec2};
use morfo_demo::{build, State, WINDOW};
use morfo_skia::{tiny_skia::Pixmap, Renderer};
use morfo_wgpu::wgpu;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{CursorIcon, ResizeDirection, Window, WindowId};

/// Where frames are painted. There is only one, so the size of its
/// variants does not matter.
#[allow(clippy::large_enum_variant)]
enum Painter {
    Cpu {
        surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
        pixmap: Pixmap,
        renderer: Renderer,
    },
    Gpu {
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        renderer: morfo_wgpu::Renderer,
    },
}

impl Painter {
    /// The GPU if asked for and available, the CPU otherwise.
    fn new(window: &Arc<Window>, gpu: bool) -> Painter {
        if gpu {
            match Self::gpu(window) {
                Ok(p) => return p,
                Err(e) => eprintln!("GPU not available ({e}); drawing with the CPU"),
            }
        }
        let context = softbuffer::Context::new(window.clone()).unwrap();
        let mut surface = softbuffer::Surface::new(&context, window.clone()).unwrap();
        let size = window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        surface.resize(NonZeroU32::new(w).unwrap(), NonZeroU32::new(h).unwrap()).unwrap();
        Painter::Cpu { surface, pixmap: Pixmap::new(w, h).unwrap(), renderer: morfo_demo::renderer() }
    }

    fn gpu(window: &Arc<Window>) -> Result<Painter, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;
        let options = wgpu::RequestAdapterOptions { compatible_surface: Some(&surface), ..Default::default() };
        let adapter = pollster::block_on(instance.request_adapter(&options)).ok_or("no adapter")?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None)).map_err(|e| e.to_string())?;
        let size = window.inner_size();
        let mut config = surface.get_default_config(&adapter, size.width.max(1), size.height.max(1)).ok_or("surface not supported")?;
        config.format = morfo_wgpu::Renderer::pick_format(&surface.get_capabilities(&adapter).formats);
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);
        let renderer = morfo_demo::gpu_renderer(&device, config.format);
        Ok(Painter::Gpu { surface, device, queue, config, renderer })
    }

    fn name(&self) -> &'static str {
        match self {
            Painter::Cpu { .. } => "CPU",
            Painter::Gpu { .. } => "GPU",
        }
    }

    /// The window lost its contents (resized, uncovered): repaint it all.
    fn invalidate(&mut self) {
        match self {
            Painter::Cpu { renderer, .. } => renderer.invalidate(),
            Painter::Gpu { renderer, .. } => renderer.invalidate(),
        }
    }

    fn paint(&mut self, ui: &Ui, w: NonZeroU32, h: NonZeroU32, scale: f32) {
        match self {
            Painter::Cpu { surface, pixmap, renderer } => {
                if pixmap.width() != w.get() || pixmap.height() != h.get() {
                    *pixmap = Pixmap::new(w.get(), h.get()).unwrap();
                    surface.resize(w, h).unwrap();
                }
                let damage = renderer.render(ui.draw_list(), pixmap, scale, ui.background());
                // Copy to the window only the rows that were repainted, when the
                // window buffer still holds the previous frame.
                let mut buffer = surface.buffer_mut().unwrap();
                let rows = if buffer.age() == 1 { damage } else { Some((0, h.get())) };
                if let Some((first, end)) = rows {
                    let (from, to) = (first as usize * w.get() as usize, end as usize * w.get() as usize);
                    for (dst, src) in buffer[from..to].iter_mut().zip(pixmap.data().as_chunks::<4>().0[from..to].iter()) {
                        *dst = (src[0] as u32) << 16 | (src[1] as u32) << 8 | src[2] as u32;
                    }
                }
                buffer.present().unwrap();
            }
            Painter::Gpu { surface, device, queue, config, renderer } => {
                let (w, h) = (w.get(), h.get());
                if config.width != w || config.height != h {
                    config.width = w;
                    config.height = h;
                    surface.configure(device, config);
                }
                // An identical frame is not even presented.
                if !renderer.changed(ui.draw_list(), (w, h), scale, ui.background()) {
                    return;
                }
                let frame = match surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(_) => {
                        surface.configure(device, config);
                        renderer.invalidate();
                        return;
                    }
                };
                let view = frame.texture.create_view(&Default::default());
                renderer.render(device, queue, &view, (w, h), scale, ui.background(), ui.draw_list());
                frame.present();
            }
        }
    }
}

struct Gfx {
    window: Arc<Window>,
    painter: Painter,
    access: Adapter,
}

struct App {
    ui: Ui,
    state: State,
    /// Draw with the GPU when there is one.
    gpu: bool,
    gfx: Option<Gfx>,
    proxy: EventLoopProxy<AccessEvent>,
    clipboard: Option<arboard::Clipboard>,
    /// When the UI asked to be woken with no input (tooltips, toasts, caret).
    wake: Option<Instant>,
    /// Border under the pointer last frame, to change the cursor only when needed.
    edge: Option<ResizeEdge>,
    /// The title bar's close button was pressed.
    quit: bool,
    mods: Modifiers,
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

const ROOT: NodeId = NodeId(u64::MAX);

/// morfo's description of the frame, as an AccessKit tree.
fn access_tree(nodes: &[AccessNode], focus: Option<Id>, scale: f64) -> TreeUpdate {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(nodes.len() + 1);
    let mut children = Vec::with_capacity(nodes.len());
    for n in nodes.iter().filter(|n| seen.insert(n.id)) {
        let role = match n.role {
            Role::Button | Role::ToggleButton => accesskit::Role::Button,
            Role::Switch => accesskit::Role::Switch,
            Role::CheckBox => accesskit::Role::CheckBox,
            Role::RadioButton => accesskit::Role::RadioButton,
            Role::Slider => accesskit::Role::Slider,
            Role::TextInput => accesskit::Role::TextInput,
            Role::MultilineTextInput => accesskit::Role::MultilineTextInput,
            Role::ComboBox => accesskit::Role::ComboBox,
            Role::ListItem => accesskit::Role::ListItem,
            Role::MenuItem => accesskit::Role::MenuItem,
            Role::Tab => accesskit::Role::Tab,
            Role::Link => accesskit::Role::Link,
            Role::Label => accesskit::Role::Label,
            Role::Heading => accesskit::Role::Heading,
            Role::Table => accesskit::Role::Table,
            Role::Row => accesskit::Role::Row,
        };
        let mut node = Node::new(role);
        node.set_label(n.label.as_str());
        if let Some(v) = &n.value {
            node.set_value(v.as_str());
        }
        if let Some(c) = n.checked {
            node.set_toggled(if c { Toggled::True } else { Toggled::False });
        }
        let r = n.rect;
        node.set_bounds(accesskit::Rect {
            x0: r.x as f64 * scale,
            y0: r.y as f64 * scale,
            x1: r.right() as f64 * scale,
            y1: r.bottom() as f64 * scale,
        });
        if !matches!(n.role, Role::Label | Role::Heading) {
            node.add_action(Action::Focus);
            node.add_action(Action::Click);
        }
        children.push(NodeId(n.id.0));
        out.push((NodeId(n.id.0), node));
    }
    let mut root = Node::new(accesskit::Role::Window);
    root.set_label("morfo · documentación");
    root.set_children(children);
    out.push((ROOT, root));
    let focus = focus.map(|f| NodeId(f.0)).filter(|f| seen.contains(&Id(f.0))).unwrap_or(ROOT);
    let mut tree = Tree::new(ROOT);
    tree.toolkit_name = Some("morfo".into());
    TreeUpdate { nodes: out, tree: Some(tree), focus }
}

/// Local offset from UTC in seconds (from `date`, which knows the time zone).
fn local_utc_offset() -> f64 {
    let out = std::process::Command::new("date").arg("+%z").output().ok();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let (sign, digits) = match text.split_at_checked(1) {
        Some(("-", d)) => (-1.0, d),
        Some((_, d)) => (1.0, d),
        None => return 0.0,
    };
    let n: f64 = digits.parse().unwrap_or(0.0);
    sign * ((n / 100.0).floor() * 3600.0 + (n % 100.0) * 60.0)
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
        // Copied or cut text goes to the system clipboard.
        if let (Some(text), Some(clipboard)) = (self.ui.take_clipboard(), self.clipboard.as_mut()) {
            let _ = clipboard.set_text(text);
        }
        // Keep the input method's candidate window next to the caret.
        if let Some(r) = self.ui.ime_area() {
            gfx.window.set_ime_cursor_area(LogicalPosition::new(r.x, r.y), LogicalSize::new(r.w, r.h));
        }
        // Screen readers: send this frame's description (only while one listens).
        let (ui, s) = (&self.ui, gfx.window.scale_factor());
        gfx.access.update_if_active(|| access_tree(ui.access_nodes(), ui.focused(), s));

        gfx.painter.paint(&self.ui, w, h, scale);

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

    /// Ctrl shortcuts; returns whether the key was one.
    fn shortcut(&mut self, key: &WinitKey) -> bool {
        let WinitKey::Character(c) = key else { return false };
        match c.to_lowercase().as_str() {
            "k" => self.state.palette = !self.state.palette,
            "a" => self.ui.key_pressed(Key::SelectAll),
            "c" => self.ui.key_pressed(Key::Copy),
            "x" => self.ui.key_pressed(Key::Cut),
            "v" => {
                if let Some(text) = self.clipboard.as_mut().and_then(|c| c.get_text().ok()) {
                    self.ui.paste(&text);
                }
            }
            _ => return false,
        }
        true
    }
}

impl ApplicationHandler<AccessEvent> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        // No window-manager decorations: the app draws its own title bar.
        // Created hidden: AccessKit must be attached before it is shown.
        let attrs = Window::default_attributes()
            .with_title("morfo · documentación")
            .with_inner_size(LogicalSize::new(WINDOW.x, WINDOW.y))
            .with_decorations(false)
            .with_visible(false);
        let window = Arc::new(el.create_window(attrs).unwrap());
        let access = Adapter::with_event_loop_proxy(&window, self.proxy.clone());
        window.set_ime_allowed(true);
        window.set_visible(true);
        let painter = Painter::new(&window, self.gpu);
        eprintln!("drawing with the {}", painter.name());
        self.gfx = Some(Gfx { window, painter, access });
    }

    /// Requests from the screen reader.
    fn user_event(&mut self, _: &ActiveEventLoop, event: AccessEvent) {
        match event.window_event {
            AccessWindowEvent::InitialTreeRequested => self.ui.set_accessibility(true),
            AccessWindowEvent::AccessibilityDeactivated => self.ui.set_accessibility(false),
            AccessWindowEvent::ActionRequested(req) => match req.action {
                Action::Click => self.ui.access_click(Id(req.target.0)),
                Action::Focus => self.ui.access_focus(Id(req.target.0)),
                _ => {}
            },
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
            WindowEvent::ModifiersChanged(m) => {
                self.mods = Modifiers { shift: m.state().shift_key(), ctrl: m.state().control_key() };
                self.ui.set_modifiers(self.mods);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.ui.scroll_wheel(match delta {
                    MouseScrollDelta::LineDelta(_, lines) => lines * 48.0,
                    MouseScrollDelta::PixelDelta(p) => (p.y / window.scale_factor()) as f32,
                });
                window.request_redraw();
            }
            // Input methods: text being composed, then the final text.
            WindowEvent::Ime(Ime::Preedit(text, _)) => {
                self.ui.ime_preedit(&text);
                window.request_redraw();
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                self.ui.ime_preedit("");
                self.ui.text_input(&text);
                window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let key = match &event.logical_key {
                    WinitKey::Named(NamedKey::Backspace) => Some(Key::Backspace),
                    WinitKey::Named(NamedKey::Delete) => Some(Key::Delete),
                    WinitKey::Named(NamedKey::ArrowLeft) => Some(Key::Left),
                    WinitKey::Named(NamedKey::ArrowRight) => Some(Key::Right),
                    WinitKey::Named(NamedKey::ArrowUp) => Some(Key::Up),
                    WinitKey::Named(NamedKey::ArrowDown) => Some(Key::Down),
                    WinitKey::Named(NamedKey::Home) => Some(Key::Home),
                    WinitKey::Named(NamedKey::End) => Some(Key::End),
                    WinitKey::Named(NamedKey::Enter) => Some(Key::Enter),
                    WinitKey::Named(NamedKey::Escape) => Some(Key::Escape),
                    WinitKey::Named(NamedKey::Tab) => Some(Key::Tab),
                    WinitKey::Named(NamedKey::Space) => Some(Key::Space),
                    _ => None,
                };
                if self.mods.ctrl {
                    // Text typed with Ctrl held is a shortcut, not text.
                    self.shortcut(&event.logical_key);
                } else {
                    if let Some(key) = key {
                        self.ui.key_pressed(key);
                    }
                    // Space activates controls and is also text.
                    if key.is_none() || key == Some(Key::Space) {
                        if let Some(text) = &event.text {
                            self.ui.text_input(text);
                        }
                    }
                }
                window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() {
    let gpu = !std::env::args().any(|a| a == "--cpu");
    let mut ui = Ui::new(Theme::dark());
    ui.set_text_measure(Box::new(morfo_demo::renderer().measure()));
    match std::fs::read_to_string(profile_path()).ok().and_then(|t| Profile::decode(&t)) {
        Some(profile) => ui.set_profile(profile),
        // First run: a short memory, so the adaptation is visible in seconds.
        None => ui.profile_mut().half_life = 300.0,
    }
    // Habits by time of day, in local time.
    ui.profile_mut().time_of_day = Some(local_utc_offset());

    let event_loop = EventLoop::<AccessEvent>::with_user_event().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        ui,
        state: State::default(),
        gpu,
        gfx: None,
        proxy: event_loop.create_proxy(),
        clipboard: arboard::Clipboard::new().ok(),
        wake: None,
        edge: None,
        quit: false,
        mods: Modifiers::default(),
    };
    event_loop.run_app(&mut app).unwrap();
}
