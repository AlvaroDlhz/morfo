//! Web backend for [`morfo`]: paints a [`DrawList`] on an HTML `<canvas>`
//! and connects the browser's events to a [`Ui`].
//!
//! [`run`] is all most apps need: it sizes the canvas to its CSS box and the
//! screen's pixel ratio, forwards pointer, wheel, keyboard and clipboard
//! events (typing goes through a hidden text area, so mobile keyboards and
//! input methods work too), keeps the usage profile in `localStorage`, and only asks the
//! browser for a frame when there was input or something is still moving —
//! at rest the page does no work at all.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use morfo::ui::LINE_HEIGHT;
use morfo::{Color, DrawCmd, DrawList, Key, Modifiers, Profile, Rect, TextMeasure, Theme, Ui, Vec2};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

const SANS: &str = "system-ui, -apple-system, 'Segoe UI', Roboto, 'Noto Sans', sans-serif";
const MONO: &str = "ui-monospace, 'SF Mono', Menlo, Consolas, 'DejaVu Sans Mono', monospace";
const PROFILE_KEY: &str = "morfo-profile";

fn css(c: Color) -> String {
    let [r, g, b, _] = c.to_rgba8();
    format!("rgba({r},{g},{b},{})", c.a.clamp(0.0, 1.0))
}

fn font(size: f32, mono: bool) -> String {
    format!("{size}px {}", if mono { MONO } else { SANS })
}

/// Text metrics from the canvas itself (cached), for [`Ui::set_text_measure`].
pub struct CanvasMeasure {
    ctx: CanvasRenderingContext2d,
    cache: RefCell<HashMap<(String, u32), f32>>,
}

impl TextMeasure for CanvasMeasure {
    fn width(&self, text: &str, size: f32) -> f32 {
        let key = (text.to_string(), size.to_bits());
        if let Some(&w) = self.cache.borrow().get(&key) {
            return w;
        }
        self.ctx.set_font(&font(size, false));
        let w = self.ctx.measure_text(text).map_or(0.0, |m| m.width() as f32);
        let mut cache = self.cache.borrow_mut();
        if cache.len() > 8192 {
            cache.clear();
        }
        cache.insert(key, w);
        w
    }
}

/// Paints draw lists with the canvas 2D API.
pub struct CanvasRenderer {
    ctx: CanvasRenderingContext2d,
}

impl CanvasRenderer {
    pub fn new(canvas: &HtmlCanvasElement) -> Result<CanvasRenderer, JsValue> {
        let ctx = canvas.get_context("2d")?.ok_or("no 2d context")?.dyn_into::<CanvasRenderingContext2d>()?;
        Ok(CanvasRenderer { ctx })
    }

    pub fn measure(&self) -> CanvasMeasure {
        CanvasMeasure { ctx: self.ctx.clone(), cache: RefCell::new(HashMap::new()) }
    }

    /// Paints `list` over `background`; `size` is the canvas size in logical
    /// (CSS) pixels and `scale` the device pixel ratio.
    pub fn render(&self, list: &DrawList, size: Vec2, scale: f32, background: Color) {
        let ctx = &self.ctx;
        let _ = ctx.set_transform(scale as f64, 0.0, 0.0, scale as f64, 0.0, 0.0);
        ctx.set_fill_style_str(&css(background));
        ctx.fill_rect(0.0, 0.0, size.x as f64, size.y as f64);
        ctx.set_text_baseline("middle");
        ctx.set_line_cap("round");
        for cmd in list.commands() {
            match *cmd {
                DrawCmd::Nop => {}
                DrawCmd::Rect { rect, radius, fill, stroke, stroke_width } => {
                    if fill.a > 0.002 {
                        rounded(ctx, rect, radius);
                        ctx.set_fill_style_str(&css(fill));
                        ctx.fill();
                    }
                    if stroke_width > 0.0 && stroke.a > 0.002 {
                        rounded(ctx, rect.inset(stroke_width * 0.5), radius - stroke_width * 0.5);
                        ctx.set_line_width(stroke_width as f64);
                        ctx.set_stroke_style_str(&css(stroke));
                        ctx.stroke();
                    }
                }
                DrawCmd::Shadow { rect, radius, blur, offset, color } => {
                    // The canvas blurs in device pixels, untouched by the transform.
                    ctx.save();
                    ctx.set_shadow_color(&css(color));
                    ctx.set_shadow_blur((blur * scale * 0.6) as f64);
                    ctx.set_shadow_offset_y((offset.y * scale) as f64);
                    rounded(ctx, rect, radius);
                    ctx.set_fill_style_str(&css(color));
                    ctx.fill();
                    ctx.restore();
                }
                DrawCmd::Text { pos, size, color, start, len, mono } => {
                    ctx.set_font(&font(size, mono));
                    ctx.set_fill_style_str(&css(color));
                    let _ = ctx.fill_text(list.text(start, len), pos.x as f64, (pos.y + size * LINE_HEIGHT * 0.5) as f64);
                }
                DrawCmd::Line { from, to, width, color } => {
                    ctx.begin_path();
                    ctx.move_to(from.x as f64, from.y as f64);
                    ctx.line_to(to.x as f64, to.y as f64);
                    ctx.set_line_width(width as f64);
                    ctx.set_stroke_style_str(&css(color));
                    ctx.stroke();
                }
                DrawCmd::Polygon { start, len, color } => {
                    let pts = list.points(start, len);
                    ctx.begin_path();
                    ctx.move_to(pts[0].x as f64, pts[0].y as f64);
                    for p in &pts[1..] {
                        ctx.line_to(p.x as f64, p.y as f64);
                    }
                    ctx.close_path();
                    ctx.set_fill_style_str(&css(color));
                    ctx.fill();
                }
                DrawCmd::PushClip { rect, .. } => {
                    ctx.save();
                    ctx.begin_path();
                    ctx.rect(rect.x as f64, rect.y as f64, rect.w as f64, rect.h as f64);
                    ctx.clip();
                }
                DrawCmd::PopClip => ctx.restore(),
            }
        }
    }
}

/// A rounded rectangle as the current path.
fn rounded(ctx: &CanvasRenderingContext2d, r: Rect, radius: f32) {
    let rad = radius.clamp(0.0, (r.w * 0.5).min(r.h * 0.5)) as f64;
    let (x, y, w, h) = (r.x as f64, r.y as f64, r.w.max(0.0) as f64, r.h.max(0.0) as f64);
    ctx.begin_path();
    ctx.move_to(x + rad, y);
    let _ = ctx.arc_to(x + w, y, x + w, y + h, rad);
    let _ = ctx.arc_to(x + w, y + h, x, y + h, rad);
    let _ = ctx.arc_to(x, y + h, x, y, rad);
    let _ = ctx.arc_to(x, y, x + w, y, rad);
    ctx.close_path();
}

struct Host {
    ui: Ui,
    renderer: CanvasRenderer,
    canvas: HtmlCanvasElement,
    /// Invisible text area that receives the keyboard, kept at the caret.
    input: web_sys::HtmlTextAreaElement,
    build: Box<dyn FnMut(&mut Ui)>,
    scheduled: bool,
    frame: Option<Closure<dyn FnMut()>>,
}

type Shared = Rc<RefCell<Host>>;

fn window() -> web_sys::Window {
    web_sys::window().expect("no window")
}

/// Asks the browser for one frame, unless one is already on its way.
fn schedule(host: &Shared) {
    let mut h = host.borrow_mut();
    if h.scheduled {
        return;
    }
    h.scheduled = true;
    if let Some(f) = h.frame.as_ref() {
        let _ = window().request_animation_frame(f.as_ref().unchecked_ref());
    }
}

fn frame(host: &Shared) {
    let (animating, wake) = {
        let mut guard = host.borrow_mut();
        let h = &mut *guard;
        h.scheduled = false;
        let scale = window().device_pixel_ratio() as f32;
        let size = Vec2::new(h.canvas.client_width() as f32, h.canvas.client_height() as f32);
        let (pw, ph) = ((size.x * scale).round() as u32, (size.y * scale).round() as u32);
        if h.canvas.width() != pw || h.canvas.height() != ph {
            h.canvas.set_width(pw);
            h.canvas.set_height(ph);
        }
        let now = window().performance().map_or(0.0, |p| p.now() / 1000.0);
        h.ui.begin(now, size);
        (h.build)(&mut h.ui);
        let f = h.ui.end();
        h.renderer.render(h.ui.draw_list(), size, scale, h.ui.background());
        if let Some(text) = h.ui.take_clipboard() {
            let _ = window().navigator().clipboard().write_text(&text);
        }
        // Keep the hidden text area at the caret: input methods open there.
        if let Some(r) = h.ui.ime_area() {
            let rect = h.canvas.get_bounding_client_rect();
            let style = h.input.style();
            let _ = style.set_property("left", &format!("{}px", rect.left() + r.x as f64));
            let _ = style.set_property("top", &format!("{}px", rect.top() + r.y as f64));
            let _ = style.set_property("height", &format!("{}px", r.h));
        }
        (f.animating, f.wake_in)
    };
    if animating {
        schedule(host);
    } else if let Some(s) = wake {
        let h = host.clone();
        let cb = Closure::once_into_js(move || schedule(&h));
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), (s * 1000.0).ceil() as i32);
    }
}

fn key(k: &str) -> Option<Key> {
    Some(match k {
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
        "ArrowLeft" => Key::Left,
        "ArrowRight" => Key::Right,
        "ArrowUp" => Key::Up,
        "ArrowDown" => Key::Down,
        "Home" => Key::Home,
        "End" => Key::End,
        "Enter" => Key::Enter,
        "Escape" => Key::Escape,
        "Tab" => Key::Tab,
        " " => Key::Space,
        _ => return None,
    })
}

/// Adds an event listener for the whole life of the page.
fn listen<E: JsCast + wasm_bindgen::convert::FromWasmAbi + 'static>(
    target: &web_sys::EventTarget,
    kind: &str,
    passive: bool,
    f: impl FnMut(E) + 'static,
) {
    let cb = Closure::<dyn FnMut(E)>::new(f);
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_passive(passive);
    let _ = target.add_event_listener_with_callback_and_add_event_listener_options(kind, cb.as_ref().unchecked_ref(), &opts);
    cb.forget();
}

/// Runs an interface on the canvas with id `canvas_id`. `build` declares the
/// interface each frame, as on any other backend.
pub fn run(canvas_id: &str, theme: Theme, build: impl FnMut(&mut Ui) + 'static) -> Result<(), JsValue> {
    let document = window().document().ok_or("no document")?;
    let canvas = document.get_element_by_id(canvas_id).ok_or("no such canvas")?.dyn_into::<HtmlCanvasElement>()?;
    // Focusable, so it receives the keyboard; no browser focus outline.
    canvas.set_tab_index(0);
    let _ = canvas.style().set_property("outline", "none");
    let renderer = CanvasRenderer::new(&canvas)?;
    let mut ui = Ui::new(theme);
    ui.set_text_measure(Box::new(renderer.measure()));
    let storage = window().local_storage().ok().flatten();
    if let Some(profile) = storage.as_ref().and_then(|s| s.get_item(PROFILE_KEY).ok().flatten()).and_then(|t| Profile::decode(&t)) {
        ui.set_profile(profile);
    }
    // The keyboard goes to an invisible text area: it gets real text input
    // (mobile keyboards, dictation, input methods) that a canvas cannot.
    let input = document.create_element("textarea")?.dyn_into::<web_sys::HtmlTextAreaElement>()?;
    for (k, v) in [
        ("position", "fixed"),
        ("left", "0"),
        ("top", "0"),
        ("width", "1px"),
        ("height", "1px"),
        ("opacity", "0"),
        ("border", "0"),
        ("padding", "0"),
        ("resize", "none"),
        ("pointer-events", "none"),
    ] {
        let _ = input.style().set_property(k, v);
    }
    for (k, v) in [("autocomplete", "off"), ("autocapitalize", "off"), ("spellcheck", "false"), ("aria-hidden", "true")] {
        let _ = input.set_attribute(k, v);
    }
    document.body().ok_or("no body")?.append_child(&input)?;
    let host: Shared = Rc::new(RefCell::new(Host {
        ui,
        renderer,
        canvas: canvas.clone(),
        input: input.clone(),
        build: Box::new(build),
        scheduled: false,
        frame: None,
    }));
    let h = host.clone();
    host.borrow_mut().frame = Some(Closure::new(move || frame(&h)));

    let target: &web_sys::EventTarget = canvas.as_ref();
    let with = |host: &Shared, f: fn(&mut Ui, Vec2)| {
        let host = host.clone();
        move |e: web_sys::PointerEvent| {
            f(&mut host.borrow_mut().ui, Vec2::new(e.offset_x() as f32, e.offset_y() as f32));
            schedule(&host);
        }
    };
    listen(target, "pointermove", true, with(&host, |ui, p| ui.pointer_moved(p)));
    listen(target, "pointerleave", true, with(&host, |ui, _| ui.pointer_left()));
    listen(
        target,
        "pointerup",
        true,
        with(&host, |ui, p| {
            ui.pointer_moved(p);
            ui.pointer_released();
        }),
    );
    {
        let (host, canvas, input) = (host.clone(), canvas.clone(), input.clone());
        listen(target, "pointerdown", true, move |e: web_sys::PointerEvent| {
            let _ = input.focus();
            let _ = canvas.set_pointer_capture(e.pointer_id());
            let mut h = host.borrow_mut();
            h.ui.pointer_moved(Vec2::new(e.offset_x() as f32, e.offset_y() as f32));
            h.ui.pointer_pressed();
            drop(h);
            schedule(&host);
        });
    }
    {
        let host = host.clone();
        listen(target, "wheel", false, move |e: web_sys::WheelEvent| {
            e.prevent_default();
            let lines = if e.delta_mode() == 1 { 40.0 } else { 1.0 };
            host.borrow_mut().ui.scroll_wheel(-(e.delta_y() * lines) as f32);
            schedule(&host);
        });
    }
    let keyboard: &web_sys::EventTarget = input.as_ref();
    let modifiers = |e: &web_sys::KeyboardEvent| Modifiers { shift: e.shift_key(), ctrl: e.ctrl_key() || e.meta_key() };
    {
        let host = host.clone();
        listen(keyboard, "keydown", false, move |e: web_sys::KeyboardEvent| {
            if e.is_composing() {
                return;
            }
            let mods = modifiers(&e);
            let k = e.key();
            let mut h = host.borrow_mut();
            h.ui.set_modifiers(mods);
            if mods.ctrl {
                match k.to_lowercase().as_str() {
                    "a" => {
                        e.prevent_default();
                        h.ui.key_pressed(Key::SelectAll)
                    }
                    "c" => h.ui.key_pressed(Key::Copy),
                    "x" => h.ui.key_pressed(Key::Cut),
                    // Ctrl+V arrives as a paste event, with the text.
                    _ => {}
                }
            } else if let Some(key) = key(&k) {
                // Keep Tab, Space and the arrows inside the interface.
                e.prevent_default();
                h.ui.key_pressed(key);
                if key == Key::Space {
                    h.ui.text_input(" ");
                }
            }
            // Printable text arrives as an input event on the text area.
            drop(h);
            schedule(&host);
        });
    }
    {
        let (host, field) = (host.clone(), input.clone());
        listen(keyboard, "input", true, move |e: web_sys::InputEvent| {
            if e.is_composing() {
                return;
            }
            let text = field.value();
            field.set_value("");
            if !text.is_empty() {
                host.borrow_mut().ui.text_input(&text);
                schedule(&host);
            }
        });
    }
    {
        let host = host.clone();
        listen(keyboard, "compositionupdate", true, move |e: web_sys::CompositionEvent| {
            host.borrow_mut().ui.ime_preedit(&e.data().unwrap_or_default());
            schedule(&host);
        });
    }
    {
        let (host, field) = (host.clone(), input.clone());
        listen(keyboard, "compositionend", true, move |e: web_sys::CompositionEvent| {
            field.set_value("");
            let mut h = host.borrow_mut();
            h.ui.ime_preedit("");
            h.ui.text_input(&e.data().unwrap_or_default());
            drop(h);
            schedule(&host);
        });
    }
    {
        let host = host.clone();
        listen(keyboard, "keyup", true, move |e: web_sys::KeyboardEvent| host.borrow_mut().ui.set_modifiers(modifiers(&e)));
    }
    {
        let host = host.clone();
        listen(keyboard, "paste", false, move |e: web_sys::ClipboardEvent| {
            if let Some(text) = e.clipboard_data().and_then(|d| d.get_data("text").ok()) {
                e.prevent_default();
                host.borrow_mut().ui.paste(&text);
                schedule(&host);
            }
        });
    }
    {
        let host = host.clone();
        listen(window().as_ref(), "resize", true, move |_: web_sys::Event| schedule(&host));
    }
    {
        // The usage profile survives reloads: this person's interface stays theirs.
        let host = host.clone();
        listen(window().as_ref(), "pagehide", true, move |_: web_sys::Event| {
            if let Some(s) = window().local_storage().ok().flatten() {
                let _ = s.set_item(PROFILE_KEY, &host.borrow().ui.profile().encode());
            }
        });
    }
    let _ = input.focus();
    schedule(&host);
    Ok(())
}
