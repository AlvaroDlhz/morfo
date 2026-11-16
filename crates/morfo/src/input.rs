//! Keyboard text entry and scrolling.

use crate::a11y::Role;
use crate::anim::SpringKind::{Smooth, Snappy};
use crate::draw::DrawCmd;
use crate::edit::{edit_text, selection, EditState};
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
        self.anchor = None;
        self.focus_claimed = true;
    }

    /// Single-line text field. Click to focus (the caret lands where you
    /// click), drag or use Shift with the arrows to select, Ctrl+A/C/X/V
    /// (as [`Key`]s and [`Ui::paste`]) for the clipboard, Tab moves to the
    /// next field, Escape leaves it. Text being composed with an input
    /// method is shown underlined at the caret.
    pub fn text_field(&mut self, key: &str, value: &mut String, placeholder: &str, width: f32) -> TextEdit {
        let id = self.declare(key);
        self.text_field_id(id, value, placeholder, width, true)
    }

    /// [`text_field`](Self::text_field) with a given id; without `framed`
    /// it draws no box of its own (it lives inside another component's).
    pub(crate) fn text_field_id(&mut self, id: crate::id::Id, value: &mut String, placeholder: &str, width: f32, framed: bool) -> TextEdit {
        let (font, pad) = (self.theme.font, if framed { self.theme.pad * 0.7 } else { 2.0 });
        let target = self.alloc(Vec2::new(width, self.theme.control));
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        self.text_ids.push(id);
        let inner = (r.w - pad * 2.0).max(1.0);
        // Long text scrolls so the caret stays in view.
        let scroll_for = |caret: f32| (caret - inner + 2.0).max(0.0);
        let mut out = TextEdit::default();
        let mut touched = false;

        // Pointer: press places the caret (Shift extends), dragging selects.
        let dragging = self.active == Some(id) && self.input.down && self.focus == Some(id);
        if dragging {
            if let Some(p) = self.input.pointer {
                let was_focused = !self.focus_changed;
                let current = if was_focused { floor_boundary(value, self.cursor) } else { 0 };
                let scroll = if was_focused { scroll_for(self.text.width(&value[..current], font)) } else { 0.0 };
                let pos = self.caret_at(value, p.x - r.x - pad + scroll, font);
                if self.input.pressed {
                    self.anchor = if self.mods.shift && was_focused { Some(self.anchor.unwrap_or(current)) } else { Some(pos) };
                }
                self.cursor = pos;
                touched = true;
            }
        }

        let mut focused = self.focus == Some(id);
        if focused {
            let mut st = EditState { cursor: self.cursor, anchor: self.anchor, shift: self.mods.shift, copied: None };
            let e = edit_text(&mut st, &self.keys, &self.typed, &self.pasted, value, false, |v, _, k| match k {
                Key::Home => Some(0),
                Key::End => Some(v.len()),
                _ => None,
            });
            self.cursor = st.cursor;
            self.anchor = st.anchor;
            if st.copied.is_some() {
                self.clipboard_out = st.copied;
            }
            out.changed = e.changed;
            out.submitted = e.submitted;
            touched |= e.touched;
            if e.escaped {
                self.focus = None;
                focused = false;
            }
        }
        out.focused = focused;

        // What is drawn: the text with any input-method composition at the caret.
        let at = if focused { floor_boundary(value, self.cursor) } else { 0 };
        let composing = focused && !self.preedit.is_empty();
        self.scratch.clear();
        if composing {
            self.scratch.push_str(&value[..at]);
            self.scratch.push_str(&self.preedit);
            self.scratch.push_str(&value[at..]);
        }
        let shown: &str = if composing { &self.scratch } else { value };
        let caret_byte = if composing { at + self.preedit.len() } else { at };
        let caret = if focused { self.text.width(&shown[..caret_byte], font) } else { 0.0 };
        let scroll = scroll_for(caret);
        let sel = if focused && !composing { selection(at, self.anchor.map(|a| floor_boundary(value, a))) } else { None };
        let sel_x = sel.map(|(a, b)| (self.text.width(&value[..a], font) - scroll, self.text.width(&value[..b], font) - scroll));
        let pre_x = composing.then(|| (self.text.width(&shown[..at], font) - scroll, caret - scroll));

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
        if framed {
            self.draw.rect_stroked(r, self.theme.radius * 0.7, self.pal[RAISED].fade(a), stroke.fade(a), 1.0 + 0.6 * focus_t);
        }
        let y = r.y + (r.h - font * LINE_HEIGHT) * 0.5;
        let x0 = r.x + pad;
        self.draw.push(DrawCmd::PushClip { rect: Rect::new(x0 - 1.0, r.y, inner + 2.0, r.h), radius: 0.0 });
        if let Some((s0, s1)) = sel_x {
            self.draw.rect(Rect::new(x0 + s0, y, s1 - s0, font * LINE_HEIGHT), 3.0, accent.with_alpha(0.3 * a));
        }
        if shown.is_empty() {
            self.draw.add_text(Vec2::new(x0, y), font, self.pal[MUTED].fade(a), placeholder);
        } else {
            self.draw.add_text(Vec2::new(x0 - scroll, y), font, self.pal[TEXT].fade(a), shown);
        }
        if let Some((u0, u1)) = pre_x {
            let uy = y + font * LINE_HEIGHT - 3.0;
            self.draw.line(Vec2::new(x0 + u0, uy), Vec2::new(x0 + u1, uy), 1.5, accent.fade(a));
        }
        if focused {
            // The caret glides to its place, and blinks while idle.
            let phase = since.rem_euclid(1.0);
            let half = font * 0.62;
            if phase < 0.55 {
                let x = x0 + caret_x;
                self.draw.line(Vec2::new(x, r.center().y - half), Vec2::new(x, r.center().y + half), 1.6, accent.fade(a));
            }
            self.wake_at(now + if phase < 0.55 { 0.55 - phase } else { 1.0 - phase });
            self.ime_rect = Some(Rect::new(x0 + caret_x, r.center().y - half, 2.0, half * 2.0));
        }
        self.draw.push(DrawCmd::PopClip);
        self.describe(id, Role::TextInput, placeholder, r, Some(value.as_str()), None);
        out
    }

    /// Byte offset in `value` whose x position is closest to `x`.
    pub(crate) fn caret_at(&self, value: &str, x: f32, font: f32) -> usize {
        let mut best = (f32::MAX, 0);
        for i in value.char_indices().map(|(i, _)| i).chain([value.len()]) {
            let d = (self.text.width(&value[..i], font) - x).abs();
            if d < best.0 {
                best = (d, i);
            }
        }
        best.1
    }

    /// A viewport of fixed `height` over content of any length. The wheel
    /// scrolls it with inertia, and pushing past either end stretches the
    /// content a little before it springs back; content outside it is not
    /// drawn.
    pub fn scroll(&mut self, key: &str, height: f32, f: impl FnOnce(&mut Ui)) {
        let id = self.declare(key);
        let target = self.alloc(Vec2::new(self.available_width(), height));
        let (r, life) = self.enter(id, target, Smooth);
        let content = self.sizes.get(&id).map_or(0.0, |s| s.y);
        let max = (content - height).max(0.0);
        let over = self.pointer_in(r);
        let k = self.steps[Snappy as usize];
        if !self.scroll_ids.contains(&id) {
            self.scroll_ids.push(id);
        }
        let n = self.nodes.get_mut(&id).unwrap();
        // A restored position waits for the content to be measured, and is
        // held for a few frames while the layout finds its sizes.
        if let Some((offset, frames)) = self.scroll_restore.get_mut(&id).filter(|_| content > 0.0) {
            n.aux.snap(offset.clamp(0.0, max));
            *frames -= 1;
            if *frames == 0 {
                self.scroll_restore.remove(&id);
            }
        }
        n.aux.set(n.aux.target().clamp(0.0, max));
        // `e1` is the stretch past an end: it only ever springs back to 0.
        let smooth = self.steps[Smooth as usize];
        n.e1.set(0.0);
        let anim = n.aux.tick(&k) | n.hover.drive(over as u8 as f32, false, &k) | n.e1.tick(&smooth);
        let (offset, hover, stretch) = (n.aux.get(), n.hover.get(), n.e1.get());
        self.animating |= anim;
        let top = target.y - offset - stretch;
        // What scrolled since last frame moves the content rigidly (`stamp`
        // keeps last frame's amount); where the area itself goes, its
        // content follows on springs as anything else.
        let scrolled = (offset + stretch) as f64;
        let moved = if n.fresh() { 0.0 } else { (n.stamp - scrolled) as f32 };
        n.stamp = scrolled;

        let (clip, cull) = (self.clip, self.draw.cull);
        let view = clip.map_or(r, |c| c.intersect(&r));
        self.clip = Some(view);
        self.draw.cull = Some(view);
        // Scrolling cuts the content above and below; to the sides it only
        // leaves room for the shadows of what is inside.
        self.draw.push(DrawCmd::PushClip { rect: Rect::new(r.x - 28.0, r.y, r.w + 56.0, r.h), radius: 0.0 });
        let gap = self.theme.gap;
        self.scrolls.push(crate::scrolling::ScrollCtx { id, view, offset, max, top });
        let pin = std::mem::take(&mut self.pin_inset);
        let outer = self.shift;
        self.shift = outer + moved;
        self.group(id, Dir::Column, Vec2::ZERO, gap, Some(Vec2::new(target.x, top)), |ui| ui.within(id, f));
        self.shift = outer;
        self.pin_inset = pin;
        self.scrolls.pop();
        self.draw.push(DrawCmd::PopClip);
        self.clip = clip;
        self.draw.cull = cull;

        // The wheel is read after the content, so a scroll area nested
        // inside this one gets it first. Steps add up on the target; the
        // spring smooths the travel.
        if over && self.input.wheel != 0.0 {
            let wheel = std::mem::take(&mut self.input.wheel);
            self.scroll_restore.remove(&id);
            let n = self.nodes.get_mut(&id).unwrap();
            let want = n.aux.target() - wheel;
            n.aux.set(want.clamp(0.0, max));
            // Past an end, the push becomes a kick that stretches the
            // content and springs back.
            let excess = want - want.clamp(0.0, max);
            if excess != 0.0 && !self.instant {
                n.e1.impulse((excess * 9.0).clamp(-1100.0, 1100.0));
            }
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
pub(crate) fn floor_boundary(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
