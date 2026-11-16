//! Keyboard text entry and scrolling.

use crate::anim::SpringKind::{Smooth, Snappy};
use crate::draw::DrawCmd;
use crate::geom::{Rect, Vec2};
use crate::theme::{ACCENT, BORDER, MUTED, RAISED, TEXT};
use crate::ui::{Dir, Key, Ui, LINE_HEIGHT};

/// What happened to a text field this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextEdit {
    /// The text was modified.
    pub changed: bool,
    /// Enter was pressed while it had the focus.
    pub submitted: bool,
    pub focused: bool,
}

impl Ui {
    /// Gives the keyboard focus to the text field `key` (declared in this
    /// frame or the next one), with the caret at the end of its text.
    pub fn request_focus(&mut self, key: &str) {
        self.focus = Some(self.id(key));
        self.cursor = usize::MAX;
        self.focus_claimed = true;
    }

    /// Single-line text field. Click to focus (the caret lands where you
    /// click), Tab moves to the next field, Escape leaves it.
    pub fn text_field(&mut self, key: &str, value: &mut String, placeholder: &str, width: f32) -> TextEdit {
        let id = self.id(key);
        let (font, pad) = (self.theme.font, self.theme.pad * 0.7);
        let target = self.alloc(Vec2::new(width, self.theme.control));
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        self.focusables.push(id);
        let inner = (r.w - pad * 2.0).max(1.0);
        // Long text scrolls so the caret stays in view.
        let scroll_for = |caret: f32| (caret - inner + 2.0).max(0.0);
        let mut out = TextEdit::default();
        let mut touched = false;

        if self.input.pressed && self.active == Some(id) {
            let was_focused = self.focus == Some(id);
            self.focus = Some(id);
            self.focus_claimed = true;
            touched = true;
            if let Some(p) = self.input.pointer {
                let old = if was_focused { self.text.width(&value[..floor_boundary(value, self.cursor)], font) } else { 0.0 };
                let x = p.x - r.x - pad + scroll_for(old);
                let mut best = (f32::MAX, 0);
                for i in value.char_indices().map(|(i, _)| i).chain([value.len()]) {
                    let d = (self.text.width(&value[..i], font) - x).abs();
                    if d < best.0 {
                        best = (d, i);
                    }
                }
                self.cursor = best.1;
            }
        }

        let mut focused = self.focus == Some(id);
        if focused {
            let mut at = floor_boundary(value, self.cursor);
            if !self.typed.is_empty() {
                value.insert_str(at, &self.typed);
                at += self.typed.len();
                out.changed = true;
                touched = true;
            }
            for i in 0..self.keys.len() {
                touched = true;
                let before = value[..at].char_indices().next_back().map(|(j, _)| j);
                let after = value[at..].chars().next().map(|c| at + c.len_utf8());
                match self.keys[i] {
                    Key::Backspace => {
                        if let Some(j) = before {
                            value.replace_range(j..at, "");
                            at = j;
                            out.changed = true;
                        }
                    }
                    Key::Delete => {
                        if let Some(j) = after {
                            value.replace_range(at..j, "");
                            out.changed = true;
                        }
                    }
                    Key::Left => at = before.unwrap_or(at),
                    Key::Right => at = after.unwrap_or(at),
                    Key::Home => at = 0,
                    Key::End => at = value.len(),
                    Key::Enter => out.submitted = true,
                    Key::Escape => {
                        self.focus = None;
                        focused = false;
                    }
                    Key::Tab => {}
                }
            }
            self.cursor = at;
        }
        out.focused = focused;

        let caret = if focused { self.text.width(&value[..self.cursor.min(value.len())], font) } else { 0.0 };
        let scroll = scroll_for(caret);
        let (now, accent) = (self.now, self.pal[ACCENT]);
        let (snappy, smooth) = (self.steps[Snappy as usize], self.steps[Smooth as usize]);
        let idle_stroke = self.pal[BORDER].mix(self.pal[TEXT], if hit.hovered { 0.3 } else { 0.0 });
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        if touched {
            n.stamp = now;
        }
        let anim = n.stroke.drive(if focused { accent } else { idle_stroke }, fresh, &smooth)
            | n.press.drive(focused as u8 as f32, fresh, &smooth)
            | n.aux.drive(caret - scroll, fresh || !focused, &snappy);
        let (stroke, focus_t, caret_x, since) = (n.stroke.get(), n.press.get().clamp(0.0, 1.0), n.aux.get(), now - n.stamp);
        self.animating |= anim;

        let a = self.alpha * life;
        self.draw.rect_stroked(r, self.theme.radius * 0.7, self.pal[RAISED].fade(a), stroke.fade(a), 1.0 + 0.6 * focus_t);
        let y = r.y + (r.h - font * LINE_HEIGHT) * 0.5;
        self.draw.push(DrawCmd::PushClip { rect: Rect::new(r.x + pad - 1.0, r.y, inner + 2.0, r.h), radius: 0.0 });
        if value.is_empty() {
            self.draw.add_text(Vec2::new(r.x + pad, y), font, self.pal[MUTED].fade(a), placeholder);
        } else {
            self.draw.add_text(Vec2::new(r.x + pad - scroll, y), font, self.pal[TEXT].fade(a), value);
        }
        if focused {
            // The caret glides to its place, and blinks while idle.
            let phase = since.rem_euclid(1.0);
            if phase < 0.55 {
                let x = r.x + pad + caret_x;
                let half = font * 0.62;
                self.draw.line(Vec2::new(x, r.center().y - half), Vec2::new(x, r.center().y + half), 1.6, accent.fade(a));
            }
            self.wake_at(now + if phase < 0.55 { 0.55 - phase } else { 1.0 - phase });
        }
        self.draw.push(DrawCmd::PopClip);
        out
    }

    /// A viewport of fixed `height` over content of any length. The wheel
    /// scrolls it with inertia; content outside it is not drawn.
    pub fn scroll(&mut self, key: &str, height: f32, f: impl FnOnce(&mut Ui)) {
        let id = self.id(key);
        let target = self.alloc(Vec2::new(self.available_width(), height));
        let (r, life) = self.enter(id, target, Smooth);
        let content = self.sizes.get(&id).map_or(0.0, |s| s.y);
        let max = (content - height).max(0.0);
        let over = self.pointer_in(r);
        let k = self.steps[Snappy as usize];
        let n = self.nodes.get_mut(&id).unwrap();
        n.aux.set(n.aux.target().clamp(0.0, max));
        let anim = n.aux.tick(&k) | n.hover.drive(over as u8 as f32, false, &k);
        let (offset, hover) = (n.aux.get(), n.hover.get());
        self.animating |= anim;

        let (clip, cull) = (self.clip, self.draw.cull);
        let view = clip.map_or(r, |c| c.intersect(&r));
        self.clip = Some(view);
        self.draw.cull = Some(view);
        self.draw.push(DrawCmd::PushClip { rect: r, radius: 0.0 });
        let gap = self.theme.gap;
        self.group(id, Dir::Column, Vec2::ZERO, gap, Some(Vec2::new(target.x, target.y - offset)), f);
        self.draw.push(DrawCmd::PopClip);
        self.clip = clip;
        self.draw.cull = cull;

        // The wheel is read after the content, so a scroll area nested
        // inside this one gets it first. Steps add up on the target; the
        // spring smooths the travel.
        if over && self.input.wheel != 0.0 {
            let wheel = std::mem::take(&mut self.input.wheel);
            let n = self.nodes.get_mut(&id).unwrap();
            n.aux.set((n.aux.target() - wheel).clamp(0.0, max));
            self.animating = true;
        }

        if max > 0.0 {
            let thumb = (height * height / content).max(28.0);
            let y = r.y + (height - thumb) * (offset / max).clamp(0.0, 1.0);
            let bar = Rect::new(r.right() - 5.0, y, 3.0 + 2.0 * hover, thumb);
            self.draw.rect(bar, 3.0, self.pal[MUTED].with_alpha((0.25 + 0.45 * hover) * self.alpha * life));
        }
    }
}

/// Largest char boundary of `s` that is `<= i`.
fn floor_boundary(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
