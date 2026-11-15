//! Design tokens. Colors live in a fixed-size palette so a whole theme can be
//! spring-animated as a single value: switching themes morphs every element.

use crate::geom::Color;

pub const BG: usize = 0;
pub const SURFACE: usize = 1;
pub const RAISED: usize = 2;
pub const BORDER: usize = 3;
pub const TEXT: usize = 4;
pub const MUTED: usize = 5;
pub const ACCENT: usize = 6;
pub const ON_ACCENT: usize = 7;
pub const SHADOW: usize = 8;
pub const PALETTE_LEN: usize = 9;

pub type Palette = [Color; PALETTE_LEN];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub palette: Palette,
    /// Corner radius of surfaces.
    pub radius: f32,
    /// Space between sibling elements.
    pub gap: f32,
    /// Inner padding of surfaces and horizontal padding of controls.
    pub pad: f32,
    /// Base font size.
    pub font: f32,
    /// Height of a standard control.
    pub control: f32,
}

impl Theme {
    pub fn dark() -> Theme {
        let mut palette = [Color::TRANSPARENT; PALETTE_LEN];
        palette[BG] = Color::hex(0x0b0c10);
        palette[SURFACE] = Color::hex(0x15171d);
        palette[RAISED] = Color::hex(0x20232c);
        palette[BORDER] = Color::hex(0x2c303b);
        palette[TEXT] = Color::hex(0xf2f3f7);
        palette[MUTED] = Color::hex(0x8b90a0);
        palette[ACCENT] = Color::hex(0x7c6cff);
        palette[ON_ACCENT] = Color::hex(0xffffff);
        palette[SHADOW] = Color::rgba(0.0, 0.0, 0.0, 0.45);
        Theme { palette, radius: 16.0, gap: 12.0, pad: 18.0, font: 15.0, control: 40.0 }
    }

    pub fn light() -> Theme {
        let mut t = Theme::dark();
        t.palette[BG] = Color::hex(0xf4f5f8);
        t.palette[SURFACE] = Color::hex(0xffffff);
        t.palette[RAISED] = Color::hex(0xeceef3);
        t.palette[BORDER] = Color::hex(0xdfe2ea);
        t.palette[TEXT] = Color::hex(0x14161c);
        t.palette[MUTED] = Color::hex(0x6b7080);
        t.palette[ACCENT] = Color::hex(0x5b4bff);
        t.palette[SHADOW] = Color::rgba(0.1, 0.12, 0.25, 0.14);
        t
    }

    pub fn with_accent(mut self, accent: Color) -> Theme {
        self.palette[ACCENT] = accent;
        self
    }
}
