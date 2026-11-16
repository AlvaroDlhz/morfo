//! Telling people how things stand: placeholders while content loads,
//! count badges, and the steps of a multi-step process.

use crate::a11y::Role;
use crate::anim::SpringKind::{Bouncy, Smooth, Snappy};
use crate::buttons::{draw_icon, Icon};
use crate::charts::DOWN;
use crate::draw::DrawCmd;
use crate::geom::{Color, Rect, Vec2};
use crate::id::Id;
use crate::theme::{ACCENT, BORDER, MUTED, ON_ACCENT, RAISED, SURFACE, TEXT};
use crate::ui::{Node, Ui, LINE_HEIGHT};

/// Widths of the placeholder lines, as fractions of the space available.
const WIDTHS: [f32; 6] = [0.42, 1.0, 0.92, 0.97, 0.7, 0.84];

/// `count` as a badge shows it: `99+` past 99.
fn count_text(count: u32) -> String {
    if count > 99 {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

impl Ui {
    /// While `loading`, grey placeholder lines with a moving shimmer stand
    /// where the content will be (`lines` of them, a heading first); once
    /// loaded, the placeholder transforms into the content `f` builds.
    /// Keys declared in `f` are namespaced under `key`.
    pub fn skeleton(&mut self, key: &str, loading: bool, lines: usize, f: impl FnOnce(&mut Ui)) {
        self.morph(key, loading, |ui| {
            if loading {
                ui.skeleton_lines(lines);
            } else {
                f(ui);
            }
        });
    }

    fn skeleton_lines(&mut self, lines: usize) {
        let avail = self.available_width();
        let font = self.theme.font;
        let now = self.now;
        // One shimmer crosses every line at the same time, left to right.
        let phase = (now * 0.75).fract() as f32;
        let band = 120.0;
        let origin = self.peek(Vec2::ZERO).x;
        let sweep = origin - band + (avail + band * 2.0) * phase;
        for i in 0..lines {
            let id = self.auto_id();
            let heading = i == 0;
            let h = if heading { font * 1.3 } else { font * 0.85 };
            let w = (avail * WIDTHS[i % WIDTHS.len()]).max(24.0);
            let target = self.alloc(Vec2::new(w, h));
            let (r, life) = self.enter(id, target, Smooth);
            let a = self.alpha * life;
            let base = self.pal[TEXT].with_alpha(if heading { 0.12 } else { 0.08 });
            self.draw.rect(r, h * 0.5, base.fade(a));
            self.draw.push(DrawCmd::PushClip { rect: r, radius: h * 0.5 });
            // A soft band: three steps of light.
            for (k, alpha) in [(-1.0f32, 0.035), (0.0, 0.07), (1.0, 0.035)] {
                let x = sweep + k * band / 3.0;
                self.draw.rect(Rect::new(x, r.y, band / 3.0 + 1.0, r.h), 0.0, self.pal[TEXT].with_alpha(alpha * a));
            }
            self.draw.push(DrawCmd::PopClip);
        }
        self.animating = true;
    }

    /// A count in a small pill, inline. It pops when the number changes and
    /// shrinks away at zero.
    pub fn badge(&mut self, key: &str, count: u32) {
        let id = self.declare(key);
        let font = self.theme.font * 0.8;
        let text = count_text(count);
        let h = (font * 1.55).round();
        let w = (self.text.width(&text, font) + 12.0).max(h);
        let target = self.alloc(Vec2::new(w, h));
        let (r, life) = self.enter(id, target, Smooth);
        self.describe(id, Role::Label, key, r, Some(&text), None);
        let a = self.alpha * life;
        self.badge_draw(id, r.center(), count, font, a);
    }

    /// A count badge pinned to the top-right corner of the element `target`
    /// (declared before it in this frame): unread messages on an icon. It
    /// pops when the number changes and shrinks away at zero.
    pub fn badge_on(&mut self, target: &str, count: u32) {
        let tid = self.id(target);
        let Some(r) = self.rect_of(tid) else { return };
        let font = self.theme.font * 0.75;
        let id = tid.child("morfo-badge");
        let corner = Vec2::new(r.right() - 6.0, r.y + 6.0);
        let a = self.alpha;
        self.badge_draw(id, corner, count, font, a);
    }

    fn badge_draw(&mut self, id: Id, center: Vec2, count: u32, font: f32, a: f32) {
        let (bouncy, frame) = (self.steps[Bouncy as usize], self.frame);
        let n = self.nodes.entry(id.child("pop")).or_insert_with(Node::new);
        let fresh = n.touch(frame);
        if !fresh && n.index != count as usize {
            // A new number: a little jump.
            n.press.snap(1.0);
            n.press.set(0.0);
        }
        n.index = count as usize;
        let anim = n.press.tick(&bouncy) | n.aux.drive((count > 0) as u8 as f32, fresh, &bouncy);
        let (pop, shown) = (n.press.get(), n.aux.get());
        self.animating |= anim;
        if shown <= 0.01 {
            return;
        }
        // At zero it keeps showing the last number while it shrinks.
        let shown_count = if count > 0 { count } else { n.e1.get() as u32 };
        if count > 0 {
            n.e1.snap(count as f32);
        }
        let text = count_text(shown_count);
        let h = (font * 1.55).round();
        let w = (self.text.width(&text, font) + 12.0).max(h);
        let s = shown.max(0.0) * (1.0 + 0.3 * pop);
        let r = Rect::new(center.x - w * 0.5, center.y - h * 0.5, w, h).scaled(s);
        let ring = r.inset(-2.0);
        self.draw.rect(ring, ring.h, self.pal[SURFACE].fade(a));
        self.draw.rect(r, r.h, Color::hex(DOWN).fade(a));
        let size = font * s.min(1.2);
        let tw = self.text.width(&text, size);
        self.draw.add_text(Vec2::new(r.center().x - tw * 0.5, r.center().y - size * LINE_HEIGHT * 0.5), size, Color::WHITE.fade(a), &text);
    }

    /// The steps of a multi-step process, as numbered circles on a line that
    /// fills up to the current one: done steps show a check (click one to go
    /// back to it), the current one is highlighted. Returns `true` when a
    /// click changed `current`.
    pub fn steps(&mut self, key: &str, labels: &[&str], current: &mut usize) -> bool {
        const DOT: f32 = 30.0;
        let id = self.declare(key);
        let count = labels.len();
        if count == 0 {
            return false;
        }
        *current = (*current).min(count - 1);
        let font = self.theme.font * 0.85;
        let width = self.available_width();
        let target = self.alloc(Vec2::new(width, DOT + 10.0 + font * LINE_HEIGHT));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        let slot = r.w / count as f32;
        let center = |i: usize| Vec2::new(r.x + slot * (i as f32 + 0.5), r.y + DOT * 0.5);
        // The line, and its filled part up to the current step.
        let smooth = self.steps[Smooth as usize];
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        self.animating |= n.aux.drive(*current as f32, fresh, &smooth);
        let progress = n.aux.get();
        let (first, last) = (center(0), center(count - 1));
        self.draw.line(first, last, 3.0, self.pal[BORDER].fade(a));
        let reach = first.x + (last.x - first.x) * (progress / (count - 1).max(1) as f32).clamp(0.0, 1.0);
        if reach > first.x + 0.5 {
            self.draw.line(first, Vec2::new(reach, first.y), 3.0, self.pal[ACCENT].fade(a));
        }
        let mut changed = false;
        let snappy = self.steps[Snappy as usize];
        for (i, label) in labels.iter().enumerate() {
            let c = center(i);
            let sid = id.with(i as u64);
            let dot = Rect::new(c.x - DOT * 0.5, c.y - DOT * 0.5, DOT, DOT);
            let area = Rect::new(r.x + slot * i as f32, r.y, slot, r.h);
            let done = i < *current;
            self.enter(sid, area, Snappy);
            let hit = if done { Some(self.hit(sid, area)) } else { None };
            let state = if i == *current {
                "actual"
            } else if done {
                "hecho"
            } else {
                "pendiente"
            };
            self.describe(sid, Role::ListItem, label, area, Some(state), Some(done));
            if hit.as_ref().is_some_and(|h| h.clicked) {
                *current = i;
                changed = true;
                self.learn(sid, label);
            }
            let hovered = hit.as_ref().is_some_and(|h| h.hovered);
            let n = self.nodes.get_mut(&sid).unwrap();
            let fresh = n.fresh();
            // 0 waiting, 1 current, 2 done.
            let level = if i == *current {
                1.0
            } else if done {
                2.0
            } else {
                0.0
            };
            let anim = n.aux.drive(level, fresh, &snappy) | n.hover.drive(hovered as u8 as f32, fresh, &snappy);
            let (lv, hover) = (n.aux.get(), n.hover.get());
            self.animating |= anim;
            let cur = (1.0 - (lv - 1.0).abs()).clamp(0.0, 1.0);
            let reached = lv.clamp(0.0, 1.0);
            if cur > 0.01 {
                self.draw.rect(dot.inset(-5.0 * cur), DOT, self.pal[ACCENT].with_alpha(0.22 * cur * a));
            }
            let fill = self.pal[RAISED].mix(self.pal[ACCENT], reached).mix(Color::WHITE, 0.12 * hover);
            let stroke = self.pal[BORDER].mix(self.pal[ACCENT], reached);
            self.draw.rect_stroked(dot.scaled(1.0 + 0.06 * hover), DOT, fill.fade(a), stroke.fade(a), 1.5);
            let check = (lv - 1.0).clamp(0.0, 1.0);
            if check > 0.02 {
                draw_icon(&mut self.draw, Icon::Check, c, DOT * 0.45 * check, self.pal[ON_ACCENT].fade(a * check));
            }
            if check < 0.98 {
                let num = (i + 1).to_string();
                let tw = self.text.width(&num, font);
                let ink = self.pal[MUTED].mix(self.pal[ON_ACCENT], reached);
                self.draw.add_text(Vec2::new(c.x - tw * 0.5, c.y - font * LINE_HEIGHT * 0.5), font, ink.fade(a * (1.0 - check)), &num);
            }
            let tw = self.text.width(label, font);
            let ink = self.pal[MUTED].mix(self.pal[TEXT], reached.max(hover));
            self.draw.add_text(Vec2::new(c.x - tw * 0.5, dot.bottom() + 8.0), font, ink.fade(a), label);
        }
        changed
    }
}
