//! Design tokens. Colors live in a fixed-size palette so a whole theme can be
//! spring-animated as a single value: switching themes morphs every element.

use crate::geom::Color;

/// A pair of a theme's colors with too little contrast between them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContrastIssue {
    pub what: &'static str,
    pub ratio: f32,
    pub needed: f32,
}

/// WCAG relative luminance of an sRGB color.
fn luminance(c: Color) -> f32 {
    let lin = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// White or near black: whichever contrasts more with `background`.
pub fn on_color(background: Color) -> Color {
    let dark = Color::hex(0x16161d);
    if contrast(background, Color::WHITE) >= contrast(background, dark) {
        Color::WHITE
    } else {
        dark
    }
}

/// The WCAG contrast ratio between two opaque colors, `1..=21`.
pub fn contrast(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

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
        // Dark enough for white text on it (4.6:1), light enough to stand
        // out on the background (4.2:1).
        palette[ACCENT] = Color::hex(0x6e5ef2);
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

    /// The same theme, denser (`density` < 1) or roomier (> 1): spacing
    /// and control heights scale, text does not.
    pub fn with_density(mut self, density: f32) -> Theme {
        self.pad *= density;
        self.gap *= density;
        self.control *= 0.75 + 0.25 * density;
        self.radius *= 0.6 + 0.4 * density;
        self
    }

    /// The same theme with every size (text included) multiplied by
    /// `factor`.
    pub fn scaled(mut self, factor: f32) -> Theme {
        self.font *= factor;
        self.control *= factor;
        self.pad *= factor;
        self.gap *= factor;
        self.radius *= factor;
        self
    }

    /// Light text on black, borders and focus you cannot miss: every pair
    /// of colors well past the WCAG ratios (7:1 for text, 4.5:1 for the
    /// edges of controls).
    pub fn high_contrast() -> Theme {
        let mut t = Theme::dark();
        t.palette[BG] = Color::hex(0x000000);
        t.palette[SURFACE] = Color::hex(0x0a0a0a);
        t.palette[RAISED] = Color::hex(0x161616);
        t.palette[BORDER] = Color::hex(0xb8b8b8);
        t.palette[TEXT] = Color::hex(0xffffff);
        t.palette[MUTED] = Color::hex(0xd6d6d6);
        t.palette[ACCENT] = Color::hex(0xffd84d);
        t.palette[ON_ACCENT] = Color::hex(0x000000);
        t.palette[SHADOW] = Color::rgba(0.0, 0.0, 0.0, 0.0);
        t
    }

    /// Like [`high_contrast`](Self::high_contrast), dark on white.
    pub fn high_contrast_light() -> Theme {
        let mut t = Theme::light();
        t.palette[BG] = Color::hex(0xffffff);
        t.palette[SURFACE] = Color::hex(0xffffff);
        t.palette[RAISED] = Color::hex(0xf2f2f2);
        t.palette[BORDER] = Color::hex(0x4a4a4a);
        t.palette[TEXT] = Color::hex(0x000000);
        t.palette[MUTED] = Color::hex(0x2e2e2e);
        t.palette[ACCENT] = Color::hex(0x0b3fc2);
        t.palette[ON_ACCENT] = Color::hex(0xffffff);
        t.palette[SHADOW] = Color::rgba(0.0, 0.0, 0.0, 0.0);
        t
    }

    /// The pairs of this theme's colors below the WCAG 2.2 AA contrast they
    /// need: 4.5:1 for text (1.4.3), 3:1 for what identifies a control
    /// (1.4.11). Empty when the theme passes. For checking a brand theme
    /// before shipping it.
    pub fn contrast_issues(&self) -> Vec<ContrastIssue> {
        let p = &self.palette;
        let checks: [(&'static str, usize, usize, f32); 9] = [
            ("text on background", TEXT, BG, 4.5),
            ("text on surface", TEXT, SURFACE, 4.5),
            ("text on raised", TEXT, RAISED, 4.5),
            ("muted text on background", MUTED, BG, 4.5),
            ("muted text on surface", MUTED, SURFACE, 4.5),
            ("text on accent", ON_ACCENT, ACCENT, 4.5),
            ("accent on background", ACCENT, BG, 3.0),
            ("borders on background", BORDER, BG, 3.0),
            ("borders on surface", BORDER, SURFACE, 3.0),
        ];
        checks
            .into_iter()
            .filter_map(|(what, a, b, needed)| {
                let ratio = contrast(p[a], p[b]);
                (ratio < needed).then_some(ContrastIssue { what, ratio, needed })
            })
            .collect()
    }

    /// The same theme with another accent; the text drawn on it turns
    /// white or near black, whichever reads better on it.
    pub fn with_accent(mut self, accent: Color) -> Theme {
        self.palette[ACCENT] = accent;
        self.palette[ON_ACCENT] = on_color(accent);
        self
    }
}

/// The names of the palette's colors in design tokens, in palette order.
const TOKEN_COLORS: [&str; PALETTE_LEN] = ["bg", "surface", "raised", "border", "text", "muted", "accent", "on-accent", "shadow"];

impl Theme {
    /// The theme as design tokens, CSS custom properties a design tool or a
    /// web page can use as they are:
    ///
    /// ```text
    /// --bg: #0b0c10;
    /// --accent: #6e5ef2;
    /// --radius: 16px;
    /// ```
    pub fn to_tokens(&self) -> String {
        let mut out = String::new();
        for (name, c) in TOKEN_COLORS.iter().zip(self.palette) {
            let [r, g, b, a] = c.to_rgba8();
            if a == 255 {
                out.push_str(&format!("--{name}: #{r:02x}{g:02x}{b:02x};\n"));
            } else {
                out.push_str(&format!("--{name}: #{r:02x}{g:02x}{b:02x}{a:02x};\n"));
            }
        }
        for (name, v) in [("radius", self.radius), ("gap", self.gap), ("pad", self.pad), ("font", self.font), ("control", self.control)] {
            out.push_str(&format!("--{name}: {v}px;\n"));
        }
        out
    }

    /// `base` with the design tokens in `tokens` applied: lines of
    /// `--name: value;` as [`to_tokens`](Self::to_tokens) writes them
    /// (`name = value` works too; `--morfo-` prefixes, comments and unknown
    /// names are skipped). Colors are `#rgb`, `#rrggbb` or `#rrggbbaa`;
    /// sizes are numbers, `px` optional. A value that cannot be read is an
    /// error naming its line.
    pub fn from_tokens(tokens: &str, base: Theme) -> Result<Theme, String> {
        let mut t = base;
        for (n, line) in tokens.lines().enumerate() {
            let line = line.split("/*").next().unwrap_or("").trim().trim_end_matches(';');
            let Some((name, value)) = line.split_once(':').or_else(|| line.split_once('=')) else { continue };
            let name = name.trim().trim_start_matches("--").trim_start_matches("morfo-");
            let value = value.trim();
            let bad = || format!("line {}: cannot read {value:?} for {name}", n + 1);
            if let Some(i) = TOKEN_COLORS.iter().position(|c| *c == name) {
                t.palette[i] = parse_color(value).ok_or_else(bad)?;
                continue;
            }
            let size = || value.trim_end_matches("px").trim().parse::<f32>().ok().filter(|v| v.is_finite() && *v >= 0.0);
            let slot = match name {
                "radius" => &mut t.radius,
                "gap" => &mut t.gap,
                "pad" => &mut t.pad,
                "font" => &mut t.font,
                "control" => &mut t.control,
                _ => continue,
            };
            *slot = size().ok_or_else(bad)?;
        }
        Ok(t)
    }
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa`.
fn parse_color(v: &str) -> Option<Color> {
    let h = v.strip_prefix('#')?;
    let n = u32::from_str_radix(h, 16).ok()?;
    let k = |x: u32| x as f32 / 255.0;
    Some(match h.len() {
        3 => Color::rgba(k((n >> 8 & 0xf) * 17), k((n >> 4 & 0xf) * 17), k((n & 0xf) * 17), 1.0),
        6 => Color::hex(n),
        8 => Color::rgba(k(n >> 24), k(n >> 16 & 0xff), k(n >> 8 & 0xff), k(n & 0xff)),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn tokens_go_there_and_back() {
        for t in [Theme::dark(), Theme::light(), Theme::high_contrast()] {
            let back = Theme::from_tokens(&t.to_tokens(), Theme::light()).unwrap();
            for (a, b) in t.palette.iter().zip(back.palette) {
                assert_eq!(a.to_rgba8(), b.to_rgba8());
            }
            assert_eq!((back.radius, back.font, back.control), (t.radius, t.font, t.control));
        }
        let t = Theme::from_tokens("/* brand */\n--morfo-accent: #f50;\nradius = 4\n--unknown: 3;", Theme::dark()).unwrap();
        assert_eq!(t.palette[ACCENT].to_hex(), 0xff5500);
        assert_eq!(t.radius, 4.0);
        assert!(Theme::from_tokens("--gap: wide;", Theme::dark()).unwrap_err().contains("line 1"));
    }
    use super::*;

    #[test]
    fn contrast_follows_wcag() {
        assert!((contrast(Color::BLACK, Color::WHITE) - 21.0).abs() < 0.01);
        assert!((contrast(Color::hex(0x777777), Color::WHITE) - 4.48).abs() < 0.02);
        assert!(Theme::high_contrast().contrast_issues().is_empty());
        assert!(Theme::high_contrast_light().contrast_issues().is_empty());
        // The regular themes keep their text readable; what they give up is
        // the contrast of the borders, which the report says.
        for t in [Theme::dark(), Theme::light()] {
            let issues = t.contrast_issues();
            assert!(issues.iter().all(|i| i.what.starts_with("borders") || i.what.starts_with("accent")), "{issues:?}");
        }
    }
}
