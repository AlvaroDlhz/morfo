//! CPU renderer for [`morfo`]: paints a [`DrawList`] into a `tiny-skia`
//! pixmap. No GPU, no windowing assumptions — hand the pixels to anything
//! (softbuffer, a canvas, a PNG, a framebuffer).
//!
//! Built to do as little as possible: only the rows that changed since the
//! previous frame are repainted, in parallel strips when there are many,
//! and glyph bitmaps are cached between frames.

use std::collections::HashMap;

use ab_glyph::{point, Font, FontArc, GlyphId, PxScale, ScaleFont};
use morfo::ui::LINE_HEIGHT;
use morfo::{Color, DrawCmd, DrawList, Rect, TextMeasure};
use tiny_skia::{LineCap, Paint, PathBuilder, Pixmap, PixmapMut, Stroke, Transform};

pub use ab_glyph::InvalidFont;
pub use tiny_skia;

/// Text metrics matching what [`Renderer`] draws. Give it to
/// [`morfo::Ui::set_text_measure`].
pub struct FontMeasure(FontArc);

fn px_scale(font: &FontArc, size: f32) -> PxScale {
    // ab_glyph scales by ascent-to-descent height; `size` is the em size.
    PxScale::from(size * font.height_unscaled() / font.units_per_em().unwrap_or(1000.0))
}

impl TextMeasure for FontMeasure {
    fn width(&self, text: &str, size: f32) -> f32 {
        let f = self.0.as_scaled(px_scale(&self.0, size));
        let mut w = 0.0;
        let mut prev: Option<GlyphId> = None;
        for ch in text.chars() {
            let g = f.glyph_id(ch);
            if let Some(p) = prev {
                w += f.kern(p, g);
            }
            w += f.h_advance(g);
            prev = Some(g);
        }
        w
    }
}

struct Glyph {
    w: i32,
    h: i32,
    left: i32,
    top: i32,
    coverage: Vec<u8>,
}

/// (glyph, size in 1/8 px, horizontal subpixel in 1/4 px, monospaced)
type GlyphKey = (u16, u32, u8, bool);

/// What the previous frame was painted with; if any of it changes,
/// everything is repainted.
type Target = (u32, u32, u32, [u8; 4]);

/// Deepest clip nesting painted exactly; deeper clips reuse the last one.
const MAX_CLIPS: usize = 16;

pub struct Renderer {
    font: FontArc,
    mono: Option<FontArc>,
    glyphs: HashMap<GlyphKey, Option<Glyph>>,
    /// Last frame's commands and text, to repaint only what changed.
    prev: Vec<DrawCmd>,
    prev_text: String,
    prev_target: Option<Target>,
    threads: usize,
}

impl Renderer {
    /// `font` is the bytes of a TrueType/OpenType font.
    pub fn new(font: Vec<u8>) -> Result<Renderer, InvalidFont> {
        Ok(Renderer {
            font: FontArc::try_from_vec(font)?,
            mono: None,
            glyphs: HashMap::new(),
            prev: Vec::new(),
            prev_text: String::new(),
            prev_target: None,
            threads: std::thread::available_parallelism().map_or(1, |n| n.get()).min(8),
        })
    }

    /// Font used for code. Without one, code is drawn in the main font.
    pub fn set_mono_font(&mut self, font: Vec<u8>) -> Result<(), InvalidFont> {
        self.mono = Some(FontArc::try_from_vec(font)?);
        self.glyphs.clear();
        self.invalidate();
        Ok(())
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

    pub fn measure(&self) -> FontMeasure {
        FontMeasure(self.font.clone())
    }

    /// Paints `list` over `background`. `scale` is the device pixel ratio.
    ///
    /// The pixmap is expected to still hold the previous frame: only the
    /// rows whose content changed are repainted, split between threads when
    /// there are enough of them. Returns the repainted rows `(first, end)`,
    /// or `None` when the frame is identical to the last one and nothing was
    /// touched.
    ///
    /// Clips are applied exactly to text, and to shapes as a band of rows
    /// (which is what vertical reveals and scrolling need) — that keeps
    /// clipping free: no mask is ever allocated or sampled.
    pub fn render(&mut self, list: &DrawList, pixmap: &mut Pixmap, scale: f32, background: Color) -> Option<(u32, u32)> {
        let (width, height) = (pixmap.width(), pixmap.height());
        let target: Target = (width, height, scale.to_bits(), background.to_rgba8());
        let rows = if self.prev_target == Some(target) {
            self.damage(list).map(|(y0, y1)| {
                let first = ((y0 * scale).floor() - 1.0).max(0.0) as u32;
                let end = ((y1 * scale).ceil() + 1.0).max(0.0) as u32;
                (first.min(height), end.min(height))
            })
        } else {
            Some((0, height))
        };
        self.prev.clear();
        self.prev.extend_from_slice(list.commands());
        self.prev_text.clear();
        self.prev_text.push_str(list.text_arena());
        self.prev_target = Some(target);
        let (first, end) = rows.filter(|(a, b)| b > a)?;

        if self.glyphs.len() > 4096 {
            self.glyphs.clear();
        }
        self.prepare_glyphs(list, scale, first, end);

        let stride = width as usize * 4;
        let total = (end - first) as usize;
        let data = &mut pixmap.data_mut()[first as usize * stride..end as usize * stride];
        let strips = if total * (width as usize) < 120_000 { 1 } else { self.threads.min(total / 32).max(1) };
        let this = &*self;
        if strips == 1 {
            this.paint_rows(list, data, width, first, end, scale, background);
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
                        this.paint_rows(list, strip, width, y0, y1, scale, background);
                    } else {
                        scope.spawn(move || this.paint_rows(list, strip, width, y0, y1, scale, background));
                    }
                }
            });
        }
        Some((first, end))
    }

    /// Vertical extent (logical pixels) of everything that differs from the
    /// previous frame.
    fn damage(&self, list: &DrawList) -> Option<(f32, f32)> {
        let (old, new) = (&self.prev[..], list.commands());
        let same = |a: &DrawCmd, b: &DrawCmd| match (*a, *b) {
            (
                DrawCmd::Text { pos, size, color, start, len, mono },
                DrawCmd::Text { pos: pos2, size: size2, color: color2, start: start2, len: len2, mono: mono2 },
            ) => {
                pos == pos2
                    && size == size2
                    && color == color2
                    && mono == mono2
                    && self.prev_text[start as usize..(start + len) as usize] == *list.text(start2, len2)
            }
            _ => a == b,
        };
        let mut span: Option<(f32, f32)> = None;
        let mut add = |cmd: &DrawCmd| {
            if let Some((a, b)) = extent(cmd) {
                span = Some(span.map_or((a, b), |(x, y)| (x.min(a), y.max(b))));
            }
        };
        if old.len() == new.len() {
            for (a, b) in old.iter().zip(new) {
                if !same(a, b) {
                    add(a);
                    add(b);
                }
            }
        } else {
            // Something was added or removed: everything between the common
            // start and the common end is suspect.
            let shorter = old.len().min(new.len());
            let head = (0..shorter).take_while(|&i| same(&old[i], &new[i])).count();
            let tail = (0..shorter - head).take_while(|&i| same(&old[old.len() - 1 - i], &new[new.len() - 1 - i])).count();
            old[head..old.len() - tail].iter().chain(&new[head..new.len() - tail]).for_each(&mut add);
        }
        span
    }

    /// Rasterizes the glyphs the rows `first..end` will need, so painting
    /// can then share the cache between threads without locking.
    fn prepare_glyphs(&mut self, list: &DrawList, scale: f32, first: u32, end: u32) {
        for cmd in list.commands() {
            let DrawCmd::Text { pos, size, start, len, mono, .. } = *cmd else { continue };
            if !touches(cmd, scale, first, end) {
                continue;
            }
            let mono = mono && self.mono.is_some();
            let font = if mono { self.mono.clone().unwrap() } else { self.font.clone() };
            let px = px_scale(&font, size * scale);
            let size_key = (size * scale * 8.0).round() as u32;
            let glyphs = &mut self.glyphs;
            walk(&font, px, list.text(start, len), pos.x * scale, |id, pen| {
                let sub = ((pen - pen.floor()) * 4.0) as u8;
                glyphs.entry((id.0, size_key, sub, mono)).or_insert_with(|| {
                    // Rasterize at the size the key stands for, not at the
                    // exact size of whoever asks first: the same key always
                    // yields the same bitmap.
                    let px = px_scale(&font, size_key as f32 / 8.0);
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
            });
        }
    }

    /// Paints device rows `first..end` of the frame into `rows` (exactly
    /// those rows of the pixmap). Independent of every other row.
    #[allow(clippy::too_many_arguments)]
    fn paint_rows(&self, list: &DrawList, rows: &mut [u8], width: u32, first: u32, end: u32, scale: f32, background: Color) {
        let stride = width as usize * 4;
        if let Some(mut all) = PixmapMut::from_bytes(rows, width, end - first) {
            all.fill(sk_color(background));
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
            // Rows of this strip visible through the current clip.
            let clip = (depth > 0).then(|| clips[depth.min(MAX_CLIPS) - 1]);
            let (y0, y1) = match clip {
                Some(c) => (
                    ((c.y * scale).round().max(0.0) as u32).clamp(first, end),
                    ((c.bottom() * scale).round().max(0.0) as u32).clamp(first, end),
                ),
                None => (first, end),
            };
            if y1 <= y0 || !touches(cmd, scale, y0, y1) {
                continue;
            }
            if let DrawCmd::Text { pos, size, color, start, len, mono } = *cmd {
                self.blit_text(rows, width, first, (y0, y1), list.text(start, len), pos.x * scale, pos.y * scale, size * scale, color, clip, scale, mono);
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
                        paint(band, width, y0, y1, shape.bounds(1.0), shape.middle(reach), fill, |x, y| edge(shape.distance(x, y)));
                    }
                    let w = stroke_width * scale;
                    if w > 0.0 && stroke.a > 0.002 {
                        let inner = Shape::new(rect.inset(stroke_width), radius - stroke_width, scale);
                        let reach = shape.r.max(w) + 1.0;
                        paint(band, width, y0, y1, shape.bounds(1.0), shape.middle(reach), stroke, |x, y| {
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
                    paint(band, width, y0, y1, shape.bounds(0.65 * blur + 1.0), shape.middle(reach), color, |x, y| {
                        let t = 1.0 - (shape.distance(x, y) / blur + 0.35).clamp(0.0, 1.0);
                        t * t
                    });
                }
                DrawCmd::Line { from, to, width: stroke_width, color } => {
                    let Some(mut band) = PixmapMut::from_bytes(band, width, y1 - y0) else { continue };
                    let ts = Transform::from_scale(scale, scale).post_translate(0.0, -(y0 as f32));
                    let mut pb = PathBuilder::new();
                    pb.move_to(from.x, from.y);
                    pb.line_to(to.x, to.y);
                    let Some(path) = pb.finish() else { continue };
                    let s = Stroke { width: stroke_width, line_cap: LineCap::Round, ..Stroke::default() };
                    let mut p = Paint::default();
                    p.set_color(sk_color(color));
                    p.anti_alias = true;
                    band.stroke_path(&path, &p, &s, ts, None);
                }
                _ => {}
            }
        }
    }

    /// Blends one line of text into `rows` (which start at device row
    /// `first`), limited to the rows `band` and to `clip` horizontally.
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
        clip: Option<Rect>,
        scale: f32,
        mono: bool,
    ) {
        let mono = mono && self.mono.is_some();
        let font = if mono { self.mono.as_ref().unwrap() } else { &self.font };
        let px = px_scale(font, size);
        let f = font.as_scaled(px);
        let baseline = (y + (size * LINE_HEIGHT - (f.ascent() - f.descent())) * 0.5 + f.ascent()).round() as i32;

        let pw = width as i32;
        let (mut x0, mut x1) = (0, pw);
        let (y0, y1) = (band.0 as i32, band.1 as i32);
        if let Some(c) = clip {
            x0 = x0.max((c.x * scale).floor() as i32);
            x1 = x1.min((c.right() * scale).ceil() as i32);
        }
        if x0 >= x1 {
            return;
        }
        let [r, g, b, _] = color.to_rgba8();
        let alpha = (color.a.clamp(0.0, 1.0) * 256.0) as u32;
        let size_key = (size * 8.0).round() as u32;
        let first = first as i32;
        walk(font, px, text, x, |id, pen| {
            let sub = ((pen - pen.floor()) * 4.0) as u8;
            let Some(Some(gl)) = self.glyphs.get(&(id.0, size_key, sub, mono)) else { return };
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
        });
    }
}

/// Calls `f(glyph, pen_x)` for each character of a line starting at `x`.
fn walk(font: &FontArc, px: PxScale, text: &str, x: f32, mut f: impl FnMut(GlyphId, f32)) {
    let scaled = font.as_scaled(px);
    let mut pen = x;
    let mut prev: Option<GlyphId> = None;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        if let Some(p) = prev {
            pen += scaled.kern(p, id);
        }
        prev = Some(id);
        f(id, pen);
        pen += scaled.h_advance(id);
    }
}

/// Vertical extent of a command in logical pixels, with a safety margin.
/// `None` for commands that paint nothing themselves.
fn extent(cmd: &DrawCmd) -> Option<(f32, f32)> {
    Some(match *cmd {
        DrawCmd::Rect { rect, stroke_width, .. } => (rect.y - stroke_width - 1.0, rect.bottom() + stroke_width + 1.0),
        DrawCmd::Shadow { rect, blur, offset, .. } => (rect.y + offset.y - blur - 1.0, rect.bottom() + offset.y + blur + 1.0),
        DrawCmd::Text { pos, size, .. } => (pos.y - size * 0.4, pos.y + size * 1.8),
        DrawCmd::Line { from, to, width, .. } => (from.y.min(to.y) - width - 1.0, from.y.max(to.y) + width + 1.0),
        // A clip that changes exposes or hides whatever is inside it.
        DrawCmd::PushClip { rect, .. } => (rect.y - 1.0, rect.bottom() + 1.0),
        DrawCmd::Nop | DrawCmd::PopClip => return None,
    })
}

/// Whether `cmd` can paint anything in device rows `first..end`.
fn touches(cmd: &DrawCmd, scale: f32, first: u32, end: u32) -> bool {
    extent(cmd).is_none_or(|(y0, y1)| (y1 * scale).ceil() + 1.0 >= first as f32 && (y0 * scale).floor() - 1.0 <= end as f32)
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
    y0: u32,
    y1: u32,
    bounds: (f32, f32, f32, f32),
    middle: (f32, f32),
    color: Color,
    coverage: impl Fn(f32, f32) -> f32,
) {
    let w = width as i32;
    let x_start = (bounds.0.floor() as i32).clamp(0, w);
    let x_end = (bounds.2.ceil() as i32).clamp(0, w);
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
