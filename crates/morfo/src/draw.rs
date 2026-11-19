//! The frame's output: a flat list of primitives for a backend to paint.
//!
//! Buffers are reused between frames, so a steady-state frame allocates
//! nothing. Text is stored in one shared arena instead of one `String` per
//! command. Commands are recorded in layers (content, floating panels,
//! tooltips) and flattened in paint order when the frame ends.

use std::collections::HashMap;
use std::sync::Arc;

use crate::geom::{Color, Rect, Vec2};

/// How a run of text is set: monospaced, bold, italic, or any mix.
/// Combine with `|`: `TextStyle::BOLD | TextStyle::ITALIC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextStyle(pub u8);

impl TextStyle {
    pub const REGULAR: TextStyle = TextStyle(0);
    pub const MONO: TextStyle = TextStyle(1);
    pub const BOLD: TextStyle = TextStyle(2);
    pub const ITALIC: TextStyle = TextStyle(4);
    pub fn mono(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn bold(self) -> bool {
        self.0 & 2 != 0
    }
    pub fn italic(self) -> bool {
        self.0 & 4 != 0
    }
}

impl std::ops::BitOr for TextStyle {
    type Output = TextStyle;
    fn bitor(self, o: TextStyle) -> TextStyle {
        TextStyle(self.0 | o.0)
    }
}

/// An image registered with [`Ui::add_image`](crate::Ui::add_image).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImageId(pub u64);

/// The pixels of an image: `width`×`height`, four bytes each (red, green,
/// blue, alpha; sRGB, alpha not premultiplied), row by row.
#[derive(Debug)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Bumped each time the pixels are replaced (a video frame, a new
    /// photo): backends upload it again.
    pub version: u32,
}

impl ImageData {
    /// The color at `(x, y)`, clamped to the image; straight alpha.
    #[inline]
    pub fn pixel(&self, x: i32, y: i32) -> [u8; 4] {
        let x = x.clamp(0, self.width as i32 - 1) as usize;
        let y = y.clamp(0, self.height as i32 - 1) as usize;
        let i = (y * self.width as usize + x) * 4;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
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
    /// string with [`DrawList::text`]. `style` picks the face: monospaced,
    /// bold, italic.
    Text {
        pos: Vec2,
        size: f32,
        color: Color,
        start: u32,
        len: u32,
        style: TextStyle,
    },
    /// Straight stroke with round caps (check marks, chevrons, carets).
    Line {
        from: Vec2,
        to: Vec2,
        width: f32,
        color: Color,
    },
    /// Filled polygon (any shape, non-zero winding). Resolve its points
    /// with [`DrawList::points`].
    Polygon {
        start: u32,
        len: u32,
        color: Color,
    },
    /// Rounded rectangle filled with a linear gradient: `start` at point
    /// `from`, `end` at point `to`, blending in between and flat beyond.
    Gradient {
        rect: Rect,
        radius: f32,
        from: Vec2,
        to: Vec2,
        start: Color,
        end: Color,
    },
    /// Clip everything until the matching `PopClip` to a rounded rectangle.
    PushClip {
        rect: Rect,
        radius: f32,
    },
    PopClip,
    /// Image `image` (at `version`) drawn into `rect` with rounded corners
    /// of `radius`: the part `uv` of it (`0..1` both ways), multiplied by
    /// `tint` (white: as it is). Resolve its pixels with
    /// [`DrawList::image`].
    Image {
        rect: Rect,
        radius: f32,
        image: ImageId,
        version: u32,
        uv: Rect,
        tint: Color,
    },
}

/// Number of paint layers: content, floating panels, tooltips and toasts.
pub(crate) const LAYERS: usize = 3;

#[derive(Default, Debug)]
pub struct DrawList {
    layers: [Vec<DrawCmd>; LAYERS],
    layer: usize,
    text: String,
    points: Vec<Vec2>,
    /// Vertical band outside which primitives are dropped (scroll areas).
    pub(crate) cull: Option<Rect>,
    /// The images registered, which outlive frames.
    images: HashMap<ImageId, Arc<ImageData>>,
    next_image: u64,
    /// Lines read right to left: text is put in reading order on that base.
    pub(crate) rtl: bool,
    /// Text being edited is drawn in the order it is stored, so the caret
    /// stays where its characters are.
    pub(crate) logical: bool,
    /// Stretches of commands (layer, from, to) that a mirror moves without
    /// flipping, with the rectangle they live in: text being edited.
    upright: Vec<(usize, usize, usize, Rect)>,
}

impl DrawList {
    pub fn clear(&mut self) {
        for l in &mut self.layers {
            l.clear();
        }
        self.text.clear();
        self.points.clear();
        self.layer = 0;
        self.cull = None;
        self.upright.clear();
    }
    /// Where a stretch of commands that a mirror must not flip starts.
    pub(crate) fn upright_start(&self) -> (usize, usize) {
        (self.layer, self.mark())
    }
    /// Ends that stretch: what was drawn since `start` lives in `rect`.
    pub(crate) fn upright_end(&mut self, start: (usize, usize), rect: Rect) {
        if start.0 == self.layer {
            self.upright.push((start.0, start.1, self.mark(), rect));
        }
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
    /// The pixels of a [`DrawCmd::Image`].
    pub fn image(&self, id: ImageId) -> Option<&ImageData> {
        self.images.get(&id).map(|d| &**d)
    }
    pub(crate) fn add_image(&mut self, data: ImageData) -> ImageId {
        self.next_image += 1;
        let id = ImageId(self.next_image);
        self.images.insert(id, Arc::new(data));
        id
    }
    pub(crate) fn replace_image(&mut self, id: ImageId, mut data: ImageData) -> bool {
        let Some(old) = self.images.get_mut(&id) else { return false };
        data.version = old.version.wrapping_add(1);
        *old = Arc::new(data);
        true
    }
    pub(crate) fn remove_image(&mut self, id: ImageId) {
        self.images.remove(&id);
    }
    /// Draws image `id` into `rect` (see [`DrawCmd::Image`]). Unknown ids
    /// draw nothing.
    pub fn image_cmd(&mut self, rect: Rect, radius: f32, id: ImageId, uv: Rect, tint: Color) {
        let Some(data) = self.images.get(&id) else { return };
        if tint.a > 0.002 && rect.w > 0.0 && rect.h > 0.0 {
            let version = data.version;
            self.push(DrawCmd::Image { rect, radius, image: id, version, uv, tint });
        }
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
    /// Rewrites, in place, the commands of the current layer recorded since
    /// `mark` with `f`, and the points of their polygons with `p`.
    pub(crate) fn map_from(&mut self, mark: usize, f: impl Fn(DrawCmd) -> DrawCmd, p: impl Fn(Vec2) -> Vec2) {
        let layer = &mut self.layers[self.layer];
        for cmd in &mut layer[mark..] {
            *cmd = f(*cmd);
            if let DrawCmd::Polygon { start, len, .. } = *cmd {
                for q in &mut self.points[start as usize..(start + len) as usize] {
                    *q = p(*q);
                }
            }
        }
    }
    pub(crate) fn layer(&self) -> usize {
        self.layer
    }
    /// Switches the layer new commands go to; returns the previous one.
    pub(crate) fn set_layer(&mut self, layer: usize) -> usize {
        std::mem::replace(&mut self.layer, layer.min(LAYERS - 1))
    }
    /// Mirrors the whole (flattened) list about the vertical line through
    /// the middle of a `width` wide view, for interfaces written right to
    /// left. Text keeps reading forwards: each line moves so that it ends
    /// where it used to start, `width_of` telling how long it is.
    pub(crate) fn mirror(&mut self, width: f32, width_of: impl Fn(&str, f32, TextStyle) -> f32) {
        let flip = |r: Rect| Rect::new(width - r.x - r.w, r.y, r.w, r.h);
        let fx = |p: Vec2| Vec2::new(width - p.x, p.y);
        // Upright stretches move by how much their rectangle moves.
        let mut shift = vec![None; if self.upright.is_empty() { 0 } else { self.layers[0].len() }];
        for &(_, from, to, r) in &self.upright {
            let dx = width - r.x - r.w - r.x;
            let end = to.min(shift.len());
            for s in &mut shift[from.min(end)..end] {
                *s = Some(dx);
            }
        }
        for (i, cmd) in self.layers[0].iter_mut().enumerate() {
            if let Some(&Some(dx)) = shift.get(i) {
                let mv = |r: &mut Rect| r.x += dx;
                match cmd {
                    DrawCmd::Rect { rect, .. } | DrawCmd::Image { rect, .. } | DrawCmd::PushClip { rect, .. } | DrawCmd::Shadow { rect, .. } => {
                        mv(rect)
                    }
                    DrawCmd::Text { pos, .. } => pos.x += dx,
                    DrawCmd::Gradient { rect, from, to, .. } => {
                        mv(rect);
                        from.x += dx;
                        to.x += dx;
                    }
                    DrawCmd::Line { from, to, .. } => {
                        from.x += dx;
                        to.x += dx;
                    }
                    DrawCmd::Polygon { start, len, .. } => {
                        for q in &mut self.points[*start as usize..(*start + *len) as usize] {
                            q.x += dx;
                        }
                    }
                    _ => {}
                }
                continue;
            }
            match cmd {
                DrawCmd::Rect { rect, .. } | DrawCmd::Image { rect, .. } | DrawCmd::PushClip { rect, .. } => *rect = flip(*rect),
                DrawCmd::Shadow { rect, offset, .. } => {
                    *rect = flip(*rect);
                    offset.x = -offset.x;
                }
                DrawCmd::Gradient { rect, from, to, .. } => {
                    *rect = flip(*rect);
                    *from = fx(*from);
                    *to = fx(*to);
                }
                DrawCmd::Text { pos, size, start, len, style, .. } => {
                    let w = width_of(&self.text[*start as usize..(*start + *len) as usize], *size, *style);
                    pos.x = width - pos.x - w;
                }
                DrawCmd::Line { from, to, .. } => {
                    *from = fx(*from);
                    *to = fx(*to);
                }
                DrawCmd::Polygon { start, len, .. } => {
                    for q in &mut self.points[*start as usize..(*start + *len) as usize] {
                        *q = fx(*q);
                    }
                }
                _ => {}
            }
        }
    }
    /// Moves the upper layers after the base one, in paint order.
    pub(crate) fn flatten(&mut self) {
        let mut offsets = [0; LAYERS];
        for l in 1..LAYERS {
            offsets[l] = offsets[l - 1] + self.layers[l - 1].len();
        }
        for (layer, from, to, _) in &mut self.upright {
            *from += offsets[*layer];
            *to += offsets[*layer];
            *layer = 0;
        }
        let (base, upper) = self.layers.split_at_mut(1);
        for l in upper {
            base[0].append(l);
        }
        self.layer = 0;
    }
    fn hidden(&self, cmd: &DrawCmd) -> bool {
        let Some(c) = self.cull else { return false };
        let (y0, y1) = match *cmd {
            DrawCmd::Rect { rect, .. } | DrawCmd::Image { rect, .. } | DrawCmd::Gradient { rect, .. } => (rect.y, rect.bottom()),
            DrawCmd::Shadow { rect, blur, offset, .. } => (rect.y - blur, rect.bottom() + blur + offset.y),
            DrawCmd::Text { pos, size, .. } => (pos.y, pos.y + size * 1.4),
            DrawCmd::Line { from, to, width, .. } => (from.y.min(to.y) - width, from.y.max(to.y) + width),
            DrawCmd::Polygon { start, len, .. } => bounds_y(self.points(start, len)),
            _ => return false,
        };
        y1 < c.y || y0 > c.bottom()
    }
    /// The points of a [`DrawCmd::Polygon`].
    pub fn points(&self, start: u32, len: u32) -> &[Vec2] {
        &self.points[start as usize..(start + len) as usize]
    }
    /// All the polygon points of this frame (what `Polygon` commands index into).
    pub fn point_arena(&self) -> &[Vec2] {
        &self.points
    }
    /// A filled polygon through `pts`.
    pub fn polygon(&mut self, pts: &[Vec2], color: Color) {
        if color.a <= 0.002 || pts.len() < 3 {
            return;
        }
        let start = self.points.len() as u32;
        self.points.extend_from_slice(pts);
        let cmd = DrawCmd::Polygon { start, len: pts.len() as u32, color };
        if self.hidden(&cmd) {
            self.points.truncate(start as usize);
        } else {
            self.layers[self.layer].push(cmd);
        }
    }
    /// A thick arc of a circle with round ends, from angle `a0` to `a1`
    /// (radians, clockwise from the positive x axis as screen y goes down).
    pub fn arc(&mut self, c: Vec2, radius: f32, a0: f32, a1: f32, width: f32, color: Color) {
        let (a0, a1) = if a1 < a0 { (a1, a0) } else { (a0, a1) };
        if a1 - a0 < 1e-4 {
            return;
        }
        let half = width * 0.5;
        let at = |a: f32, r: f32| c + Vec2::new(a.cos(), a.sin()) * r;
        let steps = (((a1 - a0) * (radius + half) / 3.0).ceil() as usize).clamp(2, 256);
        let cap = 8;
        let mut pts = Vec::with_capacity(2 * (steps + cap) + 2);
        for i in 0..=steps {
            pts.push(at(a0 + (a1 - a0) * i as f32 / steps as f32, radius + half));
        }
        // Round end at a1: half a circle from the outer edge to the inner one.
        let end = at(a1, radius);
        for i in 1..cap {
            let t = a1 + std::f32::consts::PI * i as f32 / cap as f32;
            pts.push(end + Vec2::new(t.cos(), t.sin()) * half);
        }
        for i in (0..=steps).rev() {
            pts.push(at(a0 + (a1 - a0) * i as f32 / steps as f32, radius - half));
        }
        let start = at(a0, radius);
        for i in 1..cap {
            let t = a0 + std::f32::consts::PI + std::f32::consts::PI * i as f32 / cap as f32;
            pts.push(start + Vec2::new(t.cos(), t.sin()) * half);
        }
        self.polygon(&pts, color);
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
    /// A rounded rectangle filled with a linear gradient from `start` (at
    /// point `from`) to `end` (at point `to`).
    pub fn gradient(&mut self, rect: Rect, radius: f32, from: Vec2, to: Vec2, start: Color, end: Color) {
        if start.a > 0.002 || end.a > 0.002 {
            self.push(DrawCmd::Gradient { rect, radius, from, to, start, end });
        }
    }
    pub fn line(&mut self, from: Vec2, to: Vec2, width: f32, color: Color) {
        if color.a > 0.002 {
            self.push(DrawCmd::Line { from, to, width, color });
        }
    }
    pub fn add_text(&mut self, pos: Vec2, size: f32, color: Color, text: &str) {
        self.add_text_styled(pos, size, color, text, TextStyle::REGULAR);
    }
    /// Like [`add_text`](Self::add_text), in the monospaced font.
    pub fn add_code(&mut self, pos: Vec2, size: f32, color: Color, text: &str) {
        self.add_text_styled(pos, size, color, text, TextStyle::MONO);
    }
    /// One line of text in `style`.
    pub fn add_text_styled(&mut self, pos: Vec2, size: f32, color: Color, text: &str, style: TextStyle) {
        if color.a > 0.002 && !text.is_empty() {
            // Code reads left to right whatever the interface's direction.
            let text = if self.logical { std::borrow::Cow::Borrowed(text) } else { crate::bidi::visual(text, self.rtl && !style.mono()) };
            let cmd = DrawCmd::Text { pos, size, color, start: self.text.len() as u32, len: text.len() as u32, style };
            if !self.hidden(&cmd) {
                self.text.push_str(&text);
                self.layers[self.layer].push(cmd);
            }
        }
    }
}

/// Vertical extent of a set of points.
pub(crate) fn bounds_y(pts: &[Vec2]) -> (f32, f32) {
    pts.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)))
}
