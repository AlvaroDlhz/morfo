//! The frame's output: a flat list of primitives for a backend to paint.
//!
//! Buffers are reused between frames, so a steady-state frame allocates
//! nothing. Text is stored in one shared arena instead of one `String` per
//! command. Commands are recorded in layers (content, floating panels,
//! tooltips) and flattened in paint order when the frame ends.

use crate::geom::{Color, Rect, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrawCmd {
    /// Placeholder for a reserved slot; backends skip it.
    Nop,
    /// Rounded rectangle, optionally stroked (`stroke_width == 0` = no stroke).
    Rect {
        rect: Rect,
        radius: f32,
        fill: Color,
        stroke: Color,
        stroke_width: f32,
    },
    /// Soft drop shadow of a rounded rectangle.
    Shadow {
        rect: Rect,
        radius: f32,
        blur: f32,
        offset: Vec2,
        color: Color,
    },
    /// Single line of text; `pos` is the top-left of its line box. Resolve the
    /// string with [`DrawList::text`]. `mono` asks for the monospaced font.
    Text {
        pos: Vec2,
        size: f32,
        color: Color,
        start: u32,
        len: u32,
        mono: bool,
    },
    /// Straight stroke with round caps (check marks, chevrons, carets).
    Line {
        from: Vec2,
        to: Vec2,
        width: f32,
        color: Color,
    },
    /// Clip everything until the matching `PopClip` to a rounded rectangle.
    PushClip {
        rect: Rect,
        radius: f32,
    },
    PopClip,
}

/// Number of paint layers: content, floating panels, tooltips and toasts.
pub(crate) const LAYERS: usize = 3;

#[derive(Default, Debug)]
pub struct DrawList {
    layers: [Vec<DrawCmd>; LAYERS],
    layer: usize,
    text: String,
    /// Vertical band outside which primitives are dropped (scroll areas).
    pub(crate) cull: Option<Rect>,
}

impl DrawList {
    pub fn clear(&mut self) {
        for l in &mut self.layers {
            l.clear();
        }
        self.text.clear();
        self.layer = 0;
        self.cull = None;
    }
    /// All commands, in paint order (valid once the frame has ended).
    pub fn commands(&self) -> &[DrawCmd] {
        &self.layers[0]
    }
    pub fn len(&self) -> usize {
        self.layers[0].len()
    }
    pub fn is_empty(&self) -> bool {
        self.layers[0].is_empty()
    }
    /// The string of a [`DrawCmd::Text`].
    pub fn text(&self, start: u32, len: u32) -> &str {
        &self.text[start as usize..(start + len) as usize]
    }
    /// The whole text arena of this frame (what `Text` commands index into).
    pub fn text_arena(&self) -> &str {
        &self.text
    }
    /// Position the next command of the current layer will take.
    pub(crate) fn mark(&self) -> usize {
        self.layers[self.layer].len()
    }
    /// Commands of the current layer recorded since `mark`.
    pub(crate) fn cmds_from(&self, mark: usize) -> &[DrawCmd] {
        &self.layers[self.layer][mark..]
    }
    /// Moves the commands recorded since `mark` out of the list.
    pub(crate) fn drain_from(&mut self, mark: usize, into: &mut Vec<DrawCmd>) {
        into.extend(self.layers[self.layer].drain(mark..));
    }
    pub(crate) fn layer(&self) -> usize {
        self.layer
    }
    /// Switches the layer new commands go to; returns the previous one.
    pub(crate) fn set_layer(&mut self, layer: usize) -> usize {
        std::mem::replace(&mut self.layer, layer.min(LAYERS - 1))
    }
    /// Moves the upper layers after the base one, in paint order.
    pub(crate) fn flatten(&mut self) {
        let (base, upper) = self.layers.split_at_mut(1);
        for l in upper {
            base[0].append(l);
        }
        self.layer = 0;
    }
    fn hidden(&self, cmd: &DrawCmd) -> bool {
        let Some(c) = self.cull else { return false };
        let (y0, y1) = match *cmd {
            DrawCmd::Rect { rect, .. } => (rect.y, rect.bottom()),
            DrawCmd::Shadow { rect, blur, offset, .. } => (rect.y - blur, rect.bottom() + blur + offset.y),
            DrawCmd::Text { pos, size, .. } => (pos.y, pos.y + size * 1.4),
            DrawCmd::Line { from, to, width, .. } => (from.y.min(to.y) - width, from.y.max(to.y) + width),
            _ => return false,
        };
        y1 < c.y || y0 > c.bottom()
    }
    pub fn push(&mut self, cmd: DrawCmd) {
        if !self.hidden(&cmd) {
            self.layers[self.layer].push(cmd);
        }
    }
    /// Reserves a slot in the current layer to be filled later with
    /// [`set`](Self::set) (for backgrounds whose size is only known after
    /// their content).
    pub fn reserve(&mut self) -> usize {
        self.layers[self.layer].push(DrawCmd::Nop);
        self.layers[self.layer].len() - 1
    }
    pub fn set(&mut self, slot: usize, cmd: DrawCmd) {
        if !self.hidden(&cmd) {
            self.layers[self.layer][slot] = cmd;
        }
    }
    pub fn rect(&mut self, rect: Rect, radius: f32, fill: Color) {
        if fill.a > 0.002 && rect.w > 0.0 && rect.h > 0.0 {
            self.push(DrawCmd::Rect { rect, radius, fill, stroke: Color::TRANSPARENT, stroke_width: 0.0 });
        }
    }
    pub fn rect_stroked(&mut self, rect: Rect, radius: f32, fill: Color, stroke: Color, stroke_width: f32) {
        if (fill.a > 0.002 || stroke.a > 0.002) && rect.w > 0.0 && rect.h > 0.0 {
            self.push(DrawCmd::Rect { rect, radius, fill, stroke, stroke_width });
        }
    }
    pub fn line(&mut self, from: Vec2, to: Vec2, width: f32, color: Color) {
        if color.a > 0.002 {
            self.push(DrawCmd::Line { from, to, width, color });
        }
    }
    pub fn add_text(&mut self, pos: Vec2, size: f32, color: Color, text: &str) {
        self.add_text_styled(pos, size, color, text, false);
    }
    /// Like [`add_text`](Self::add_text), in the monospaced font.
    pub fn add_code(&mut self, pos: Vec2, size: f32, color: Color, text: &str) {
        self.add_text_styled(pos, size, color, text, true);
    }
    pub(crate) fn add_text_styled(&mut self, pos: Vec2, size: f32, color: Color, text: &str, mono: bool) {
        if color.a > 0.002 && !text.is_empty() {
            let cmd = DrawCmd::Text { pos, size, color, start: self.text.len() as u32, len: text.len() as u32, mono };
            if !self.hidden(&cmd) {
                self.text.push_str(text);
                self.layers[self.layer].push(cmd);
            }
        }
    }
}
