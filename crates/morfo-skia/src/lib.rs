//! CPU renderer for [`morfo`]: paints a [`DrawList`] into a `tiny-skia`
//! pixmap. No GPU, no windowing assumptions — hand the pixels to anything
//! (softbuffer, a canvas, a PNG, a framebuffer).
//!
//! Built to do as little as possible: only the rectangle around what
//! changed since the previous frame is repainted, in parallel strips when
//! it is large, and glyph bitmaps and text metrics are cached between
//! frames.

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::Arc;

use morfo::ui::LINE_HEIGHT;
use morfo::{Color, DrawCmd, DrawList, Rect, TextStyle, Vec2};
use morfo_fonts::ab_glyph::{point, Font};
use morfo_fonts::{Face, FontSet};
use tiny_skia::{Pixmap, PixmapMut};

pub use morfo_fonts::{FontMeasure, InvalidFont};
pub use tiny_skia;

struct Glyph {
    w: i32,
    h: i32,
    left: i32,
    top: i32,
    coverage: Vec<u8>,
}

/// (glyph, size in 1/8 px, horizontal subpixel in 1/4 px, face)
type GlyphKey = (u16, u32, u8, Face);

/// How far apart, in device pixels, the two passes of faked bold text are.
fn bold_offset(size: f32) -> f32 {
    (size * 0.045).clamp(0.5, 2.0)
}

/// What the previous frame was painted with; if any of it changes,
/// everything is repainted.
type Target = (u32, u32, u32, [u8; 4]);

/// Deepest clip nesting painted exactly; deeper clips reuse the last one.
const MAX_CLIPS: usize = 16;

pub struct Renderer {
    fonts: Arc<FontSet>,
    glyphs: HashMap<GlyphKey, Option<Glyph>>,
    /// Last frame's commands and text, to repaint only what changed.
    prev: Vec<DrawCmd>,
    prev_text: String,
    prev_points: Vec<Vec2>,
    prev_target: Option<Target>,
    threads: usize,
}

impl Renderer {
    /// `font` is the bytes of a TrueType/OpenType font.
    pub fn new(font: Vec<u8>) -> Result<Renderer, InvalidFont> {
        Ok(Renderer::with_fonts(FontSet::new(font)?))
    }

    /// A renderer drawing with every face of `fonts`.
    pub fn with_fonts(fonts: FontSet) -> Renderer {
        Renderer {
            fonts: Arc::new(fonts),
            glyphs: HashMap::new(),
            prev: Vec::new(),
            prev_text: String::new(),
            prev_points: Vec::new(),
            prev_target: None,
            threads: std::thread::available_parallelism().map_or(1, |n| n.get()).min(8),
        }
    }

    /// Changes the fonts (adds a face, a fallback): `f` gets them.
    fn fonts_mut(&mut self, f: impl FnOnce(&mut FontSet) -> Result<(), InvalidFont>) -> Result<(), InvalidFont> {
        f(Arc::make_mut(&mut self.fonts))?;
        self.glyphs.clear();
        self.invalidate();
        Ok(())
    }

    /// Font used for code. Without one, code is drawn in the main font.
    pub fn set_mono_font(&mut self, font: Vec<u8>) -> Result<(), InvalidFont> {
        self.fonts_mut(|f| f.set_mono(font))
    }

    /// Font for bold text. Without one, bold is drawn twice, a hair apart.
    pub fn set_bold_font(&mut self, font: Vec<u8>) -> Result<(), InvalidFont> {
        self.fonts_mut(|f| f.set_bold(font))
    }

    /// Font for italic text. Without one, italic stays upright.
    pub fn set_italic_font(&mut self, font: Vec<u8>) -> Result<(), InvalidFont> {
        self.fonts_mut(|f| f.set_italic(font))
    }

    /// A font for the characters the others lack (symbols, other scripts),
    /// tried after those added before it.
    pub fn add_fallback_font(&mut self, font: Vec<u8>) -> Result<(), InvalidFont> {
        self.fonts_mut(|f| f.add_fallback(font))
    }

    /// The fonts this renderer draws with.
    pub fn fonts(&self) -> &Arc<FontSet> {
        &self.fonts
    }

    /// Maximum number of threads used to paint a frame (1 = no threads).
    pub fn set_threads(&mut self, threads: usize) {
        self.threads = threads.max(1);
    }

    /// Forgets the previous frame: the next [`render`](Self::render) repaints
    /// everything. Call it if the pixmap was drawn to by something else.
    pub fn invalidate(&mut self) {
        self.prev_target = None;
    }

    /// Text metrics that match what this renderer draws.
    pub fn measure(&self) -> FontMeasure {
        FontMeasure::new(self.fonts.clone())
    }

    /// Paints `list` over `background`. `scale` is the device pixel ratio.
    ///
    /// The pixmap is expected to still hold the previous frame: only the
    /// rectangle around what changed is repainted, split between threads
    /// when it is large enough. Returns the rows it spans `(first, end)`, or
    /// `None` when the frame is identical to the last one and nothing was
    /// touched.
    ///
    /// Clips are applied as rectangles (their corner radius is ignored):
    /// rows and columns outside them are simply not visited, so clipping
    /// is free — no mask is ever allocated or sampled.
    pub fn render(&mut self, list: &DrawList, pixmap: &mut Pixmap, scale: f32, background: Color) -> Option<(u32, u32)> {
        let (width, height) = (pixmap.width(), pixmap.height());
        let target: Target = (width, height, scale.to_bits(), background.to_rgba8());
        // What changed, as rows and columns of the pixmap.
        let area = if self.prev_target == Some(target) {
            self.damage(list).map(|[x0, y0, x1, y1]| {
                let lo = |v: f32, max: u32| (((v * scale).floor() - 1.0).max(0.0) as u32).min(max);
                let hi = |v: f32, max: u32| (((v * scale).ceil() + 1.0).max(0.0) as u32).min(max);
                ((lo(y0, height), hi(y1, height)), (lo(x0, width) as i32, hi(x1, width) as i32))
            })
        } else {
            Some(((0, height), (0, width as i32)))
        };
        self.prev.clear();
        self.prev.extend_from_slice(list.commands());
        self.prev_text.clear();
        self.prev_text.push_str(list.text_arena());
        self.prev_points.clear();
        self.prev_points.extend_from_slice(list.point_arena());
        self.prev_target = Some(target);
        let ((first, end), cols) = area.filter(|((a, b), (c, d))| b > a && d > c)?;

        if self.glyphs.len() > 4096 {
            self.glyphs.clear();
        }
        self.prepare_glyphs(list, scale, first, end);

        let stride = width as usize * 4;
        let total = (end - first) as usize;
        let data = &mut pixmap.data_mut()[first as usize * stride..end as usize * stride];
        let strips = if total * ((cols.1 - cols.0) as usize) < 120_000 { 1 } else { self.threads.min(total / 32).max(1) };
        let this = &*self;
        if strips == 1 {
            this.paint_rows(list, data, width, cols, first, end, scale, background);
        } else {
            std::thread::scope(|scope| {
                let mut rest = data;
                let mut y = first;
                for i in 0..strips {
                    let h = (end - y) / (strips - i) as u32;
                    let (strip, tail) = std::mem::take(&mut rest).split_at_mut(h as usize * stride);
                    rest = tail;
                    let (y0, y1) = (y, y + h);
                    y = y1;
                    if i + 1 == strips {
                        this.paint_rows(list, strip, width, cols, y0, y1, scale, background);
                    } else {
                        scope.spawn(move || this.paint_rows(list, strip, width, cols, y0, y1, scale, background));
                    }
                }
            });
        }
        Some((first, end))
    }

    /// The rectangle (logical pixels, `[x0, y0, x1, y1]`) around everything
    /// that differs from the previous frame.
    fn damage(&self, list: &DrawList) -> Option<[f32; 4]> {
        let (old, new) = (&self.prev[..], list.commands());
        let same = |a: &DrawCmd, b: &DrawCmd| match (*a, *b) {
            (
                DrawCmd::Text { pos, size, color, start, len, style },
                DrawCmd::Text { pos: pos2, size: size2, color: color2, start: start2, len: len2, style: style2 },
            ) => {
                pos == pos2
                    && size == size2
                    && color == color2
                    && style == style2
                    && self.prev_text[start as usize..(start + len) as usize] == *list.text(start2, len2)
            }
            (DrawCmd::Polygon { start, len, color }, DrawCmd::Polygon { start: start2, len: len2, color: color2 }) => {
                color == color2 && self.prev_points[start as usize..(start + len) as usize] == *list.points(start2, len2)
            }
            _ => a == b,
        };
        let (old_pts, new_pts) = (&self.prev_points[..], list.point_arena());
        let (old_text, new_text) = (&self.prev_text[..], list.text_arena());
        let mut span: Option<[f32; 4]> = None;
        let mut add = |cmd: &DrawCmd, pts: &[Vec2], text: &str| {
            if let (Some((y0, y1)), Some((x0, x1))) = (extent(cmd, pts), self.extent_x(cmd, pts, text)) {
                span = Some(span.map_or([x0, y0, x1, y1], |s| [s[0].min(x0), s[1].min(y0), s[2].max(x1), s[3].max(y1)]));
            }
        };
        if old.len() == new.len() {
            for (a, b) in old.iter().zip(new) {
                if !same(a, b) {
                    add(a, old_pts, old_text);
                    add(b, new_pts, new_text);
                }
            }
        } else {
            // Something was added or removed: everything between the common
            // start and the common end is suspect.
            let shorter = old.len().min(new.len());
            let head = (0..shorter).take_while(|&i| same(&old[i], &new[i])).count();
            let tail = (0..shorter - head).take_while(|&i| same(&old[old.len() - 1 - i], &new[new.len() - 1 - i])).count();
            old[head..old.len() - tail].iter().for_each(|c| add(c, old_pts, old_text));
            new[head..new.len() - tail].iter().for_each(|c| add(c, new_pts, new_text));
        }
        span
    }

    /// Horizontal extent of a command in logical pixels, with a safety
    /// margin. `text` is the arena its string lives in.
    fn extent_x(&self, cmd: &DrawCmd, points: &[Vec2], text: &str) -> Option<(f32, f32)> {
        Some(match *cmd {
            DrawCmd::Polygon { start, len, .. } => {
                let (x0, x1) = points[start as usize..(start + len) as usize].iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
                (x0 - 1.0, x1 + 1.0)
            }
            DrawCmd::Rect { rect, stroke_width, .. } => (rect.x - stroke_width - 1.0, rect.right() + stroke_width + 1.0),
            DrawCmd::Image { rect, .. } | DrawCmd::Gradient { rect, .. } => (rect.x - 1.0, rect.right() + 1.0),
            DrawCmd::Shadow { rect, blur, offset, .. } => (rect.x + offset.x - blur - 1.0, rect.right() + offset.x + blur + 1.0),
            DrawCmd::Text { pos, size, start, len, style, .. } => {
                let end = pos.x + self.fonts.width(&text[start as usize..(start + len) as usize], size, style);
                // Glyphs may reach a little past their advance on both sides.
                (pos.x - size, end + size)
            }
            DrawCmd::Line { from, to, width, .. } => (from.x.min(to.x) - width - 1.0, from.x.max(to.x) + width + 1.0),
            // A clip that changes exposes or hides whatever is inside it.
            DrawCmd::PushClip { rect, .. } => (rect.x - 1.0, rect.right() + 1.0),
            _ => return None,
        })
    }

    /// Rasterizes the glyphs the rows `first..end` will need, so painting
    /// can then share the cache between threads without locking.
    fn prepare_glyphs(&mut self, list: &DrawList, scale: f32, first: u32, end: u32) {
        let fonts = self.fonts.clone();
        for cmd in list.commands() {
            let DrawCmd::Text { pos, size, start, len, style, .. } = *cmd else { continue };
            if !touches(cmd, list.point_arena(), scale, first, end) {
                continue;
            }
            let device = size * scale;
            let size_key = (device * 8.0).round() as u32;
            let (_, fake_bold) = fonts.pick(style);
            let passes: &[f32] = if fake_bold { &[0.0, 1.0] } else { &[0.0] };
            let glyphs = &mut self.glyphs;
            fonts.walk(list.text(start, len), device, style, pos.x * scale, |face, id, pen| {
                for &k in passes {
                    let pen = pen + k * bold_offset(device);
                    let sub = ((pen - pen.floor()) * 4.0) as u8;
                    glyphs.entry((id.0, size_key, sub, face)).or_insert_with(|| {
                        // Rasterize at the size the key stands for, not at the
                        // exact size of whoever asks first: the same key always
                        // yields the same bitmap.
                        let font = fonts.face(face);
                        let px = fonts.px_scale(face, size_key as f32 / 8.0);
                        let outlined = font.outline_glyph(id.with_scale_and_position(px, point(sub as f32 * 0.25, 0.0)))?;
                        let bounds = outlined.px_bounds();
                        let (w, h) = (bounds.width() as i32, bounds.height() as i32);
                        let mut coverage = vec![0u8; (w * h).max(0) as usize];
                        outlined.draw(|gx, gy, c| {
                            if let Some(px) = coverage.get_mut((gy as i32 * w + gx as i32) as usize) {
                                *px = (c.clamp(0.0, 1.0) * 255.0) as u8;
                            }
                        });
                        Some(Glyph { w, h, left: bounds.min.x as i32, top: bounds.min.y as i32, coverage })
                    });
                }
            });
        }
    }

    /// Paints columns `damage` of device rows `first..end` of the frame
    /// into `rows` (exactly those rows of the pixmap). Every pixel is
    /// independent of every other.
    #[allow(clippy::too_many_arguments)]
    fn paint_rows(&self, list: &DrawList, rows: &mut [u8], width: u32, damage: (i32, i32), first: u32, end: u32, scale: f32, background: Color) {
        let stride = width as usize * 4;
        if damage == (0, width as i32) {
            if let Some(mut all) = PixmapMut::from_bytes(rows, width, end - first) {
                all.fill(sk_color(background));
            }
        } else {
            let c = sk_color(background).premultiply().to_color_u8();
            let px = [c.red(), c.green(), c.blue(), c.alpha()];
            for row in rows.chunks_exact_mut(stride) {
                for p in row[damage.0 as usize * 4..damage.1 as usize * 4].as_chunks_mut::<4>().0 {
                    *p = px;
                }
            }
        }
        let mut clips = [Rect::default(); MAX_CLIPS];
        let mut depth = 0usize;
        for cmd in list.commands() {
            match *cmd {
                DrawCmd::PushClip { rect, .. } => {
                    let rect = if depth > 0 { clips[depth.min(MAX_CLIPS) - 1].intersect(&rect) } else { rect };
                    if depth < MAX_CLIPS {
                        clips[depth] = rect;
                    }
                    depth += 1;
                    continue;
                }
                DrawCmd::PopClip => {
                    depth = depth.saturating_sub(1);
                    continue;
                }
                DrawCmd::Nop => continue,
                _ => {}
            }
            // Rows of this strip visible through the current clip, and the
            // columns.
            let clip = (depth > 0).then(|| clips[depth.min(MAX_CLIPS) - 1]);
            let cols = clip.map_or(damage, |c| {
                (((c.x * scale).floor() as i32).clamp(damage.0, damage.1), ((c.right() * scale).ceil() as i32).clamp(damage.0, damage.1))
            });
            if cols.1 <= cols.0 {
                continue;
            }
            let (y0, y1) = match clip {
                Some(c) => {
                    (((c.y * scale).round().max(0.0) as u32).clamp(first, end), ((c.bottom() * scale).round().max(0.0) as u32).clamp(first, end))
                }
                None => (first, end),
            };
            if y1 <= y0 || !touches(cmd, list.point_arena(), scale, y0, y1) {
                continue;
            }
            if let DrawCmd::Text { pos, size, color, start, len, style } = *cmd {
                self.blit_text(rows, width, first, (y0, y1), list.text(start, len), pos.x * scale, pos.y * scale, size * scale, color, cols, style);
                continue;
            }
            let band = &mut rows[(y0 - first) as usize * stride..(y1 - first) as usize * stride];
            match *cmd {
                DrawCmd::Rect { rect, radius, fill, stroke, stroke_width } => {
                    let shape = Shape::new(rect, radius, scale);
                    if fill.a > 0.002 {
                        // Coverage only varies near the left and right ends;
                        // in between it depends on the row alone.
                        let reach = shape.r + 1.0;
                        paint(band, width, cols, y0, y1, shape.bounds(1.0), shape.middle(reach), fill, |x, y| edge(shape.distance(x, y)));
                    }
                    let w = stroke_width * scale;
                    if w > 0.0 && stroke.a > 0.002 {
                        let inner = Shape::new(rect.inset(stroke_width), radius - stroke_width, scale);
                        let reach = shape.r.max(w) + 1.0;
                        paint(band, width, cols, y0, y1, shape.bounds(1.0), shape.middle(reach), stroke, |x, y| {
                            edge(shape.distance(x, y)) - if inner.empty() { 0.0 } else { edge(inner.distance(x, y)) }
                        });
                    }
                }
                DrawCmd::Shadow { rect, radius, blur, offset, color } => {
                    if color.a < 0.004 {
                        continue;
                    }
                    // Opacity falls off smoothly with the distance to the
                    // shape: full a little inside it, zero `0.65 * blur` out.
                    let shape = Shape::new(rect.translate(offset), radius, scale);
                    let blur = (blur * scale).max(0.01);
                    let reach = shape.r.max(0.35 * blur) + 1.0;
                    paint(band, width, cols, y0, y1, shape.bounds(0.65 * blur + 1.0), shape.middle(reach), color, |x, y| {
                        let t = 1.0 - (shape.distance(x, y) / blur + 0.35).clamp(0.0, 1.0);
                        t * t
                    });
                }
                DrawCmd::Gradient { rect, radius, from, to, start, end } => {
                    let shape = Shape::new(rect, radius, scale);
                    paint_gradient(band, width, cols, y0, y1, &shape, (from * scale, to * scale), start, end);
                }
                DrawCmd::Polygon { start, len, color } => {
                    fill_polygon(band, width, cols, y0, y1, list.points(start, len), scale, color);
                }
                DrawCmd::Line { from, to, width: stroke_width, color } => {
                    // A stroke with round ends is a capsule; filling it with the
                    // row-by-row rasterizer keeps partial repaints exact.
                    fill_polygon(band, width, cols, y0, y1, &capsule(from, to, stroke_width * 0.5), scale, color);
                }
                DrawCmd::Image { rect, radius, image, uv, tint, .. } => {
                    if let Some(data) = list.image(image) {
                        paint_image(band, width, cols, y0, y1, Shape::new(rect, radius, scale), rect, uv, data, tint, scale);
                    }
                }
                _ => {}
            }
        }
    }

    /// Blends one line of text into `rows` (which start at device row
    /// `first`), limited to the rows `band` and the columns `cols`.
    #[allow(clippy::too_many_arguments)]
    fn blit_text(
        &self,
        rows: &mut [u8],
        width: u32,
        first: u32,
        band: (u32, u32),
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: Color,
        cols: (i32, i32),
        style: TextStyle,
    ) {
        let baseline = self.fonts.baseline(style, y, size, LINE_HEIGHT);
        let (_, fake_bold) = self.fonts.pick(style);
        let passes: &[f32] = if fake_bold { &[0.0, 1.0] } else { &[0.0] };

        let pw = width as i32;
        let (x0, x1) = cols;
        let (y0, y1) = (band.0 as i32, band.1 as i32);
        if x0 >= x1 {
            return;
        }
        let [r, g, b, _] = color.to_rgba8();
        let alpha = (color.a.clamp(0.0, 1.0) * 256.0) as u32;
        let size_key = (size * 8.0).round() as u32;
        let first = first as i32;
        self.fonts.walk(text, size, style, x, |face, id, pen| {
            for &k in passes {
                let pen = pen + k * bold_offset(size);
                let sub = ((pen - pen.floor()) * 4.0) as u8;
                let Some(Some(gl)) = self.glyphs.get(&(id.0, size_key, sub, face)) else { continue };
                let gx0 = pen.floor() as i32 + gl.left;
                let gy0 = baseline + gl.top;
                for row in (y0 - gy0).max(0)..(y1 - gy0).min(gl.h) {
                    let dst_row = ((gy0 + row - first) * pw) as usize;
                    let src_row = (row * gl.w) as usize;
                    for col in (x0 - gx0).max(0)..(x1 - gx0).min(gl.w) {
                        let c = gl.coverage[src_row + col as usize] as u32;
                        if c == 0 {
                            continue;
                        }
                        let a = (c * alpha) >> 8; // 0..=255
                        let inv = 255 - a;
                        let i = (dst_row + (gx0 + col) as usize) * 4;
                        let d = &mut rows[i..i + 4];
                        d[0] = ((r as u32 * a + d[0] as u32 * inv) / 255) as u8;
                        d[1] = ((g as u32 * a + d[1] as u32 * inv) / 255) as u8;
                        d[2] = ((b as u32 * a + d[2] as u32 * inv) / 255) as u8;
                        d[3] = ((255 * a + d[3] as u32 * inv) / 255) as u8;
                    }
                }
            }
        });
    }
}

/// Vertical extent of a command in logical pixels, with a safety margin.
/// `None` for commands that paint nothing themselves.
fn extent(cmd: &DrawCmd, points: &[Vec2]) -> Option<(f32, f32)> {
    Some(match *cmd {
        DrawCmd::Polygon { start, len, .. } => {
            let (y0, y1) = points[start as usize..(start + len) as usize].iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
            (y0 - 1.0, y1 + 1.0)
        }
        DrawCmd::Rect { rect, stroke_width, .. } => (rect.y - stroke_width - 1.0, rect.bottom() + stroke_width + 1.0),
        DrawCmd::Image { rect, .. } | DrawCmd::Gradient { rect, .. } => (rect.y - 1.0, rect.bottom() + 1.0),
        DrawCmd::Shadow { rect, blur, offset, .. } => (rect.y + offset.y - blur - 1.0, rect.bottom() + offset.y + blur + 1.0),
        DrawCmd::Text { pos, size, .. } => (pos.y - size * 0.4, pos.y + size * 1.8),
        DrawCmd::Line { from, to, width, .. } => (from.y.min(to.y) - width - 1.0, from.y.max(to.y) + width + 1.0),
        // A clip that changes exposes or hides whatever is inside it.
        DrawCmd::PushClip { rect, .. } => (rect.y - 1.0, rect.bottom() + 1.0),
        _ => return None,
    })
}

/// Whether `cmd` can paint anything in device rows `first..end`.
fn touches(cmd: &DrawCmd, points: &[Vec2], scale: f32, first: u32, end: u32) -> bool {
    extent(cmd, points).is_none_or(|(y0, y1)| (y1 * scale).ceil() + 1.0 >= first as f32 && (y0 * scale).floor() - 1.0 <= end as f32)
}

fn sk_color(c: Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba(c.r.clamp(0.0, 1.0), c.g.clamp(0.0, 1.0), c.b.clamp(0.0, 1.0), c.a.clamp(0.0, 1.0))
        .unwrap_or(tiny_skia::Color::TRANSPARENT)
}

/// A rounded rectangle in device pixels, as a signed distance field:
/// negative inside, positive outside, in pixels from the boundary.
#[derive(Clone, Copy)]
struct Shape {
    cx: f32,
    cy: f32,
    /// Half size.
    hw: f32,
    hh: f32,
    r: f32,
}

impl Shape {
    fn new(rect: Rect, radius: f32, scale: f32) -> Shape {
        let (hw, hh) = (rect.w * scale * 0.5, rect.h * scale * 0.5);
        Shape { cx: rect.x * scale + hw, cy: rect.y * scale + hh, hw, hh, r: (radius * scale).min(hw).min(hh).max(0.0) }
    }

    fn empty(&self) -> bool {
        self.hw <= 0.0 || self.hh <= 0.0
    }

    #[inline(always)]
    fn distance(&self, x: f32, y: f32) -> f32 {
        let qx = (x - self.cx).abs() - (self.hw - self.r);
        let qy = (y - self.cy).abs() - (self.hh - self.r);
        let (ox, oy) = (qx.max(0.0), qy.max(0.0));
        (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - self.r
    }

    /// (x0, y0, x1, y1) grown by `margin`.
    fn bounds(&self, margin: f32) -> (f32, f32, f32, f32) {
        (self.cx - self.hw - margin, self.cy - self.hh - margin, self.cx + self.hw + margin, self.cy + self.hh + margin)
    }

    /// The span of x, `reach` away from both ends, where the distance (down
    /// to `-reach`) depends on the row alone.
    fn middle(&self, reach: f32) -> (f32, f32) {
        (self.cx - self.hw + reach, self.cx + self.hw - reach)
    }
}

/// Anti-aliased coverage of a pixel whose center is `d` from the boundary.
#[inline(always)]
fn edge(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

/// Blends `color` into the rows `y0..y1` held by `band`, weighted by
/// `coverage(x, y)` at each pixel center.
///
/// Coverage is evaluated per pixel only outside `middle`; inside that span
/// it is taken once per row and the run is filled in one go, so a large
/// shape costs its outline plus a plain fill. Being a pure function of the
/// pixel position, the result is the same however the frame is split into
/// bands.
#[allow(clippy::too_many_arguments)]
fn paint(
    band: &mut [u8],
    width: u32,
    cols: (i32, i32),
    y0: u32,
    y1: u32,
    bounds: (f32, f32, f32, f32),
    middle: (f32, f32),
    color: Color,
    coverage: impl Fn(f32, f32) -> f32,
) {
    let x_start = (bounds.0.floor() as i32).clamp(cols.0, cols.1);
    let x_end = (bounds.2.ceil() as i32).clamp(cols.0, cols.1);
    let row_start = (bounds.1.floor().max(0.0) as u32).max(y0);
    let row_end = (bounds.3.ceil().max(0.0) as u32).min(y1);
    if x_start >= x_end || row_start >= row_end {
        return;
    }
    let (mut mid0, mut mid1) = ((middle.0.ceil() as i32).clamp(x_start, x_end), (middle.1.floor() as i32).clamp(x_start, x_end));
    if mid1 <= mid0 {
        // Too narrow for a middle: every pixel is evaluated.
        mid0 = x_end;
        mid1 = x_end;
    }
    let mid_x = (middle.0 + middle.1) * 0.5;
    let [r, g, b, _] = color.to_rgba8();
    let alpha = color.a.clamp(0.0, 1.0) * 255.0;
    let stride = width as usize * 4;
    for row in row_start..row_end {
        let y = row as f32 + 0.5;
        let line = &mut band[(row - y0) as usize * stride..][..stride];
        for x in x_start..mid0 {
            blend(&mut line[x as usize * 4..][..4], r, g, b, (alpha * coverage(x as f32 + 0.5, y) + 0.5) as u32);
        }
        if mid1 > mid0 {
            let a = (alpha * coverage(mid_x, y) + 0.5) as u32;
            let run = &mut line[mid0 as usize * 4..mid1 as usize * 4];
            if a >= 255 {
                for px in run.as_chunks_mut::<4>().0 {
                    *px = [r, g, b, 255];
                }
            } else if a > 0 {
                // Source premultiplied once for the whole run.
                let (sr, sg, sb, inv) = (r as u32 * a, g as u32 * a, b as u32 * a, 255 - a);
                for px in run.as_chunks_mut::<4>().0 {
                    px[0] = ((sr + px[0] as u32 * inv + 127) / 255) as u8;
                    px[1] = ((sg + px[1] as u32 * inv + 127) / 255) as u8;
                    px[2] = ((sb + px[2] as u32 * inv + 127) / 255) as u8;
                    px[3] = ((255 * a + px[3] as u32 * inv + 127) / 255) as u8;
                }
            }
        }
        for x in mid1..x_end {
            blend(&mut line[x as usize * 4..][..4], r, g, b, (alpha * coverage(x as f32 + 0.5, y) + 0.5) as u32);
        }
    }
}

/// Fills `shape` with a linear gradient from `start` at `ends.0` to `end` at
/// `ends.1` (device pixels), into the rows `y0..y1` held by `band`.
#[allow(clippy::too_many_arguments)]
fn paint_gradient(band: &mut [u8], width: u32, cols: (i32, i32), y0: u32, y1: u32, shape: &Shape, ends: (Vec2, Vec2), start: Color, end: Color) {
    let (x_lo, y_lo, x_hi, y_hi) = shape.bounds(1.0);
    let x_start = (x_lo.floor() as i32).clamp(cols.0, cols.1);
    let x_end = (x_hi.ceil() as i32).clamp(cols.0, cols.1);
    let row_start = (y_lo.floor().max(0.0) as u32).max(y0);
    let row_end = (y_hi.ceil().max(0.0) as u32).min(y1);
    let d = ends.1 - ends.0;
    let inv = 1.0 / (d.x * d.x + d.y * d.y).max(1e-6);
    let stride = width as usize * 4;
    for row in row_start..row_end {
        let y = row as f32 + 0.5;
        let line = &mut band[(row - y0) as usize * stride..][..stride];
        for x in x_start.max(0)..x_end {
            let xf = x as f32 + 0.5;
            let cover = edge(shape.distance(xf, y));
            if cover <= 0.0 {
                continue;
            }
            let t = (((xf - ends.0.x) * d.x + (y - ends.0.y) * d.y) * inv).clamp(0.0, 1.0);
            let c = start.mix(end, t);
            let [r, g, b, _] = c.to_rgba8();
            blend(&mut line[x as usize * 4..][..4], r, g, b, (c.a.clamp(0.0, 1.0) * 255.0 * cover + 0.5) as u32);
        }
    }
}

/// Fills a polygon (non-zero winding) into the rows `y0..y1` held by `band`.
///
/// Each row is computed on its own, from four sub-scanlines with exact
/// horizontal coverage, so the pixels never depend on how the frame is cut
/// into bands: partial repaints match full ones exactly.
#[allow(clippy::too_many_arguments)]
fn fill_polygon(band: &mut [u8], width: u32, cols: (i32, i32), y0: u32, y1: u32, pts: &[Vec2], scale: f32, color: Color) {
    const SUB: usize = 4;
    if pts.len() < 3 {
        return;
    }
    let dev: Vec<(f32, f32)> = pts.iter().map(|p| (p.x * scale, p.y * scale)).collect();
    let (mut min_y, mut max_y, mut min_x, mut max_x) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for &(x, y) in &dev {
        (min_y, max_y, min_x, max_x) = (min_y.min(y), max_y.max(y), min_x.min(x), max_x.max(x));
    }
    let row_start = (min_y.floor().max(0.0) as u32).max(y0);
    let row_end = (max_y.ceil().max(0.0) as u32).min(y1);
    let x_start = (min_x.floor() as i32).clamp(cols.0, cols.1) as usize;
    let x_end = (max_x.ceil() as i32).clamp(cols.0, cols.1) as usize;
    if row_start >= row_end || x_start >= x_end {
        return;
    }
    let [r, g, b, _] = color.to_rgba8();
    let alpha = color.a.clamp(0.0, 1.0) * 255.0;
    let stride = width as usize * 4;
    let mut cover = vec![0.0f32; x_end - x_start];
    let mut crossings: Vec<(f32, i32)> = Vec::with_capacity(16);
    for row in row_start..row_end {
        cover.iter_mut().for_each(|c| *c = 0.0);
        for s in 0..SUB {
            let sy = row as f32 + (s as f32 + 0.5) / SUB as f32;
            crossings.clear();
            for i in 0..dev.len() {
                let (a, b) = (dev[i], dev[(i + 1) % dev.len()]);
                if (a.1 <= sy) != (b.1 <= sy) {
                    let x = a.0 + (sy - a.1) * (b.0 - a.0) / (b.1 - a.1);
                    crossings.push((x, if b.1 > a.1 { 1 } else { -1 }));
                }
            }
            crossings.sort_by(|p, q| p.0.total_cmp(&q.0));
            let (mut winding, mut open) = (0, 0.0);
            for &(x, dir) in &crossings {
                let before = winding;
                winding += dir;
                if before == 0 && winding != 0 {
                    open = x;
                } else if before != 0 && winding == 0 {
                    add_span(&mut cover, x_start, open, x, 1.0 / SUB as f32);
                }
            }
        }
        let line = &mut band[(row - y0) as usize * stride..][..stride];
        for (i, &c) in cover.iter().enumerate() {
            if c > 0.0 {
                let x = x_start + i;
                blend(&mut line[x * 4..][..4], r, g, b, (alpha * c.min(1.0) + 0.5) as u32);
            }
        }
    }
}

/// The outline of a stroke from `a` to `b` with round ends of radius `r`.
fn capsule(a: Vec2, b: Vec2, r: f32) -> Vec<Vec2> {
    const CAP: usize = 8;
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let base = if dx == 0.0 && dy == 0.0 { 0.0 } else { dy.atan2(dx) };
    let mut pts = Vec::with_capacity(2 * CAP + 2);
    for (c, from) in [(b, base - FRAC_PI_2), (a, base + FRAC_PI_2)] {
        for i in 0..=CAP {
            let t = from + PI * i as f32 / CAP as f32;
            pts.push(Vec2::new(c.x + t.cos() * r, c.y + t.sin() * r));
        }
    }
    pts
}

/// Adds `weight` times the coverage of the horizontal span `x0..x1` to
/// `cover` (whose first cell is pixel column `origin`).
fn add_span(cover: &mut [f32], origin: usize, x0: f32, x1: f32, weight: f32) {
    let (x0, x1) = (x0 - origin as f32, x1 - origin as f32);
    let (x0, x1) = (x0.max(0.0), x1.min(cover.len() as f32));
    if x1 <= x0 {
        return;
    }
    let (first, last) = (x0.floor() as usize, (x1.ceil() as usize).min(cover.len()));
    for (i, c) in cover.iter_mut().enumerate().take(last).skip(first) {
        let (p0, p1) = (i as f32, i as f32 + 1.0);
        *c += (x1.min(p1) - x0.max(p0)).max(0.0) * weight;
    }
}

/// Paints the part `uv` of image `data` into `rect` (logical pixels), with
/// the rounded corners of `shape` (device pixels), multiplied by `tint`,
/// limited to rows `y0..y1` and columns `cols`. Bilinear sampling, done on
/// premultiplied colors so transparent edges do not darken; each pixel
/// depends on nothing but its own position, so partial repaints match.
#[allow(clippy::too_many_arguments)]
fn paint_image(
    band: &mut [u8],
    width: u32,
    cols: (i32, i32),
    y0: u32,
    y1: u32,
    shape: Shape,
    rect: Rect,
    uv: Rect,
    data: &morfo::ImageData,
    tint: Color,
    scale: f32,
) {
    let (x0, top, x1, bottom) = shape.bounds(1.0);
    let x_start = (x0.floor() as i32).clamp(cols.0, cols.1);
    let x_end = (x1.ceil() as i32).clamp(cols.0, cols.1);
    let row_start = (top.floor().max(0.0) as u32).max(y0);
    let row_end = (bottom.ceil().max(0.0) as u32).min(y1);
    if x_start >= x_end || row_start >= row_end || rect.w <= 0.0 || rect.h <= 0.0 {
        return;
    }
    let (iw, ih) = (data.width as f32, data.height as f32);
    // Image pixels per device pixel, both ways.
    let (du, dv) = (uv.w * iw / (rect.w * scale), uv.h * ih / (rect.h * scale));
    let (u0, v0) = (uv.x * iw - rect.x * scale * du, uv.y * ih - rect.y * scale * dv);
    let t = [tint.r.clamp(0.0, 1.0), tint.g.clamp(0.0, 1.0), tint.b.clamp(0.0, 1.0), tint.a.clamp(0.0, 1.0)];
    let stride = width as usize * 4;
    for row in row_start..row_end {
        let y = row as f32 + 0.5;
        let sy = v0 + y * dv - 0.5;
        let (fy, ty) = (sy.floor(), sy - sy.floor());
        let line = &mut band[(row - y0) as usize * stride..][..stride];
        for x in x_start..x_end {
            let px = x as f32 + 0.5;
            let cover = edge(shape.distance(px, y));
            if cover <= 0.0 {
                continue;
            }
            let sx = u0 + px * du - 0.5;
            let (fx, tx) = (sx.floor(), sx - sx.floor());
            // Premultiplied bilinear blend of the four texels around.
            let mut acc = [0.0f32; 4];
            for (dx, dy, w) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
                let p = data.pixel(fx as i32 + dx, fy as i32 + dy);
                let a = p[3] as f32 * w;
                acc[0] += p[0] as f32 * a;
                acc[1] += p[1] as f32 * a;
                acc[2] += p[2] as f32 * a;
                acc[3] += a;
            }
            let alpha = acc[3] / 255.0 * t[3] * cover;
            if alpha <= 0.0 {
                continue;
            }
            // Premultiplied source over the premultiplied pixel.
            let k = t[3] * cover / 255.0;
            let src = [acc[0] * t[0] * k, acc[1] * t[1] * k, acc[2] * t[2] * k];
            let inv = 1.0 - alpha.min(1.0);
            let d = &mut line[x as usize * 4..][..4];
            d[0] = (src[0] + d[0] as f32 * inv + 0.5).min(255.0) as u8;
            d[1] = (src[1] + d[1] as f32 * inv + 0.5).min(255.0) as u8;
            d[2] = (src[2] + d[2] as f32 * inv + 0.5).min(255.0) as u8;
            d[3] = (alpha.min(1.0) * 255.0 + d[3] as f32 * inv + 0.5).min(255.0) as u8;
        }
    }
}

/// Source-over of a straight-alpha color onto a premultiplied pixel.
#[inline(always)]
fn blend(px: &mut [u8], r: u8, g: u8, b: u8, a: u32) {
    if a == 0 {
        return;
    }
    let (a, inv) = (a.min(255), 255 - a.min(255));
    px[0] = ((r as u32 * a + px[0] as u32 * inv + 127) / 255) as u8;
    px[1] = ((g as u32 * a + px[1] as u32 * inv + 127) / 255) as u8;
    px[2] = ((b as u32 * a + px[2] as u32 * inv + 127) / 255) as u8;
    px[3] = ((255 * a + px[3] as u32 * inv + 127) / 255) as u8;
}

#[cfg(test)]
mod image_tests {
    use super::*;
    use morfo::{Fit, Theme, Ui};

    /// A 2×2 image (red, green / blue, white) stretched over a square: each
    /// quarter shows its color, and repainting only part of the frame gives
    /// the same pixels as painting all of it.
    #[test]
    fn images_are_painted_and_partial_repaints_match() {
        let paths = [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        ];
        let Some(font) = paths.iter().find_map(|p| std::fs::read(p).ok()) else { return };
        let mut ui = Ui::new(Theme::dark());
        let px = vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255];
        let id = ui.add_image(2, 2, px).unwrap();
        let mut t = 0.0;
        let mut frame = |ui: &mut Ui, x: f32| {
            t += 1.0;
            ui.set_instant(true);
            ui.begin(t, Vec2::new(200.0, 200.0));
            ui.space(x);
            ui.image("i", id, Vec2::new(80.0, 80.0), Fit::Fill, "");
            ui.end();
        };
        for _ in 0..3 {
            frame(&mut ui, 0.0);
        }
        let mut renderer = Renderer::new(font.clone()).unwrap();
        let mut pixmap = Pixmap::new(200, 200).unwrap();
        renderer.render(ui.draw_list(), &mut pixmap, 1.0, Color::BLACK);
        let r = ui.draw_list().commands().iter().find_map(|c| if let DrawCmd::Image { rect, .. } = *c { Some(rect) } else { None }).unwrap();
        let at = |p: &Pixmap, x: f32, y: f32| {
            let i = ((y as usize) * 200 + x as usize) * 4;
            [p.data()[i], p.data()[i + 1], p.data()[i + 2]]
        };
        let q = |fx: f32, fy: f32| (r.x + r.w * fx, r.y + r.h * fy);
        let (x, y) = q(0.12, 0.12);
        assert_eq!(at(&pixmap, x, y), [255, 0, 0], "top left is red");
        let (x, y) = q(0.88, 0.12);
        assert_eq!(at(&pixmap, x, y), [0, 255, 0], "top right is green");
        let (x, y) = q(0.12, 0.88);
        assert_eq!(at(&pixmap, x, y), [0, 0, 255], "bottom left is blue");
        let (x, y) = q(0.5, 0.5);
        let mid = at(&pixmap, x, y);
        assert!(mid.iter().all(|&c| c > 60 && c < 200), "the middle blends the four: {mid:?}");
        // Moved down: the partial repaint must equal a full one.
        frame(&mut ui, 40.0);
        frame(&mut ui, 40.0);
        renderer.render(ui.draw_list(), &mut pixmap, 1.0, Color::BLACK);
        let mut full = Pixmap::new(200, 200).unwrap();
        Renderer::new(font).unwrap().render(ui.draw_list(), &mut full, 1.0, Color::BLACK);
        assert!(pixmap.data() == full.data(), "partial repaint differs from a full one");
    }
}

#[cfg(test)]
mod style_tests {
    use super::*;
    use morfo::{DrawList, TextStyle};

    /// Ink (summed coverage) of `text` drawn in `style`.
    fn ink(renderer: &mut Renderer, style: TextStyle, text: &str) -> u64 {
        let mut list = DrawList::default();
        list.add_text_styled(Vec2::new(4.0, 4.0), 20.0, Color::WHITE, text, style);
        let mut pixmap = Pixmap::new(320, 40).unwrap();
        renderer.invalidate();
        renderer.render(&list, &mut pixmap, 1.0, Color::BLACK);
        pixmap.data().as_chunks::<4>().0.iter().map(|p| p[0] as u64).sum()
    }

    /// Bold text has more ink than regular, with or without a bold face;
    /// a character the main face lacks is drawn from a fallback.
    #[test]
    fn bold_is_heavier_and_fallbacks_fill_gaps() {
        let read = |paths: &[&str]| paths.iter().find_map(|p| std::fs::read(p).ok());
        let Some(regular) = read(&["/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"]) else { return };
        let mut renderer = Renderer::new(regular).unwrap();
        let plain = ink(&mut renderer, TextStyle::REGULAR, "Heavy words");
        let faked = ink(&mut renderer, TextStyle::BOLD, "Heavy words");
        assert!(faked > plain + plain / 10, "fake bold {faked} vs {plain}");
        // Liberation has no box-drawing heavy check mark; DejaVu does.
        let gap = ink(&mut renderer, TextStyle::REGULAR, "\u{2714}");
        let Some(dejavu) = read(&["/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "/usr/share/fonts/TTF/DejaVuSans.ttf"]) else { return };
        renderer.add_fallback_font(dejavu).unwrap();
        let filled = ink(&mut renderer, TextStyle::REGULAR, "\u{2714}");
        assert!(filled > gap, "the fallback draws it: {filled} vs {gap}");
    }
}
