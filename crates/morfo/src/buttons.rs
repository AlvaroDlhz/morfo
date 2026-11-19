//! More kinds of buttons. They all run on one engine, so every kind supports
//! every [`Variant`] and [`Size`], and a button morphs smoothly when its
//! kind, state, variant or size changes.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use crate::a11y::Role;
use crate::anim::SpringKind::{Smooth, Snappy};
use crate::draw::DrawList;
use crate::geom::{Color, Rect, Vec2};
use crate::id::Id;
use crate::theme::{ACCENT, TEXT};
use crate::ui::{Response, Ui, LINE_HEIGHT};
use crate::widgets::{variant_colors, Size, Variant};

/// Vector icons, crisp at any size: the built-in ones, drawn with strokes,
/// or your own from SVG path data in a 24×24 box, the size most icon sets
/// (Lucide, Material, Tabler, Feather) publish:
///
/// ```
/// # use morfo::Icon;
/// const BELL: Icon = Icon::Stroke("M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9 M10.3 21a1.94 1.94 0 0 0 3.4 0");
/// const DOT: Icon = Icon::Fill("M12 7a5 5 0 1 0 0 10a5 5 0 1 0 0-10Z");
/// ```
///
/// Malformed path data draws nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Icon {
    Plus,
    Minus,
    Close,
    Check,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    ChevronDown,
    Menu,
    Search,
    Home,
    Download,
    Play,
    Pause,
    Star,
    Maximize,
    Restore,
    /// Your own, outlined: SVG path data in a 24×24 box, stroked in the
    /// built-in icons' weight.
    Stroke(&'static str),
    /// Your own, solid: SVG path data in a 24×24 box, filled.
    Fill(&'static str),
}

impl Icon {
    pub const ALL: [Icon; 18] = [
        Icon::Plus,
        Icon::Minus,
        Icon::Close,
        Icon::Check,
        Icon::ArrowLeft,
        Icon::ArrowRight,
        Icon::ArrowUp,
        Icon::ArrowDown,
        Icon::ChevronDown,
        Icon::Menu,
        Icon::Search,
        Icon::Home,
        Icon::Download,
        Icon::Play,
        Icon::Pause,
        Icon::Star,
        Icon::Maximize,
        Icon::Restore,
    ];
}

/// Draws `icon` centered on `c`, fitting a `size`-pixel square.
pub(crate) fn draw_icon(list: &mut DrawList, icon: Icon, c: Vec2, size: f32, color: Color) {
    if size < 1.0 || color.a < 0.002 {
        return;
    }
    let k = size * 0.5;
    let width = (size * 0.12).max(1.4);
    // Points in a unit square (-1..1), optionally rotated.
    let p = |x: f32, y: f32| Vec2::new(c.x + x * k, c.y + y * k);
    let rot = |x: f32, y: f32, a: f32| {
        let (s, co) = a.sin_cos();
        p(x * co - y * s, x * s + y * co)
    };
    let mut l = |a: Vec2, b: Vec2| list.line(a, b, width, color);
    match icon {
        Icon::Stroke(d) | Icon::Fill(d) => {
            let Some(path) = crate::path::Path::svg(d) else { return };
            let (view, into) = (Rect::new(0.0, 0.0, 24.0, 24.0), Rect::new(c.x - k, c.y - k, size, size));
            if matches!(icon, Icon::Fill(_)) {
                path.fill_in(list, view, into, color);
            } else {
                path.stroke_in(list, view, into, width, color);
            }
            return;
        }
        _ => {}
    }
    let mut arrow = |a: f32| {
        l(rot(-0.8, 0.0, a), rot(0.8, 0.0, a));
        l(rot(0.25, -0.55, a), rot(0.8, 0.0, a));
        l(rot(0.25, 0.55, a), rot(0.8, 0.0, a));
    };
    match icon {
        Icon::ArrowRight => return arrow(0.0),
        Icon::ArrowDown => return arrow(FRAC_PI_2),
        Icon::ArrowLeft => return arrow(PI),
        Icon::ArrowUp => return arrow(-FRAC_PI_2),
        _ => {}
    }
    let mut poly = |pts: &[(f32, f32)], closed: bool| {
        for w in pts.windows(2) {
            l(p(w[0].0, w[0].1), p(w[1].0, w[1].1));
        }
        if closed && pts.len() > 2 {
            let (a, b) = (pts[pts.len() - 1], pts[0]);
            l(p(a.0, a.1), p(b.0, b.1));
        }
    };
    match icon {
        Icon::Plus => {
            poly(&[(-0.8, 0.0), (0.8, 0.0)], false);
            poly(&[(0.0, -0.8), (0.0, 0.8)], false);
        }
        Icon::Minus => poly(&[(-0.8, 0.0), (0.8, 0.0)], false),
        Icon::Close => {
            poly(&[(-0.65, -0.65), (0.65, 0.65)], false);
            poly(&[(-0.65, 0.65), (0.65, -0.65)], false);
        }
        Icon::Check => poly(&[(-0.75, 0.05), (-0.25, 0.55), (0.75, -0.5)], false),
        Icon::ChevronDown => poly(&[(-0.6, -0.25), (0.0, 0.35), (0.6, -0.25)], false),
        Icon::Menu => {
            for y in [-0.55, 0.0, 0.55] {
                poly(&[(-0.75, y), (0.75, y)], false);
            }
        }
        Icon::Search => {
            let ring: Vec<(f32, f32)> =
                (0..14).map(|i| (i as f32 / 14.0 * TAU).sin_cos()).map(|(s, co)| (-0.15 + 0.5 * co, -0.15 + 0.5 * s)).collect();
            poly(&ring, true);
            poly(&[(0.22, 0.22), (0.8, 0.8)], false);
        }
        Icon::Home => {
            poly(&[(-0.85, 0.05), (0.0, -0.75), (0.85, 0.05)], false);
            poly(&[(-0.6, -0.12), (-0.6, 0.8), (0.6, 0.8), (0.6, -0.12)], false);
        }
        Icon::Download => {
            poly(&[(0.0, -0.85), (0.0, 0.35)], false);
            poly(&[(-0.45, -0.05), (0.0, 0.35), (0.45, -0.05)], false);
            poly(&[(-0.75, 0.8), (0.75, 0.8)], false);
        }
        Icon::Play => poly(&[(-0.45, -0.7), (0.7, 0.0), (-0.45, 0.7)], true),
        Icon::Pause => {
            poly(&[(-0.35, -0.7), (-0.35, 0.7)], false);
            poly(&[(0.35, -0.7), (0.35, 0.7)], false);
        }
        Icon::Star => {
            let pts: Vec<(f32, f32)> = (0..10)
                .map(|i| {
                    let r = if i % 2 == 0 { 0.88 } else { 0.38 };
                    let (s, co) = (i as f32 / 10.0 * TAU - FRAC_PI_2).sin_cos();
                    (r * co, r * s + 0.06)
                })
                .collect();
            poly(&pts, true);
        }
        Icon::Maximize => poly(&[(-0.7, -0.7), (0.7, -0.7), (0.7, 0.7), (-0.7, 0.7)], true),
        Icon::Restore => {
            poly(&[(-0.7, -0.35), (0.35, -0.35), (0.35, 0.7), (-0.7, 0.7)], true);
            poly(&[(-0.35, -0.35), (-0.35, -0.7), (0.7, -0.7), (0.7, 0.35), (0.35, 0.35)], false);
        }
        _ => {}
    }
}

/// What a button shows and how it behaves.
pub(crate) struct Spec<'a> {
    pub label: &'a str,
    pub icon: Option<Icon>,
    /// Whether the icon slot is open (it opens and closes animated).
    pub show_icon: bool,
    pub variant: Variant,
    pub size: Size,
    /// Busy: shrinks into a circle with a spinner and ignores clicks.
    pub busy: bool,
    /// Seconds the press must last to confirm (hold-to-confirm buttons).
    pub hold: Option<f32>,
    /// On/off state, for buttons that stay pressed.
    pub checked: Option<bool>,
}

pub(crate) fn metrics(font: f32, control: f32, pad: f32, size: Size) -> (f32, f32, f32) {
    match size {
        Size::Sm => (font * 0.87, control * 0.78, pad * 0.7),
        Size::Md => (font, control, pad),
        Size::Lg => (font * 1.18, control * 1.25, pad * 1.45),
    }
}

impl Ui {
    /// The engine behind every button. Returns the response and, for hold
    /// buttons, whether the hold completed this frame.
    pub(crate) fn button_core(&mut self, id: Id, spec: Spec) -> (Response, bool) {
        let t = self.theme;
        let (font, h, pad) = metrics(t.font, t.control, t.pad, spec.size);
        let icon_only = spec.label.is_empty();
        let text_w = if icon_only { 0.0 } else { self.text.width(spec.label, font) };
        let icon_px = (font * 1.1).round();
        let gap = (font * 0.5).round();
        let with_icon = spec.icon.is_some() && spec.show_icon;
        let natural = if icon_only { h } else { text_w + pad * 2.0 + if with_icon { icon_px + gap } else { 0.0 } };
        let target = self.alloc(Vec2::new(if spec.busy { h } else { natural }, h));
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        if self.a11y_on {
            let role = if spec.checked.is_some() { Role::ToggleButton } else { Role::Button };
            let name = if spec.label.is_empty() { spec.icon.map(|i| format!("{i:?}")).unwrap_or_default() } else { spec.label.to_string() };
            self.describe(id, role, &name, r, spec.busy.then_some("busy"), spec.checked);
        }
        let interactive = !spec.busy;
        let (fill, fill_hover, stroke, ink) = variant_colors(&self.pal, spec.variant);

        let now = self.now;
        let (snappy, smooth) = (self.steps[Snappy as usize], self.steps[Smooth as usize]);
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        // Hold to confirm: progress follows the clock while pressed, and
        // springs back if the press ends early.
        let mut done = false;
        let mut holding = false;
        if let Some(secs) = spec.hold {
            if hit.held && interactive {
                if !n.open {
                    n.open = true;
                    n.stamp = now;
                }
                let p = if n.latch { 1.0 } else { ((now - n.stamp) as f32 / secs.max(0.05)).min(1.0) };
                if p >= 1.0 && !n.latch {
                    n.latch = true;
                    done = true;
                }
                n.e2.snap(p);
                holding = !n.latch;
            } else {
                n.open = false;
                n.latch = false;
                n.e2.set(0.0);
            }
        }
        let anim = n.hover.drive((hit.hovered && interactive) as u8 as f32, fresh, &snappy)
            | n.press.drive((hit.held && interactive) as u8 as f32, fresh, &snappy)
            | n.fill.drive(fill, fresh, &smooth)
            | n.stroke.drive(stroke, fresh, &smooth)
            | n.ink.drive(ink, fresh, &smooth)
            | n.aux.drive(font, fresh, &smooth)
            | n.radius.drive(with_icon as u8 as f32, fresh, &smooth)
            | n.e1.drive(spec.busy as u8 as f32, fresh, &smooth)
            | n.e2.tick(&smooth);
        let (hover, press) = (n.hover.get().clamp(0.0, 1.0), n.press.get());
        let (fill, stroke, ink, font_now) = (n.fill.get(), n.stroke.get(), n.ink.get(), n.aux.get());
        let (icon_t, busy_t, hold_t) = (n.radius.get().clamp(0.0, 1.2), n.e1.get().clamp(0.0, 1.0), n.e2.get().clamp(0.0, 1.0));
        self.animating |= anim || holding || spec.busy;

        let a = self.alpha * life;
        let r = r.scaled((0.92 + 0.08 * life) * (1.0 - 0.045 * press));
        let ink = if spec.variant == Variant::Ghost { ink.mix(self.pal[TEXT], hover) } else { ink };
        self.draw.rect_stroked(r, r.h * 0.5, fill.mix(fill_hover, hover).fade(a), stroke.fade(a), 1.0);
        if hold_t > 0.002 {
            let overlay = if spec.variant == Variant::Solid { Color::WHITE.with_alpha(0.3) } else { self.pal[ACCENT].with_alpha(0.32) };
            self.draw.rect(Rect::new(r.x, r.y, r.w * hold_t, r.h), r.h * 0.5, overlay.fade(a));
        }

        // Content rides the animated box and font size, so it stays centered
        // mid-morph; it fades out while the spinner fades in.
        let content_a = a * (1.0 - busy_t);
        let scale = font_now / font;
        let (tw, ip) = (text_w * scale, icon_px * scale);
        let cy = r.center().y;
        if icon_only {
            if let Some(icon) = spec.icon {
                draw_icon(&mut self.draw, icon, r.center(), ip, ink.fade(content_a));
            }
        } else {
            let slot = if spec.icon.is_some() { icon_t * (ip + gap) } else { 0.0 };
            let x = r.center().x - (tw + slot) * 0.5;
            if let Some(icon) = spec.icon {
                draw_icon(&mut self.draw, icon, Vec2::new(x + ip * icon_t * 0.5, cy), ip * icon_t, ink.fade(content_a * icon_t.min(1.0)));
            }
            let pos = Vec2::new(x + slot, r.y + (r.h - font_now * LINE_HEIGHT) * 0.5);
            self.draw.add_text(pos, font_now, ink.fade(content_a), spec.label);
        }
        if busy_t > 0.01 {
            // Eight dots chasing each other around the center.
            let (radius, dot) = (r.h * 0.2, (r.h * 0.085).max(2.0));
            let turn = (now * 1.2).fract() as f32 * TAU;
            for i in 0..8 {
                let ang = turn + i as f32 / 8.0 * TAU;
                let c = r.center() + Vec2::new(ang.cos(), ang.sin()) * radius;
                let alpha = (i + 1) as f32 / 8.0;
                self.draw.rect(Rect::new(c.x - dot * 0.5, c.y - dot * 0.5, dot, dot), dot, ink.fade(a * busy_t * alpha));
            }
        }

        let clicked = hit.clicked && interactive && spec.hold.is_none();
        if clicked || done {
            match (spec.label, spec.icon) {
                ("", Some(icon)) => self.learn(id, &format!("{icon:?}")),
                (label, _) => self.learn(id, label),
            }
        }
        (
            Response {
                clicked,
                hovered: hit.hovered && interactive,
                held: hit.held && interactive,
                long_pressed: hit.long_pressed && interactive,
                rect: r,
            },
            done,
        )
    }

    /// A round button with only an icon.
    pub fn icon_button(&mut self, key: &str, icon: Icon, variant: Variant, size: Size) -> Response {
        let id = self.declare(key);
        let spec = Spec { label: "", icon: Some(icon), show_icon: true, variant, size, busy: false, hold: None, checked: None };
        self.button_core(id, spec).0
    }

    /// A button with an icon before its label.
    pub fn button_icon(&mut self, key: &str, icon: Icon, label: &str, variant: Variant, size: Size) -> Response {
        let id = self.declare(key);
        self.button_icon_id(id, icon, label, variant, size)
    }

    pub(crate) fn button_icon_id(&mut self, id: Id, icon: Icon, label: &str, variant: Variant, size: Size) -> Response {
        let spec = Spec { label, icon: Some(icon), show_icon: true, variant, size, busy: false, hold: None, checked: None };
        self.button_core(id, spec).0
    }

    /// A button that stays pressed. When on, it takes a stronger variant and
    /// a check mark slides in before its label. Returns `true` when flipped.
    pub fn toggle_button(&mut self, key: &str, label: &str, value: &mut bool, variant: Variant, size: Size) -> bool {
        let id = self.declare(key);
        let (off, on) = match variant {
            Variant::Solid | Variant::Soft => (Variant::Soft, Variant::Solid),
            Variant::Outline => (Variant::Outline, Variant::Solid),
            Variant::Ghost => (Variant::Ghost, Variant::Soft),
        };
        let spec = Spec {
            label,
            icon: Some(Icon::Check),
            show_icon: *value,
            variant: if *value { on } else { off },
            size,
            busy: false,
            hold: None,
            checked: Some(*value),
        };
        let clicked = self.button_core(id, spec).0.clicked;
        if clicked {
            *value = !*value;
        }
        clicked
    }

    /// A button with a busy state: while `loading`, it shrinks into a circle
    /// with a spinner and ignores clicks; then it grows back.
    pub fn loading_button(&mut self, key: &str, label: &str, loading: bool, variant: Variant, size: Size) -> Response {
        let id = self.declare(key);
        let spec = Spec { label, icon: None, show_icon: false, variant, size, busy: loading, hold: None, checked: None };
        self.button_core(id, spec).0
    }

    /// A button for consequential actions: it only fires after being held
    /// for `seconds`, filling up while pressed and emptying if let go
    /// early. Returns `true` once, the moment the hold completes.
    pub fn hold_button(&mut self, key: &str, label: &str, seconds: f32, variant: Variant, size: Size) -> bool {
        let id = self.declare(key);
        let spec = Spec { label, icon: None, show_icon: false, variant, size, busy: false, hold: Some(seconds), checked: None };
        self.button_core(id, spec).1
    }

    /// Current time, as given to [`begin`](Self::begin).
    pub fn now(&self) -> f64 {
        self.now
    }
}
