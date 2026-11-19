//! Wrapped text: paragraphs and a multi-line text area.

use crate::a11y::Role;
use crate::anim::SpringKind::{Smooth, Snappy};
use crate::draw::{DrawCmd, TextStyle};
use crate::edit::{edit_text, remember, selection, undo_redo, EditState};
use crate::geom::{Rect, Vec2};
use crate::input::{floor_boundary, TextEdit};
use crate::theme::{ACCENT, BORDER, MUTED, RAISED, TEXT};
use crate::ui::{Key, TextMeasure, Ui, LINE_HEIGHT};

/// One visual line of wrapped text: `text[start..end]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Line {
    pub start: usize,
    pub end: usize,
}

/// A run of text with a look of its own, in a [`Ui::rich`] paragraph:
/// `Span::new("bold").bold()`, `"plain".into()`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span<'a> {
    pub text: &'a str,
    pub style: TextStyle,
    /// In the accent color (a highlight, a term being defined).
    pub accent: bool,
    /// In the secondary color.
    pub muted: bool,
}

impl<'a> Span<'a> {
    pub fn new(text: &'a str) -> Span<'a> {
        Span { text, style: TextStyle::REGULAR, accent: false, muted: false }
    }
    pub fn bold(mut self) -> Self {
        self.style = self.style | TextStyle::BOLD;
        self
    }
    pub fn italic(mut self) -> Self {
        self.style = self.style | TextStyle::ITALIC;
        self
    }
    pub fn mono(mut self) -> Self {
        self.style = self.style | TextStyle::MONO;
        self
    }
    pub fn accent(mut self) -> Self {
        self.accent = true;
        self
    }
    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }
}

impl<'a> From<&'a str> for Span<'a> {
    fn from(text: &'a str) -> Span<'a> {
        Span::new(text)
    }
}

/// A piece of a laid-out [`Ui::rich`] paragraph: bytes `start..end` of span
/// `span`, at `x` on line `line`.
#[derive(Clone, Copy)]
pub(crate) struct Run {
    span: u16,
    start: u32,
    end: u32,
    x: f32,
    line: u16,
}

/// A paragraph broken into lines, kept from frame to frame: it is only
/// broken again when its text, its width or its font size change.
#[derive(Default)]
pub(crate) struct Wrapped {
    /// What `lines` was computed for (0: nothing yet).
    key: u64,
    lines: Vec<Line>,
    /// For a rich paragraph: its runs, and how many lines they take.
    runs: Vec<Run>,
    count: u16,
}

/// Breaks `text` into lines no wider than `width`: at newlines, between
/// words, and inside words too long to fit on a line of their own.
pub(crate) fn wrap(text: &str, width: f32, font: f32, m: &dyn TextMeasure, out: &mut Vec<Line>) {
    out.clear();
    let mut base = 0;
    for hard in text.split('\n') {
        let (mut line_start, mut line_end, mut cursor) = (0usize, 0usize, 0usize);
        for tok in hard.split_inclusive(' ') {
            let word_end = cursor + tok.trim_end_matches(' ').len();
            if line_end > line_start && m.width(&hard[line_start..word_end], font) > width {
                out.push(Line { start: base + line_start, end: base + line_end });
                line_start = cursor;
            }
            // A word wider than the whole line is cut between characters.
            while word_end > line_start && m.width(&hard[line_start..word_end], font) > width {
                let mut k = line_start;
                for (ci, ch) in hard[line_start..word_end].char_indices() {
                    let next = line_start + ci + ch.len_utf8();
                    if k > line_start && m.width(&hard[line_start..next], font) > width {
                        break;
                    }
                    k = next;
                }
                if k >= word_end {
                    break;
                }
                out.push(Line { start: base + line_start, end: base + k });
                line_start = k;
            }
            line_end = word_end;
            cursor += tok.len();
        }
        out.push(Line { start: base + line_start, end: base + line_end.max(line_start) });
        base += hard.len() + 1;
    }
}

/// Index of the line holding byte offset `at`.
fn line_of(lines: &[Line], at: usize) -> usize {
    lines.iter().rposition(|l| l.start <= at).unwrap_or(0)
}

/// Byte offset in `line` whose x position is closest to `x`.
fn closest(text: &str, line: Line, x: f32, font: f32, m: &dyn TextMeasure) -> usize {
    let mut best = (f32::MAX, line.start);
    for b in text[line.start..line.end].char_indices().map(|(i, _)| line.start + i).chain([line.end]) {
        let d = (m.width(&text[line.start..b], font) - x).abs();
        if d < best.0 {
            best = (d, b);
        }
    }
    best.1
}

impl Ui {
    /// Text that wraps to the available width.
    pub fn paragraph(&mut self, text: &str) {
        self.paragraph_styled(text, TEXT, self.theme.font);
    }

    /// Wrapped text in the secondary color.
    pub fn paragraph_muted(&mut self, text: &str) {
        self.paragraph_styled(text, MUTED, self.theme.font * 0.95);
    }

    fn paragraph_styled(&mut self, text: &str, color: usize, font: f32) {
        let id = self.auto_id();
        let width = self.available_width();
        let key = (crate::ui::hash_str(text) ^ ((width.to_bits() as u64) << 32 | font.to_bits() as u64).wrapping_mul(0xff51_afd7_ed55_8ccd)) | 1;
        let mut wrapped = self.wraps.remove(&id).unwrap_or_default();
        if wrapped.key != key {
            wrap(text, width, font, &*self.text, &mut wrapped.lines);
            wrapped.key = key;
        }
        let lh = font * LINE_HEIGHT;
        let target = self.alloc(Vec2::new(width, wrapped.lines.len() as f32 * lh));
        let (r, life) = self.enter(id, target, Smooth);
        self.describe(id, Role::Label, text, r, None, None);
        let ink = self.pal[color].fade(self.alpha * life);
        for (i, l) in wrapped.lines.iter().enumerate() {
            self.draw.add_text(Vec2::new(r.x, r.y + i as f32 * lh), font, ink, &text[l.start..l.end]);
        }
        self.wraps.insert(id, wrapped);
    }

    /// A paragraph of runs in different styles and colors, wrapped to the
    /// available width as one text:
    ///
    /// ```
    /// # use morfo::{Span, Theme, Ui, Vec2};
    /// # let mut ui = Ui::new(Theme::dark());
    /// # ui.begin(0.0, Vec2::new(800.0, 600.0));
    /// ui.rich(&[Span::new("Press "), Span::new("Save").bold(), Span::new(" or "), Span::new("Ctrl+S").mono(), " to keep it.".into()]);
    /// # ui.end();
    /// ```
    ///
    /// Without bold or italic faces in the renderer, bold is drawn twice a
    /// hair apart and italic stays upright.
    pub fn rich(&mut self, spans: &[Span]) {
        let id = self.auto_id();
        let width = self.available_width();
        let font = self.theme.font;
        // What the layout depends on: every span's text and look, the width.
        let mut key = (width.to_bits() as u64) << 32 | font.to_bits() as u64;
        for s in spans {
            key = (key ^ crate::ui::hash_str(s.text) ^ (s.style.0 as u64) << 56).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        }
        let key = key | 1;
        let mut laid = self.wraps.remove(&id).unwrap_or_default();
        if laid.key != key {
            laid.runs.clear();
            let (mut x, mut line) = (0.0f32, 0u16);
            for (si, s) in spans.iter().enumerate() {
                for (k, hard) in s.text.split('\n').enumerate() {
                    if k > 0 {
                        line += 1;
                        x = 0.0;
                    }
                    let base = hard.as_ptr() as usize - s.text.as_ptr() as usize;
                    let mut at = 0;
                    for tok in hard.split_inclusive(' ') {
                        let word = tok.trim_end_matches(' ');
                        let w = self.text.width_styled(word, font, s.style);
                        if x > 0.0 && x + w > width {
                            line += 1;
                            x = 0.0;
                        }
                        let (start, end) = (base + at, base + at + tok.len());
                        // Consecutive pieces of one span on one line are one run.
                        match laid.runs.last_mut() {
                            Some(r) if r.span == si as u16 && r.line == line && r.end as usize == start => r.end = end as u32,
                            _ => laid.runs.push(Run { span: si as u16, start: start as u32, end: end as u32, x, line }),
                        }
                        x += self.text.width_styled(tok, font, s.style);
                        at += tok.len();
                    }
                }
            }
            laid.count = line + 1;
            laid.key = key;
        }
        let lh = font * LINE_HEIGHT;
        let target = self.alloc(Vec2::new(width, laid.count as f32 * lh));
        let (r, life) = self.enter(id, target, Smooth);
        if self.a11y_on {
            let all: String = spans.iter().map(|s| s.text).collect();
            self.describe(id, Role::Label, &all, r, None, None);
        }
        let a = self.alpha * life;
        for run in &laid.runs {
            let Some(s) = spans.get(run.span as usize) else { continue };
            let Some(text) = s.text.get(run.start as usize..run.end as usize) else { continue };
            let color = if s.accent {
                self.pal[ACCENT]
            } else if s.muted {
                self.pal[MUTED]
            } else {
                self.pal[TEXT]
            };
            let pos = Vec2::new(r.x + run.x, r.y + run.line as f32 * lh);
            self.draw.add_text_styled(pos, font, color.fade(a), text.trim_end_matches(' '), s.style);
        }
        self.wraps.insert(id, laid);
    }

    /// Multi-line text field: wraps as you type, Enter starts a new line,
    /// the arrows move through lines, and it scrolls to keep the caret in
    /// view (the wheel scrolls it too). Selection, clipboard and input
    /// methods work as in [`text_field`](Self::text_field).
    pub fn text_area(&mut self, key: &str, value: &mut String, placeholder: &str, size: Vec2) -> TextEdit {
        let id = self.declare(key);
        let (font, pad) = (self.theme.font, self.theme.pad * 0.7);
        let lh = font * LINE_HEIGHT;
        let target = self.alloc(size);
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        self.text_ids.push(id);
        let inner = Rect::new(r.x + pad, r.y + pad * 0.6, (r.w - pad * 2.0).max(1.0), (r.h - pad * 1.2).max(1.0));
        let mut lines = Vec::new();
        wrap(value, inner.w, font, &*self.text, &mut lines);
        let mut offset = self.nodes[&id].e1.target();
        let mut out = TextEdit::default();
        let mut touched = false;

        // Pointer: press places the caret (Shift extends), dragging selects.
        if self.active == Some(id) && self.input.down && self.focus == Some(id) {
            if let Some(p) = self.input.pointer {
                let p = Vec2::new(self.upright_x(p.x, r), p.y);
                let li = (((p.y - inner.y + offset) / lh).floor().max(0.0) as usize).min(lines.len() - 1);
                let pos = closest(value, lines[li], p.x - inner.x, font, &*self.text);
                if self.input.pressed {
                    let was_focused = !self.focus_changed;
                    let current = floor_boundary(value, self.cursor);
                    self.anchor = if self.mods.shift && was_focused { Some(self.anchor.unwrap_or(current)) } else { Some(pos) };
                }
                self.cursor = pos;
                touched = true;
            }
        }

        let mut focused = self.focus == Some(id);
        if let (true, Some(at)) = (focused, undo_redo(&mut self.histories, id, &self.keys, value, self.cursor)) {
            self.cursor = at;
            self.anchor = None;
            out.changed = true;
            touched = true;
        } else if focused {
            let width = inner.w;
            let m = &*self.text;
            let busy = !self.keys.is_empty() || !self.typed.is_empty() || !self.pasted.is_empty();
            let before = busy.then(|| (value.clone(), self.cursor));
            let mut st = EditState { cursor: self.cursor, anchor: self.anchor, shift: self.mods.shift, word: self.mods.ctrl, copied: None };
            let e = edit_text(&mut st, &self.keys, &self.typed, &self.pasted, value, true, |v, at, k| {
                let mut ls = Vec::new();
                wrap(v, width, font, m, &mut ls);
                let li = line_of(&ls, at);
                let line = ls[li];
                match k {
                    Key::Home => Some(line.start),
                    Key::End => Some(line.end),
                    Key::Up | Key::Down => {
                        let x = m.width(&v[line.start..at.min(line.end)], font);
                        let to = if k == Key::Up { li.checked_sub(1) } else { Some(li + 1).filter(|&n| n < ls.len()) };
                        Some(match to {
                            Some(n) => closest(v, ls[n], x, font, m),
                            None if k == Key::Up => 0,
                            None => v.len(),
                        })
                    }
                    _ => None,
                }
            });
            if let (true, Some((text, at))) = (e.changed, before) {
                let typing = self.keys.is_empty() && self.pasted.is_empty();
                remember(&mut self.histories, id, text, at, self.now, typing);
            }
            self.cursor = st.cursor;
            self.anchor = st.anchor;
            if st.copied.is_some() {
                self.clipboard_out = st.copied;
            }
            out.changed = e.changed;
            touched |= e.touched;
            if e.escaped {
                self.focus = None;
                focused = false;
            }
            if out.changed {
                wrap(value, inner.w, font, &*self.text, &mut lines);
            }
        }
        out.focused = focused;

        // Scroll: the wheel, and following the caret.
        let content = lines.len() as f32 * lh;
        let max = (content - inner.h).max(0.0);
        if self.pointer_in(r) && self.input.wheel != 0.0 {
            offset -= std::mem::take(&mut self.input.wheel);
        }
        // The caret only belongs to this field while it has the focus.
        let caret = if focused { floor_boundary(value, self.cursor) } else { 0 };
        let caret_line = line_of(&lines, caret);
        if focused && touched {
            let y = caret_line as f32 * lh;
            offset = offset.min(y).max(y + lh - inner.h);
        }
        let offset = offset.clamp(0.0, max);
        let cl = lines[caret_line];
        let caret_x = self.text.width(&value[cl.start..caret.clamp(cl.start, cl.end)], font);
        let sel = if focused { selection(caret, self.anchor.map(|a| floor_boundary(value, a))) } else { None };

        let now = self.now;
        let accent = self.pal[ACCENT];
        let idle = self.pal[BORDER].mix(self.pal[TEXT], if hit.hovered { 0.3 } else { 0.0 });
        let (snappy, smooth) = (self.steps[Snappy as usize], self.steps[Smooth as usize]);
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        if touched {
            n.stamp = now;
        }
        let anim = n.stroke.drive(if focused { accent } else { idle }, fresh, &smooth)
            | n.press.drive(focused as u8 as f32, fresh, &smooth)
            | n.e1.drive(offset, fresh, &snappy);
        let (stroke, focus_t, off, since) = (n.stroke.get(), n.press.get().clamp(0.0, 1.0), n.e1.get(), now - n.stamp);
        self.animating |= anim;

        let a = self.alpha * life;
        let upright = self.draw.upright_start();
        self.draw.logical = true;
        self.draw.rect_stroked(r, self.theme.radius * 0.7, self.pal[RAISED].fade(a), stroke.fade(a), 1.0 + 0.6 * focus_t);
        let clip = Rect::new(inner.x - 2.0, r.y + 2.0, inner.w + 4.0, r.h - 4.0);
        self.draw.push(DrawCmd::PushClip { rect: clip, radius: 0.0 });
        if value.is_empty() {
            self.draw.add_text(inner.min(), font, self.pal[MUTED].fade(a), placeholder);
        }
        let first = (off / lh).floor().max(0.0) as usize;
        let visible = (inner.h / lh).ceil() as usize + 2;
        for (i, l) in lines.iter().enumerate().skip(first).take(visible) {
            let y = inner.y + i as f32 * lh - off;
            if let Some((s0, s1)) = sel {
                // The part of the selection on this line (a little past its
                // end when it continues on the next one).
                let (a0, a1) = (s0.max(l.start), s1.min(l.end));
                if s0 <= l.end && s1 >= l.start && (a0 < a1 || s1 > l.end) {
                    let x0 = self.text.width(&value[l.start..a0.min(l.end)], font);
                    let x1 = self.text.width(&value[l.start..a1.max(a0)], font) + if s1 > l.end { 6.0 } else { 0.0 };
                    self.draw.rect(Rect::new(inner.x + x0, y, x1 - x0, lh), 3.0, accent.with_alpha(0.3 * a));
                }
            }
            self.draw.add_text(Vec2::new(inner.x, y), font, self.pal[TEXT].fade(a), &value[l.start..l.end]);
        }
        self.draw.push(DrawCmd::PopClip);
        if focused {
            let y = inner.y + caret_line as f32 * lh - off;
            let x = inner.x + caret_x;
            let phase = since.rem_euclid(1.0);
            if phase < 0.55 {
                self.draw.line(Vec2::new(x, y + lh * 0.12), Vec2::new(x, y + lh * 0.88), 1.6, accent.fade(a));
            }
            self.wake_at(now + if phase < 0.55 { 0.55 - phase } else { 1.0 - phase });
            self.ime_rect = Some(self.upright_rect(Rect::new(x, y, 2.0, lh), r));
            if !self.preedit.is_empty() {
                // Text being composed floats just below the caret.
                let w = self.text.width(&self.preedit, font) + 16.0;
                let bubble = Rect::new(x, y + lh + 2.0, w, lh + 8.0);
                self.draw.rect_stroked(bubble, 8.0, self.pal[RAISED].fade(a), accent.fade(a), 1.0);
                let pre = std::mem::take(&mut self.preedit);
                self.draw.add_text(Vec2::new(bubble.x + 8.0, bubble.y + 4.0), font, self.pal[TEXT].fade(a), &pre);
                let uy = bubble.y + 4.0 + lh - 3.0;
                self.draw.line(Vec2::new(bubble.x + 8.0, uy), Vec2::new(bubble.right() - 8.0, uy), 1.5, accent.fade(a));
                self.preedit = pre;
            }
        }
        if max > 0.0 {
            let thumb = (inner.h * inner.h / content).max(20.0);
            let y = inner.y + (inner.h - thumb) * (off / max).clamp(0.0, 1.0);
            self.draw.rect(Rect::new(r.right() - 6.0, y, 3.0, thumb), 2.0, self.pal[MUTED].with_alpha(0.4 * a));
        }
        self.draw.logical = false;
        self.draw.upright_end(upright, r);
        self.describe(id, Role::MultilineTextInput, placeholder, r, Some(value.as_str()), None);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::ApproxText;

    fn lines(text: &str, width: f32) -> Vec<&str> {
        let mut out = Vec::new();
        wrap(text, width, 10.0, &ApproxText, &mut out);
        out.iter().map(|l| &text[l.start..l.end]).collect()
    }

    #[test]
    fn wraps_words_newlines_and_long_words() {
        // ApproxText: 5.4 px per character at size 10.
        assert_eq!(lines("uno dos tres cuatro", 45.0), ["uno dos", "tres", "cuatro"]);
        assert_eq!(lines("a\n\nb", 100.0), ["a", "", "b"]);
        assert_eq!(lines("abcdefghij", 30.0), ["abcde", "fghij"]);
        assert_eq!(lines("", 30.0), [""]);
    }
}
