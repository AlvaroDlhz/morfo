//! Floating components: everything that lives above the normal flow.
//!
//! They are drawn in upper paint layers, swallow the pointer over the area
//! they cover, and animate out as well as in: the library keeps building
//! them while their presence spring is above zero.

use crate::anim::smoothstep;
use crate::anim::SpringKind::{Smooth, Snappy};
use crate::draw::DrawCmd;
use crate::geom::{Color, Rect, Vec2};
use crate::id::Id;
use crate::theme::{BG, BORDER, RAISED, SHADOW, TEXT};
use crate::ui::{Dir, Key, Ui, LINE_HEIGHT};
use crate::widgets::{Size, Variant};

pub(crate) struct Toast {
    id: Id,
    text: String,
    until: f64,
}

/// How a floating panel is placed and behaves.
pub(crate) struct Floating {
    /// Shape the panel grows out of (and shrinks back into).
    pub from: Rect,
    /// Top-left corner when open; ignored if `center`.
    pub pos: Vec2,
    pub center: bool,
    /// Fixed width, or `None` to fit the content.
    pub width: Option<f32>,
    pub inset: f32,
    pub gap: f32,
    /// Darken everything behind.
    pub dim: bool,
}

fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t, a.w + (b.w - a.w) * t, a.h + (b.h - a.h) * t)
}

impl Ui {
    /// A panel above the content that morphs out of `cfg.from`. Returns
    /// `true` when the user dismissed it (click outside, Escape, or
    /// [`Ui::close`] from inside).
    pub(crate) fn floating(&mut self, id: Id, shown: bool, cfg: Floating, f: impl FnOnce(&mut Ui)) -> bool {
        let p = self.presence_id(id, shown, Smooth);
        if p <= 0.001 && !shown {
            return false;
        }
        let pc = p.clamp(0.0, 1.0);
        let vp = self.viewport();
        let screen = Rect::new(0.0, 0.0, vp.x, vp.y);
        let mut size = self.sizes.get(&id).copied().unwrap_or(Vec2::ZERO);
        if let Some(w) = cfg.width {
            size.x = w;
        }
        let mut pos = if cfg.center { Vec2::new((vp.x - size.x) * 0.5, (vp.y - size.y) * 0.5) } else { cfg.pos };
        pos.x = pos.x.min(vp.x - size.x - 8.0).max(8.0);
        pos.y = pos.y.min(vp.y - size.y - 8.0).max(8.0);
        // `p` may overshoot a little: the panel lands with a soft bounce.
        let panel = lerp_rect(cfg.from, Rect::from_min_size(pos, size), p);
        let radius = self.theme.radius;
        self.dismiss = false;
        let mut dismissed = false;
        self.detached(1, id.with(0), pos, cfg.width.unwrap_or(4096.0), |ui| {
            if cfg.dim {
                ui.draw.rect(screen, 0.0, Color::BLACK.with_alpha(0.5 * pc));
            }
            let a = smoothstep(pc * 1.6);
            ui.draw.push(DrawCmd::Shadow { rect: panel, radius, blur: 36.0, offset: Vec2::new(0.0, 14.0), color: ui.pal[SHADOW].fade(a) });
            ui.draw.rect_stroked(panel, radius, ui.pal[RAISED].fade(a), ui.pal[BORDER].fade(a), 1.0);
            ui.draw.push(DrawCmd::PushClip { rect: panel, radius: 0.0 });
            ui.clip = Some(panel);
            // Content appears once the panel has opened enough to hold it.
            ui.alpha = smoothstep((pc - 0.25) / 0.75);
            let (_, mut content) = ui.group(id, Dir::Column, Vec2::splat(cfg.inset), cfg.gap, Some(pos), f);
            if let Some(w) = cfg.width {
                content.x = w;
                ui.sizes.insert(id, content);
            }
            ui.draw.push(DrawCmd::PopClip);
            ui.clip = None;
            if shown {
                // Nothing underneath reacts while this is open; a press on
                // the panel itself is swallowed, one outside closes it.
                ui.blockers.push((1, screen));
                ui.hit(id.with(1), panel);
                dismissed = ui.hit(id.with(2), screen).clicked;
            }
        });
        if shown && (self.dismiss || self.keys.contains(&Key::Escape)) {
            dismissed = true;
        }
        self.dismiss = false;
        dismissed
    }

    /// Closes the dialog or menu being built. Call it from inside one.
    pub fn close(&mut self) {
        self.dismiss = true;
    }

    /// A modal dialog that grows out of the element `trigger` (usually the
    /// button that opens it) and shrinks back into it when closed. Clicking
    /// outside, pressing Escape or calling [`Ui::close`] sets `open` to false.
    pub fn dialog(&mut self, key: &str, trigger: &str, open: &mut bool, width: f32, f: impl FnOnce(&mut Ui)) {
        let id = self.id(key);
        let vp = self.viewport();
        let from = self.rect_of(self.id(trigger)).unwrap_or(Rect::new(vp.x * 0.5, vp.y * 0.5, 0.0, 0.0));
        let cfg = Floating { from, pos: Vec2::ZERO, center: true, width: Some(width), inset: self.theme.pad * 1.2, gap: self.theme.gap, dim: true };
        if self.floating(id, *open, cfg, f) {
            *open = false;
        }
    }

    /// A button that unfolds a list of options. `items` are `(key, label)`;
    /// returns the index of the option chosen this frame.
    pub fn menu(&mut self, key: &str, label: &str, items: &[(&str, &str)]) -> Option<usize> {
        let id = self.id(key);
        let (shown, anchor) = self.menu_trigger(id, label);
        let width = self.menu_width(anchor, items.iter().map(|(_, l)| *l));
        let mut picked = None;
        let dismissed = self.floating(id.child("panel"), shown, self.menu_cfg(anchor), |ui| {
            for (i, (item, label)) in items.iter().enumerate() {
                if ui.menu_row(id.child(item), label, width, TEXT) {
                    picked = Some(i);
                }
            }
        });
        if (picked.is_some() || dismissed) && self.open == Some(id) {
            self.open = None;
        }
        picked
    }

    /// The button of a menu; returns whether the menu is open and where it is.
    pub(crate) fn menu_trigger(&mut self, id: Id, label: &str) -> (bool, Rect) {
        let r = self.button_id(id, label, Variant::Outline, Size::Md);
        if r.clicked {
            self.menu_more = false;
            self.open = if self.open == Some(id) { None } else { Some(id) };
        }
        (self.open == Some(id), r.rect)
    }

    pub(crate) fn menu_width<'a>(&self, anchor: Rect, labels: impl Iterator<Item = &'a str>) -> f32 {
        let font = self.theme.font;
        let widest = labels.map(|l| self.text.width(l, font)).fold(0.0, f32::max);
        (widest + self.theme.pad * 1.6).max(anchor.w)
    }

    pub(crate) fn menu_cfg(&self, anchor: Rect) -> Floating {
        Floating {
            from: Rect::new(anchor.x, anchor.bottom(), anchor.w, 4.0),
            pos: Vec2::new(anchor.x, anchor.bottom() + 6.0),
            center: false,
            width: None,
            inset: 6.0,
            gap: 2.0,
            dim: false,
        }
    }

    /// One option of a menu; `true` when clicked.
    pub(crate) fn menu_row(&mut self, id: Id, label: &str, width: f32, color: usize) -> bool {
        let font = self.theme.font;
        let target = self.alloc(Vec2::new(width, (self.theme.control * 0.9).round()));
        let (r, life) = self.enter(id, target, Snappy);
        let hit = self.hit(id, r);
        let (hover, _, _) = self.drive3(id, hit.hovered || hit.held, hit.held, 0.0, Snappy);
        let a = self.alpha * life;
        self.draw.rect(r, 10.0, self.pal[TEXT].with_alpha(0.09 * hover * a));
        let pos = Vec2::new(r.x + self.theme.pad * 0.8, r.y + (r.h - font * LINE_HEIGHT) * 0.5);
        self.draw.add_text(pos, font, self.pal[color].fade(a), label);
        if hit.clicked {
            self.profile.record(id, self.now);
        }
        hit.clicked
    }

    /// A hint that appears after resting the pointer on element `key`
    /// (declare it right after that element).
    pub fn tooltip(&mut self, key: &str, text: &str) {
        const DELAY: f64 = 0.45;
        let target = self.id(key);
        let Some(r) = self.rect_of(target) else { return };
        let hovered = self.pointer_in(r) && self.active.is_none();
        let id = target.child("tooltip");
        let now = self.now;
        let n = self.nodes.entry(id).or_insert_with(crate::ui::Node::new);
        if !hovered {
            n.stamp = 0.0;
        } else if n.stamp == 0.0 {
            n.stamp = now;
        }
        let since = n.stamp;
        let due = hovered && now - since >= DELAY;
        if hovered && !due {
            self.wake_at(since + DELAY);
        }
        let p = self.presence_id(id, due, Snappy).clamp(0.0, 1.0);
        if p <= 0.001 {
            return;
        }
        let font = self.theme.font * 0.87;
        let size = Vec2::new(self.text.width(text, font) + 20.0, font * LINE_HEIGHT + 12.0);
        let vp = self.viewport();
        let x = (r.x + (r.w - size.x) * 0.5).min(vp.x - size.x - 6.0).max(6.0);
        // Above the element, or below if there is no room.
        let y = if r.y - size.y - 8.0 >= 6.0 { r.y - size.y - 8.0 + (1.0 - p) * 5.0 } else { r.bottom() + 8.0 - (1.0 - p) * 5.0 };
        let below = self.draw.set_layer(2);
        let cull = self.draw.cull.take();
        let bubble = Rect::new(x, y, size.x, size.y);
        self.draw.rect(bubble, 9.0, self.pal[TEXT].fade(p));
        self.draw.add_text(Vec2::new(x + 10.0, y + 6.0), font, self.pal[BG].fade(p), text);
        self.draw.cull = cull;
        self.draw.set_layer(below);
    }

    /// Shows a brief notification in the corner. It leaves by itself after a
    /// few seconds, or when clicked; the rest of the stack closes the gap.
    pub fn toast(&mut self, text: &str) {
        self.toast_seq += 1;
        self.toasts.push(Toast { id: Id::new("morfo-toast").with(self.toast_seq), text: text.to_string(), until: self.now + 3.5 });
        self.animating = true;
    }

    /// Lays out and paints the toast stack; called when the frame ends.
    pub(crate) fn draw_toasts(&mut self) {
        if self.toasts.is_empty() {
            return;
        }
        let mut toasts = std::mem::take(&mut self.toasts);
        let vp = self.viewport();
        let (font, h, pad) = (self.theme.font * 0.93, self.theme.control, self.theme.pad);
        let (alpha, clip, cull) = (self.alpha, self.clip, self.draw.cull.take());
        self.clip = None;
        let below = self.draw.set_layer(2);
        let mut bottom = vp.y - 18.0;
        // Newest at the bottom; older ones stack upwards.
        for t in toasts.iter_mut().rev() {
            let alive = self.now < t.until;
            if alive {
                self.wake_at(t.until);
            }
            let p = self.presence_id(t.id.child("presence"), alive, Smooth);
            let pc = p.clamp(0.0, 1.0);
            let w = self.text.width(&t.text, font) + pad * 2.0;
            let (r, _) = self.enter(t.id, Rect::new(vp.x - w - 18.0, bottom - h, w, h), Smooth);
            bottom -= (h + 8.0) * pc;
            if !alive && p <= 0.001 {
                t.until = f64::NEG_INFINITY;
                continue;
            }
            let r = r.translate(Vec2::new((1.0 - p) * 48.0, 0.0));
            self.draw.push(DrawCmd::Shadow { rect: r, radius: h * 0.5, blur: 20.0, offset: Vec2::new(0.0, 8.0), color: self.pal[SHADOW].fade(pc) });
            self.draw.rect_stroked(r, h * 0.5, self.pal[RAISED].fade(pc), self.pal[BORDER].fade(pc), 1.0);
            self.draw.add_text(Vec2::new(r.x + pad, r.y + (h - font * LINE_HEIGHT) * 0.5), font, self.pal[TEXT].fade(pc), &t.text);
            if alive {
                self.blockers.push((2, r));
                if self.hit(t.id, r).clicked {
                    t.until = self.now;
                    self.animating = true;
                }
            }
        }
        toasts.retain(|t| t.until != f64::NEG_INFINITY);
        self.toasts = toasts;
        self.draw.set_layer(below);
        self.draw.cull = cull;
        self.alpha = alpha;
        self.clip = clip;
    }
}
