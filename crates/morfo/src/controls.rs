//! More controls: a two-thumb range slider, a rotary knob, a star rating,
//! color swatches and a two-dimensional pad.

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::fmt::Write;

use crate::a11y::Role;
use crate::anim::SpringKind::{Bouncy, Gentle, Smooth, Snappy};
use crate::buttons::{draw_icon, Icon};
use crate::draw::{DrawCmd, DrawList};
use crate::geom::{Color, Rect, Vec2};
use crate::theme::{ACCENT, BORDER, MUTED, RAISED, SURFACE, TEXT};
use crate::ui::{Key, Node, Ui, LINE_HEIGHT};

/// A round white thumb with a soft shadow, as sliders use.
fn thumb(list: &mut DrawList, c: Vec2, d: f32, a: f32) {
    let r = Rect::new(c.x - d * 0.5, c.y - d * 0.5, d, d);
    list.push(DrawCmd::Shadow { rect: r, radius: d * 0.5, blur: 8.0, offset: Vec2::new(0.0, 2.0), color: Color::BLACK.with_alpha(0.3 * a) });
    list.rect(r, d * 0.5, Color::WHITE.fade(a));
}

/// A five-pointed star filled with `color`, centered on `c` in a
/// `size`-pixel square (the same shape as [`Icon::Star`]'s outline).
fn filled_star(list: &mut DrawList, c: Vec2, size: f32, color: Color) {
    let k = size * 0.5;
    let pts: Vec<Vec2> = (0..10)
        .map(|i| {
            let r = if i % 2 == 0 { 0.88 } else { 0.38 };
            let a = i as f32 / 10.0 * TAU - FRAC_PI_2;
            c + Vec2::new(a.cos() * r * k, (a.sin() * r + 0.06) * k)
        })
        .collect();
    list.polygon(&pts, color);
}

impl Ui {
    /// A slider with two thumbs selecting a range within `0..=1`. Pressing
    /// the track takes the nearest thumb; neither can pass the other.
    /// Returns `true` while the user is changing it.
    pub fn range_slider(&mut self, key: &str, lo: &mut f32, hi: &mut f32, width: f32) -> bool {
        let id = self.declare(key);
        let h = (self.theme.control * 0.6).round();
        let target = self.alloc(Vec2::new(width, h));
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        let d = h * 0.7;
        let span = (r.w - d).max(1.0);
        if *lo > *hi {
            std::mem::swap(lo, hi);
        }
        let pointer = self.input.pointer;
        let n = self.nodes.get_mut(&id).unwrap();
        let mut changed = false;
        if let (true, Some(p)) = (hit.held, pointer) {
            let v = ((p.x - r.x - d * 0.5) / span).clamp(0.0, 1.0);
            if !n.latch {
                // Grab the thumb nearest to the press (the upper one on a tie
                // to the right of both).
                n.latch = true;
                n.open = (v - *hi).abs() < (v - *lo).abs() || v > *hi;
            }
            if n.open {
                let v = v.max(*lo);
                changed = *hi != v;
                *hi = v;
            } else {
                let v = v.min(*hi);
                changed = *lo != v;
                *lo = v;
            }
        } else {
            n.latch = false;
        }
        let upper = n.open;
        // Arrow keys move the thumb grabbed last.
        let step = self.arrows(id) * 0.05;
        if step != 0.0 {
            if upper {
                *hi = (*hi + step).clamp(*lo, 1.0);
            } else {
                *lo = (*lo + step).clamp(0.0, *hi);
            }
            changed = true;
        }
        if hit.released {
            self.learn(id, key);
        }
        if self.a11y_on {
            let value = format!("{:.0} % – {:.0} %", *lo * 100.0, *hi * 100.0);
            self.describe(id, Role::Slider, key, r, Some(&value), None);
        }
        let snappy = self.steps[Snappy as usize];
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        let anim = n.aux.drive(*lo, fresh, &snappy)
            | n.e1.drive(*hi, fresh, &snappy)
            | n.hover.drive((hit.hovered || hit.held) as u8 as f32, fresh, &snappy)
            | n.press.drive(hit.held as u8 as f32, fresh, &snappy);
        let (a_lo, a_hi, hover, press) = (n.aux.get(), n.e1.get(), n.hover.get(), n.press.get());
        self.animating |= anim;

        let a = self.alpha * life;
        let (x_lo, x_hi) = (r.x + d * 0.5 + span * a_lo, r.x + d * 0.5 + span * a_hi);
        let cy = r.center().y;
        let bar = 6.0 + 2.0 * hover;
        self.draw.rect(Rect::new(r.x, cy - bar * 0.5, r.w, bar), bar * 0.5, self.pal[BORDER].fade(a));
        self.draw.rect(Rect::new(x_lo, cy - bar * 0.5, (x_hi - x_lo).max(0.0), bar), bar * 0.5, self.pal[ACCENT].fade(a));
        for (x, is_upper) in [(x_lo, false), (x_hi, true)] {
            let grabbed = if is_upper == upper { press } else { 0.0 };
            thumb(&mut self.draw, Vec2::new(x, cy), d * (0.85 + 0.15 * hover + 0.12 * grabbed) * life, a);
        }
        changed
    }

    /// A rotary knob over `0..=1` with a caption below. Drag up or down, or
    /// use the wheel over it. Returns `true` while the user is changing it.
    pub fn knob(&mut self, key: &str, label: &str, value: &mut f32) -> bool {
        const START: f32 = 0.75 * PI;
        const SWEEP: f32 = 1.5 * PI;
        let id = self.declare(key);
        let d = (self.theme.control * 1.7).round();
        let font = self.theme.font * 0.85;
        let label_w = self.text.width(label, font);
        let target = self.alloc(Vec2::new(d.max(label_w), d + 4.0 + font * LINE_HEIGHT));
        let (r, life) = self.enter(id, target, Smooth);
        let dial = Rect::new(r.x + (r.w - d) * 0.5, r.y, d, d);
        let hit = self.hit(id, dial);
        let over = self.pointer_in(dial);
        let wheel = if over { std::mem::take(&mut self.input.wheel) } else { 0.0 };
        let pointer = self.input.pointer;
        let n = self.nodes.get_mut(&id).unwrap();
        let before = *value;
        if let (true, Some(p)) = (hit.held, pointer) {
            if !n.latch {
                n.latch = true;
                n.stamp = p.y as f64;
                n.e2.snap(*value);
            }
            // 160 px of travel cover the whole range.
            *value = (n.e2.get() + (n.stamp as f32 - p.y) / 160.0).clamp(0.0, 1.0);
        } else {
            n.latch = false;
        }
        if wheel != 0.0 {
            *value = (*value + wheel / 600.0).clamp(0.0, 1.0);
        }
        *value = (*value + self.arrows(id) * 0.05).clamp(0.0, 1.0);
        self.describe_level(id, Role::Slider, label, dial, *value);
        let changed = *value != before;
        if hit.released || wheel != 0.0 {
            self.learn(id, label);
        }
        let (snappy, smooth) = (self.steps[Snappy as usize], self.steps[Smooth as usize]);
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        let anim = n.aux.drive(*value, fresh, &smooth)
            | n.hover.drive((over || hit.held) as u8 as f32, fresh, &snappy)
            | n.press.drive(hit.held as u8 as f32, fresh, &snappy);
        let (t, hover, press) = (n.aux.get().clamp(0.0, 1.0), n.hover.get(), n.press.get());
        self.animating |= anim || wheel != 0.0;

        let a = self.alpha * life;
        let c = dial.center();
        let ring = d * 0.42;
        let w = 4.0 + 1.5 * hover;
        self.draw.arc(c, ring, START, START + SWEEP, w, self.pal[BORDER].fade(a));
        if t > 0.001 {
            self.draw.arc(c, ring, START, START + SWEEP * t, w, self.pal[ACCENT].fade(a));
        }
        let cap = d * (0.56 - 0.03 * press) * life;
        let cap_r = Rect::new(c.x - cap * 0.5, c.y - cap * 0.5, cap, cap);
        self.draw.push(DrawCmd::Shadow {
            rect: cap_r,
            radius: cap * 0.5,
            blur: 10.0,
            offset: Vec2::new(0.0, 3.0),
            color: Color::BLACK.with_alpha(0.25 * a),
        });
        self.draw.rect_stroked(cap_r, cap * 0.5, self.pal[SURFACE].mix(self.pal[RAISED], hover).fade(a), self.pal[BORDER].fade(a), 1.0);
        let ang = START + SWEEP * t;
        let dir = Vec2::new(ang.cos(), ang.sin());
        self.draw.line(c + dir * (cap * 0.12), c + dir * (cap * 0.36), 2.5, self.pal[TEXT].fade(a));
        let pos = Vec2::new(r.x + (r.w - label_w) * 0.5, dial.bottom() + 4.0);
        self.draw.add_text(pos, font, self.pal[MUTED].fade(a), label);
        changed
    }

    /// A rating of up to `max` stars. Hovering previews, clicking sets it, and
    /// clicking the current value clears it. Returns `true` when it changed.
    pub fn rating(&mut self, key: &str, value: &mut u8, max: u8) -> bool {
        let id = self.declare(key);
        let s = (self.theme.control * 0.8).round();
        let gap = 4.0;
        let count = max.max(1) as usize;
        let target = self.alloc(Vec2::new(count as f32 * s + (count - 1) as f32 * gap, s));
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        let under = self
            .input
            .pointer
            .filter(|_| hit.hovered || hit.held)
            .map(|p| (((p.x - r.x) / (s + gap)).floor().clamp(0.0, count as f32 - 1.0)) as u8 + 1);
        let mut changed = false;
        let step = self.arrows(id) as i32;
        if step != 0 {
            *value = (*value as i32 + step).clamp(0, count as i32) as u8;
            changed = true;
        }
        if self.a11y_on {
            let v = format!("{} / {}", *value, count);
            self.describe(id, Role::Slider, key, r, Some(&v), None);
        }
        if let (true, Some(n)) = (hit.clicked, under) {
            *value = if *value == n { 0 } else { n };
            changed = true;
            self.learn(id, key);
        }
        let shown = under.unwrap_or(*value);
        let (bouncy, snappy) = (self.steps[Bouncy as usize], self.steps[Snappy as usize]);
        let a = self.alpha * life;
        for i in 0..count {
            let star = id.with(i as u64);
            let n = self.nodes.entry(star).or_insert_with(Node::new);
            let fresh = n.touch(self.frame);
            let on = (i as u8) < shown;
            let anim = n.aux.drive(on as u8 as f32, fresh, &bouncy) | n.hover.drive((under == Some(i as u8 + 1)) as u8 as f32, fresh, &snappy);
            let (fill, pop) = (n.aux.get(), n.hover.get());
            self.animating |= anim;
            let c = Vec2::new(r.x + s * 0.5 + i as f32 * (s + gap), r.center().y);
            // Previewed stars are lighter than chosen ones.
            let preview = under.is_some() && (i as u8) >= *value;
            let accent = self.pal[ACCENT].with_alpha(if preview { 0.55 } else { 1.0 });
            let size = s * 0.82 * (1.0 + 0.16 * pop) * life;
            if fill > 0.02 {
                filled_star(&mut self.draw, c, size * fill.clamp(0.0, 1.2), accent.fade(a * fill.clamp(0.0, 1.0)));
            }
            draw_icon(&mut self.draw, Icon::Star, c, size, self.pal[MUTED].mix(accent, fill.clamp(0.0, 1.0)).fade(a));
        }
        changed
    }

    /// A row of color swatches; the selection ring springs between them.
    /// Returns `true` when the selection changed.
    pub fn swatches(&mut self, key: &str, colors: &[Color], selected: &mut usize) -> bool {
        let id = self.declare(key);
        let d = (self.theme.control * 0.72).round();
        let gap = 10.0;
        let count = colors.len();
        let target = self.alloc(Vec2::new(count as f32 * d + count.saturating_sub(1) as f32 * gap + 8.0, d + 8.0));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        let mut changed = false;
        let mut ring = None;
        let snappy = self.steps[Snappy as usize];
        for (i, &color) in colors.iter().enumerate() {
            let cell = Rect::new(r.x + 4.0 + i as f32 * (d + gap), r.y + 4.0, d, d);
            let sid = id.with(i as u64);
            let hit = self.hit(sid, cell);
            self.describe(sid, Role::RadioButton, key, cell, None, Some(*selected == i));
            if self.focus == Some(sid) && self.arrows(sid) != 0.0 {
                // Arrows move the selection, and the focus with it.
                let to = (*selected as i32 + self.arrows(sid) as i32).clamp(0, colors.len() as i32 - 1) as usize;
                if to != *selected {
                    *selected = to;
                    changed = true;
                    self.focus = Some(id.with(to as u64));
                }
            }
            if hit.clicked && *selected != i {
                *selected = i;
                changed = true;
                self.learn(sid, key);
            }
            if *selected == i {
                ring = Some(cell);
            }
            let n = self.nodes.entry(sid).or_insert_with(Node::new);
            let fresh = n.touch(self.frame);
            n.rect.snap(cell);
            let anim = n.hover.drive(hit.hovered as u8 as f32, fresh, &snappy) | n.press.drive(hit.held as u8 as f32, fresh, &snappy);
            let (hover, press) = (n.hover.get(), n.press.get());
            self.animating |= anim;
            let c = cell.scaled((1.0 + 0.1 * hover - 0.1 * press) * life);
            self.draw.rect(c, d, color.fade(a));
        }
        if let Some(cell) = ring {
            let (bouncy, gentle) = (self.steps[Bouncy as usize], self.steps[Gentle as usize]);
            let n = self.nodes.entry(id.child("ring")).or_insert_with(Node::new);
            n.reframe(self.frame, self.shift);
            let anim = n.begin(self.frame, cell.inset(-4.0), &bouncy, &gentle);
            let rr = n.rect.get();
            self.animating |= anim;
            self.draw.rect_stroked(rr, rr.h, Color::TRANSPARENT, self.pal[TEXT].with_alpha(0.85 * a), 2.0);
        }
        changed
    }

    /// A two-dimensional pad: drag the point anywhere inside it. `value` is
    /// in `0..=1` on both axes, `(0, 0)` at the top left. Returns `true`
    /// while the user is changing it.
    pub fn xy_pad(&mut self, key: &str, value: &mut Vec2, size: Vec2) -> bool {
        let id = self.declare(key);
        let target = self.alloc(size);
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        let mut changed = false;
        if let (true, Some(p)) = (hit.held, self.input.pointer) {
            let v = Vec2::new(((p.x - r.x) / r.w.max(1.0)).clamp(0.0, 1.0), ((p.y - r.y) / r.h.max(1.0)).clamp(0.0, 1.0));
            changed = v != *value;
            *value = v;
        }
        if self.focus == Some(id) {
            for k in &self.keys {
                let d = match k {
                    Key::Left => Vec2::new(-0.05, 0.0),
                    Key::Right => Vec2::new(0.05, 0.0),
                    Key::Up => Vec2::new(0.0, -0.05),
                    Key::Down => Vec2::new(0.0, 0.05),
                    _ => continue,
                };
                *value = Vec2::new((value.x + d.x).clamp(0.0, 1.0), (value.y + d.y).clamp(0.0, 1.0));
                changed = true;
            }
        }
        if hit.released {
            self.learn(id, key);
        }
        if self.a11y_on {
            let v = format!("x {:.2}, y {:.2}", value.x, value.y);
            self.describe(id, Role::Slider, key, r, Some(&v), None);
        }
        let snappy = self.steps[Snappy as usize];
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        let anim = n.aux.drive(value.x, fresh, &snappy)
            | n.e1.drive(value.y, fresh, &snappy)
            | n.hover.drive((hit.hovered || hit.held) as u8 as f32, fresh, &snappy)
            | n.press.drive(hit.held as u8 as f32, fresh, &snappy);
        let (x, y, hover, press) = (n.aux.get(), n.e1.get(), n.hover.get(), n.press.get());
        self.animating |= anim;

        let a = self.alpha * life;
        let radius = self.theme.radius * 0.8;
        self.draw.rect_stroked(r, radius, self.pal[RAISED].fade(a), self.pal[BORDER].mix(self.pal[ACCENT], 0.5 * hover).fade(a), 1.0);
        let grid = self.pal[BORDER].with_alpha(0.7 * a);
        for q in [0.25, 0.5, 0.75] {
            self.draw.line(Vec2::new(r.x + r.w * q, r.y + 6.0), Vec2::new(r.x + r.w * q, r.bottom() - 6.0), 1.0, grid);
            self.draw.line(Vec2::new(r.x + 6.0, r.y + r.h * q), Vec2::new(r.right() - 6.0, r.y + r.h * q), 1.0, grid);
        }
        let c = Vec2::new(r.x + r.w * x, r.y + r.h * y);
        let cross = self.pal[ACCENT].with_alpha((0.35 + 0.35 * hover) * a);
        self.draw.line(Vec2::new(c.x, r.y + 4.0), Vec2::new(c.x, r.bottom() - 4.0), 1.5, cross);
        self.draw.line(Vec2::new(r.x + 4.0, c.y), Vec2::new(r.right() - 4.0, c.y), 1.5, cross);
        let dot = 18.0 * (1.0 + 0.15 * hover - 0.1 * press) * life;
        let ring = Rect::new(c.x - dot * 0.5 - 3.0, c.y - dot * 0.5 - 3.0, dot + 6.0, dot + 6.0);
        self.draw.rect(ring, ring.w, self.pal[ACCENT].with_alpha(0.35 * a));
        thumb(&mut self.draw, c, dot, a);
        self.scratch.clear();
        let _ = write!(self.scratch, "x {:.2}  ·  y {:.2}", value.x, value.y);
        let font = self.theme.font * 0.8;
        let pos = Vec2::new(r.x + 10.0, r.bottom() - font * LINE_HEIGHT - 6.0);
        self.draw.add_text(pos, font, self.pal[MUTED].fade(a), &self.scratch);
        changed
    }
}
