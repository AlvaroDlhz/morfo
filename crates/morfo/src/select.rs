//! Selection controls: check boxes, radio buttons, chips, steppers and
//! accordions.

use std::fmt::Write;

use crate::anim::SpringKind::{Bouncy, Smooth, Snappy};
use crate::draw::DrawCmd;
use crate::geom::{Rect, Vec2};
use crate::theme::{ACCENT, BORDER, MUTED, ON_ACCENT, RAISED, TEXT};
use crate::ui::{Ui, LINE_HEIGHT};
use crate::widgets::{Size, Variant};

const BOX: f32 = 20.0;

impl Ui {
    /// Check box with its label. The mark is drawn stroke by stroke.
    /// Returns `true` when the user toggled it.
    pub fn checkbox(&mut self, key: &str, label: &str, value: &mut bool) -> bool {
        let id = self.id(key);
        let font = self.theme.font;
        let target = self.alloc(Vec2::new(BOX + 10.0 + self.text.width(label, font), (font * LINE_HEIGHT).max(BOX)));
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        if hit.clicked {
            *value = !*value;
            self.profile.record(id, self.now);
        }
        let (hover, press, t) = self.drive3(id, hit.hovered, hit.held, *value as u8 as f32, Snappy);
        let (a, t) = (self.alpha * life, t.clamp(0.0, 1.0));
        let b = Rect::new(r.x, r.y + (r.h - BOX) * 0.5, BOX, BOX).scaled(1.0 - 0.1 * press);
        let accent = self.pal[ACCENT];
        let stroke = self.pal[BORDER].mix(self.pal[TEXT], 0.25 + 0.35 * hover).mix(accent, t);
        self.draw.rect_stroked(b, 6.0, accent.with_alpha(t * a), stroke.fade(a), 1.5);
        if t > 0.02 {
            let at = |x: f32, y: f32| Vec2::new(b.x + b.w * x, b.y + b.h * y);
            let (p0, p1, p2) = (at(0.25, 0.53), at(0.43, 0.70), at(0.76, 0.33));
            let ink = self.pal[ON_ACCENT].fade(a);
            self.draw.line(p0, p0 + (p1 - p0) * (t / 0.4).min(1.0), 2.0, ink);
            if t > 0.4 {
                self.draw.line(p1, p1 + (p2 - p1) * ((t - 0.4) / 0.6), 2.0, ink);
            }
        }
        let pos = Vec2::new(r.x + BOX + 10.0, r.y + (r.h - font * LINE_HEIGHT) * 0.5);
        self.draw.add_text(pos, font, self.pal[TEXT].fade(a), label);
        hit.clicked
    }

    /// One option of an exclusive group: selected while `*value == this`.
    /// Returns `true` when the user picked it.
    pub fn radio(&mut self, key: &str, label: &str, value: &mut usize, this: usize) -> bool {
        let id = self.id(key);
        let font = self.theme.font;
        let target = self.alloc(Vec2::new(BOX + 10.0 + self.text.width(label, font), (font * LINE_HEIGHT).max(BOX)));
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        let picked = hit.clicked && *value != this;
        if picked {
            *value = this;
            self.profile.record(id, self.now);
        }
        let (hover, press, t) = self.drive3(id, hit.hovered, hit.held, (*value == this) as u8 as f32, Bouncy);
        let a = self.alpha * life;
        let b = Rect::new(r.x, r.y + (r.h - BOX) * 0.5, BOX, BOX).scaled(1.0 - 0.1 * press);
        let accent = self.pal[ACCENT];
        let stroke = self.pal[BORDER].mix(self.pal[TEXT], 0.25 + 0.35 * hover).mix(accent, t.clamp(0.0, 1.0));
        self.draw.rect_stroked(b, BOX, self.pal[TEXT].with_alpha(0.0), stroke.fade(a), 1.5);
        self.draw.rect(b.scaled(0.5 * t.max(0.0)), BOX, accent.fade(a));
        let pos = Vec2::new(r.x + BOX + 10.0, r.y + (r.h - font * LINE_HEIGHT) * 0.5);
        self.draw.add_text(pos, font, self.pal[TEXT].fade(a), label);
        picked
    }

    /// A small pill that stays on or off (filters, tags). Returns `true`
    /// when the user toggled it.
    pub fn chip(&mut self, key: &str, label: &str, value: &mut bool) -> bool {
        let id = self.id(key);
        let variant = if *value { Variant::Soft } else { Variant::Outline };
        let clicked = self.button_id(id, label, variant, Size::Sm).clicked;
        if clicked {
            *value = !*value;
        }
        clicked
    }

    /// Whole number with − and + buttons, kept within `min..=max`. The
    /// digits roll like an odometer. Returns `true` when the value changed.
    pub fn stepper(&mut self, key: &str, value: &mut i32, min: i32, max: i32) -> bool {
        let id = self.id(key);
        let h = (self.theme.control * 0.9).round();
        let value_w = 52.0;
        let target = self.alloc(Vec2::new(h * 2.0 + value_w, h));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        self.draw.rect(r, h * 0.5, self.pal[RAISED].fade(a));
        let mut changed = false;
        for (i, delta) in [(0u64, -1), (1, 1)] {
            let button = id.with(i);
            let area = Rect::new(if i == 0 { r.x } else { r.right() - h }, r.y, h, h);
            let enabled = if delta < 0 { *value > min } else { *value < max };
            self.enter(button, area, Snappy);
            let hit = self.hit(button, area);
            if hit.clicked && enabled {
                *value += delta;
                changed = true;
                self.profile.record(id, self.now);
            }
            let (hover, press, _) = self.drive3(button, hit.hovered && enabled, hit.held && enabled, 0.0, Snappy);
            self.draw.rect(area.inset(4.0).scaled(1.0 - 0.12 * press), h, self.pal[TEXT].with_alpha(0.1 * hover * a));
            let (c, ink) = (area.center(), self.pal[TEXT].fade(a * if enabled { 1.0 } else { 0.3 }));
            self.draw.line(c - Vec2::new(5.0, 0.0), c + Vec2::new(5.0, 0.0), 2.0, ink);
            if delta > 0 {
                self.draw.line(c - Vec2::new(0.0, 5.0), c + Vec2::new(0.0, 5.0), 2.0, ink);
            }
        }
        // The shown value chases the real one; in between, both neighbours
        // are visible, sliding past.
        let (_, _, v) = self.drive3(id, false, false, *value as f32, Snappy);
        let (low, frac) = (v.floor(), v - v.floor());
        let font = self.theme.font;
        let window = Rect::new(r.x + h, r.y, value_w, h);
        self.draw.push(DrawCmd::PushClip { rect: window, radius: 0.0 });
        for (n, slide, alpha) in [(low as i32, -frac, 1.0 - frac), (low as i32 + 1, 1.0 - frac, frac)] {
            if alpha < 0.01 {
                continue;
            }
            self.scratch.clear();
            let _ = write!(self.scratch, "{n}");
            let w = self.text.width(&self.scratch, font);
            let pos = Vec2::new(window.x + (value_w - w) * 0.5, window.y + (h - font * LINE_HEIGHT) * 0.5 + slide * h * 0.7);
            self.draw.add_text(pos, font, self.pal[TEXT].fade(a * alpha), &self.scratch);
        }
        self.draw.push(DrawCmd::PopClip);
        changed
    }

    /// Stacked sections of which at most one is open. `f(ui, i)` builds the
    /// body of section `i` (only while it is visible).
    pub fn accordion(&mut self, key: &str, titles: &[&str], open: &mut Option<usize>, mut f: impl FnMut(&mut Ui, usize)) {
        let id = self.id(key);
        let font = self.theme.font;
        for (i, title) in titles.iter().enumerate() {
            let header = id.with(i as u64);
            let target = self.alloc(Vec2::new(self.available_width(), self.theme.control));
            let (r, life) = self.enter(header, target, Smooth);
            let hit = self.hit(header, r);
            if hit.clicked {
                *open = if *open == Some(i) { None } else { Some(i) };
                self.profile.record(header, self.now);
            }
            let is_open = *open == Some(i);
            let (hover, _, t) = self.drive3(header, hit.hovered, hit.held, is_open as u8 as f32, Smooth);
            let a = self.alpha * life;
            self.draw.rect(r, 10.0, self.pal[TEXT].with_alpha(0.07 * hover * a));
            let ink = self.pal[MUTED].mix(self.pal[TEXT], (t + hover).clamp(0.0, 1.0));
            self.draw.add_text(Vec2::new(r.x + 12.0, r.y + (r.h - font * LINE_HEIGHT) * 0.5), font, ink.fade(a), title);
            // Chevron: ">" when closed turns into "v" when open.
            let c = Vec2::new(r.right() - 20.0, r.center().y);
            let turn = |closed: (f32, f32), opened: (f32, f32)| {
                c + Vec2::new(closed.0 + (opened.0 - closed.0) * t, closed.1 + (opened.1 - closed.1) * t)
            };
            let (p0, p1, p2) = (turn((-2.5, -5.0), (-5.0, -2.5)), turn((2.5, 0.0), (0.0, 2.5)), turn((-2.5, 5.0), (5.0, -2.5)));
            self.draw.line(p0, p1, 2.0, ink.fade(a));
            self.draw.line(p1, p2, 2.0, ink.fade(a));
            self.reveal_id(header.child("body"), is_open, |ui| {
                ui.row(|ui| {
                    ui.space(12.0);
                    ui.column(|ui| f(ui, i));
                });
            });
        }
    }
}
