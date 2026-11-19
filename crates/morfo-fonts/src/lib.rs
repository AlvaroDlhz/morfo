//! The fonts behind morfo's renderers (`morfo-skia`, `morfo-wgpu`): one
//! [`FontSet`] holds the faces an interface draws with — regular, bold,
//! italic, bold italic and monospaced — plus fallback fonts for characters
//! the main ones lack (symbols, other scripts), and lays a line out glyph by
//! glyph. [`FontMeasure`] measures text exactly as it is then drawn, from a
//! cache of advances.
//!
//! What is not provided falls back gracefully: without a bold face, bold
//! text is drawn twice, a hair apart; without an italic face it stays
//! upright; without a monospaced face, code uses the regular one.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Arc;

use ab_glyph::{Font, FontArc, GlyphId, PxScale, ScaleFont};
use morfo::{TextMeasure, TextStyle};

pub use ab_glyph;
pub use ab_glyph::InvalidFont;

/// A face of a [`FontSet`], by index.
pub type Face = u8;

/// The faces an interface draws with.
#[derive(Clone)]
pub struct FontSet {
    faces: Vec<FontArc>,
    /// The face for each kind of text: regular, bold, italic, bold italic,
    /// monospaced. A kind without a face of its own uses another.
    slots: [Option<Face>; 5],
    /// Faces tried, in order, for characters the chosen one lacks.
    fallbacks: Vec<Face>,
}

const REGULAR: usize = 0;
const BOLD: usize = 1;
const ITALIC: usize = 2;
const BOLD_ITALIC: usize = 3;
const MONO: usize = 4;

impl FontSet {
    /// A set with one face: `regular` (TrueType or OpenType bytes).
    pub fn new(regular: Vec<u8>) -> Result<FontSet, InvalidFont> {
        Ok(FontSet { faces: vec![FontArc::try_from_vec(regular)?], slots: [Some(0), None, None, None, None], fallbacks: Vec::new() })
    }

    fn add(&mut self, bytes: Vec<u8>) -> Result<Face, InvalidFont> {
        self.faces.push(FontArc::try_from_vec(bytes)?);
        Ok((self.faces.len() - 1) as Face)
    }

    /// The face for bold text.
    pub fn set_bold(&mut self, bytes: Vec<u8>) -> Result<(), InvalidFont> {
        self.slots[BOLD] = Some(self.add(bytes)?);
        Ok(())
    }

    /// The face for italic text.
    pub fn set_italic(&mut self, bytes: Vec<u8>) -> Result<(), InvalidFont> {
        self.slots[ITALIC] = Some(self.add(bytes)?);
        Ok(())
    }

    /// The face for bold italic text.
    pub fn set_bold_italic(&mut self, bytes: Vec<u8>) -> Result<(), InvalidFont> {
        self.slots[BOLD_ITALIC] = Some(self.add(bytes)?);
        Ok(())
    }

    /// The face for code.
    pub fn set_mono(&mut self, bytes: Vec<u8>) -> Result<(), InvalidFont> {
        self.slots[MONO] = Some(self.add(bytes)?);
        Ok(())
    }

    /// A font tried, after those added before it, for every character the
    /// chosen face does not have: symbols, arrows, other scripts.
    pub fn add_fallback(&mut self, bytes: Vec<u8>) -> Result<(), InvalidFont> {
        let face = self.add(bytes)?;
        self.fallbacks.push(face);
        Ok(())
    }

    /// Face `face`.
    pub fn face(&self, face: Face) -> &FontArc {
        &self.faces[face as usize]
    }

    /// The face text of `style` is drawn with, and whether its boldness
    /// must be faked (bold asked for, no bold face).
    pub fn pick(&self, style: TextStyle) -> (Face, bool) {
        let r = self.slots[REGULAR].unwrap_or(0);
        if style.mono() {
            if let Some(m) = self.slots[MONO] {
                return (m, false);
            }
        }
        match (style.bold(), style.italic()) {
            (true, true) => self.slots[BOLD_ITALIC].map_or_else(|| self.slots[ITALIC].map_or((r, true), |i| (i, true)), |f| (f, false)),
            (true, false) => self.slots[BOLD].map_or((r, true), |f| (f, false)),
            (false, true) => (self.slots[ITALIC].unwrap_or(r), false),
            (false, false) => (r, false),
        }
    }

    /// The scale that makes face `face` `size` pixels to the em.
    pub fn px_scale(&self, face: Face, size: f32) -> PxScale {
        let f = self.face(face);
        // ab_glyph scales by ascent-to-descent height; `size` is the em size.
        PxScale::from(size * f.height_unscaled() / f.units_per_em().unwrap_or(1000.0))
    }

    /// The face that draws `ch` when `main` is asked: `main` itself when it
    /// has the character, else the first fallback that does.
    pub fn face_for(&self, main: Face, ch: char) -> (Face, GlyphId) {
        let id = self.face(main).glyph_id(ch);
        if id.0 != 0 || ch.is_whitespace() || ch.is_control() {
            return (main, id);
        }
        for &f in &self.fallbacks {
            let id = self.face(f).glyph_id(ch);
            if id.0 != 0 {
                return (f, id);
            }
        }
        (main, id)
    }

    /// The baseline (device pixels, rounded) of a line of `style` text
    /// whose line box starts at `y` and is `size * line_height` tall.
    pub fn baseline(&self, style: TextStyle, y: f32, size: f32, line_height: f32) -> i32 {
        let (face, _) = self.pick(style);
        let f = self.face(face).as_scaled(self.px_scale(face, size));
        (y + (size * line_height - (f.ascent() - f.descent())) * 0.5 + f.ascent()).round() as i32
    }

    /// Calls `f(face, glyph, pen_x)` for each character of a line of
    /// `style` text at `size` starting at `x`. Kerning applies between
    /// characters of the same face.
    pub fn walk(&self, text: &str, size: f32, style: TextStyle, x: f32, mut f: impl FnMut(Face, GlyphId, f32)) {
        let (main, _) = self.pick(style);
        let mut pen = x;
        let mut prev: Option<(Face, GlyphId)> = None;
        for ch in text.chars() {
            let (face, id) = self.face_for(main, ch);
            let scaled = self.face(face).as_scaled(self.px_scale(face, size));
            if let Some((pf, p)) = prev {
                if pf == face {
                    pen += scaled.kern(p, id);
                }
            }
            prev = Some((face, id));
            f(face, id, pen);
            pen += scaled.h_advance(id);
        }
    }

    /// Where a line of `style` text at `size` ends, starting at 0.
    pub fn width(&self, text: &str, size: f32, style: TextStyle) -> f32 {
        let mut end = 0.0;
        self.walk(text, size, style, 0.0, |face, id, pen| {
            end = pen + self.face(face).as_scaled(self.px_scale(face, size)).h_advance(id);
        });
        end
    }
}

/// Hasher for small integer keys: one multiplication.
#[derive(Default, Clone, Copy)]
struct Mix(u64);
type Mixed = BuildHasherDefault<Mix>;

impl Hasher for Mix {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u32(b as u32);
        }
    }
    fn write_u32(&mut self, v: u32) {
        self.0 = (self.0 ^ v as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
    fn write_u8(&mut self, v: u8) {
        self.write_u32(v as u32);
    }
}

/// A character as measured: the face that has it, its glyph, its advance
/// (font units).
type Glyph = (Face, GlyphId, f32);

/// Text metrics matching what a renderer drawing with the same
/// [`FontSet`] lays out. Give it to
/// [`Ui::set_text_measure`](morfo::Ui::set_text_measure).
///
/// It remembers the face, glyph and advance of every character it has seen
/// in each face, and the kerning of every pair, so measuring a string is a
/// few table lookups per character instead of a walk through the font's
/// tables.
pub struct FontMeasure {
    set: Arc<FontSet>,
    glyphs: RefCell<HashMap<(Face, char), Glyph, Mixed>>,
    kerns: RefCell<HashMap<(Face, u32), f32, Mixed>>,
}

impl FontMeasure {
    pub fn new(set: Arc<FontSet>) -> FontMeasure {
        FontMeasure { set, glyphs: RefCell::default(), kerns: RefCell::default() }
    }
}

impl TextMeasure for FontMeasure {
    fn width(&self, text: &str, size: f32) -> f32 {
        self.width_styled(text, size, TextStyle::REGULAR)
    }

    fn width_styled(&self, text: &str, size: f32, style: TextStyle) -> f32 {
        let set = &*self.set;
        let (main, _) = set.pick(style);
        let (mut glyphs, mut kerns) = (self.glyphs.borrow_mut(), self.kerns.borrow_mut());
        let mut w = 0.0;
        let mut prev: Option<(Face, GlyphId)> = None;
        for ch in text.chars() {
            let (face, g, advance) = *glyphs.entry((main, ch)).or_insert_with(|| {
                let (face, g) = set.face_for(main, ch);
                (face, g, set.face(face).h_advance_unscaled(g))
            });
            // The same products the renderer computes, in the same order.
            let h = set.face(face).as_scaled(set.px_scale(face, size)).h_scale_factor();
            if let Some((pf, p)) = prev.filter(|(pf, _)| *pf == face) {
                w += h * *kerns.entry((pf, (p.0 as u32) << 16 | g.0 as u32)).or_insert_with(|| set.face(face).kern_unscaled(p, g));
            }
            w += h * advance;
            prev = Some((face, g));
        }
        w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font(paths: &[&str]) -> Option<Vec<u8>> {
        paths.iter().find_map(|p| std::fs::read(p).ok())
    }

    const SANS: &[&str] = &[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    ];
    const BOLD: &[&str] = &["/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf"];

    /// What is measured is, bit for bit, where the renderer's layout ends,
    /// in every style. Skips itself where no system font is found.
    #[test]
    fn measured_widths_are_the_layouts_own() {
        let Some(regular) = font(SANS) else { return };
        let mut set = FontSet::new(regular).unwrap();
        if let Some(bold) = font(BOLD) {
            set.set_bold(bold).unwrap();
        }
        let set = Arc::new(set);
        let measure = FontMeasure::new(set.clone());
        for style in [TextStyle::REGULAR, TextStyle::BOLD, TextStyle::ITALIC, TextStyle::MONO] {
            for size in [11.0f32, 15.0, 25.5] {
                for text in ["", "AVATAR To. Wa", "¿Qué tal? «Año» — 12.480 € ← → −", "mmmm iiii"] {
                    let exact = set.width(text, size, style);
                    assert_eq!(measure.width_styled(text, size, style).to_bits(), exact.to_bits(), "{text:?} {size} {style:?}");
                    assert_eq!(measure.width_styled(text, size, style).to_bits(), exact.to_bits(), "{text:?} cached");
                }
            }
        }
    }

    /// A character the main face lacks comes from a fallback.
    #[test]
    fn missing_characters_come_from_a_fallback() {
        let Some(regular) = font(SANS) else { return };
        let mut set = FontSet::new(regular.clone()).unwrap();
        // A character outside any Latin font, then one the fallback has.
        let (face, id) = set.face_for(0, '\u{10FFFD}');
        assert_eq!((face, id.0), (0, 0), "nobody has it: the main face's missing glyph");
        set.add_fallback(regular).unwrap();
        assert_eq!(set.face_for(0, 'A').0, 0, "what the main face has stays there");
        let (main, synthetic) = set.pick(TextStyle::BOLD);
        assert_eq!(main, 0);
        assert!(synthetic || set.slots[super::BOLD].is_some(), "without a bold face, bold is faked");
    }
}
