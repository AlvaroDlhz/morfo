//! Images: pixels the application registers once and shows anywhere,
//! with rounded corners, cropped, fitted or tinted. Decoding files is the
//! application's (or a crate's) job: morfo takes the pixels.

use crate::a11y::Role;
use crate::anim::SpringKind::Smooth;
use crate::draw::{ImageData, ImageId};
use crate::geom::{Color, Rect, Vec2};
use crate::ui::Ui;

/// How an image fills a box of another shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Fit {
    /// Fills the box, cut at the edges that do not fit (the center stays).
    #[default]
    Cover,
    /// Fits whole inside the box, leaving room on two sides.
    Contain,
    /// Stretched to the box.
    Fill,
}

/// Where image `w`×`h` goes inside `area` and which part of it shows.
pub(crate) fn place(area: Rect, w: f32, h: f32, fit: Fit) -> (Rect, Rect) {
    let full = Rect::new(0.0, 0.0, 1.0, 1.0);
    if w <= 0.0 || h <= 0.0 || area.w <= 0.0 || area.h <= 0.0 {
        return (area, full);
    }
    let (box_ratio, img_ratio) = (area.w / area.h, w / h);
    match fit {
        Fit::Fill => (area, full),
        Fit::Contain => {
            let s = (area.w / w).min(area.h / h);
            let size = Vec2::new(w * s, h * s);
            (Rect::new(area.x + (area.w - size.x) * 0.5, area.y + (area.h - size.y) * 0.5, size.x, size.y), full)
        }
        Fit::Cover if img_ratio > box_ratio => {
            // Wider than the box: the sides are cut.
            let u = box_ratio / img_ratio;
            (area, Rect::new((1.0 - u) * 0.5, 0.0, u, 1.0))
        }
        Fit::Cover => {
            let v = img_ratio / box_ratio;
            (area, Rect::new(0.0, (1.0 - v) * 0.5, 1.0, v))
        }
    }
}

impl Ui {
    /// Registers an image of `width`×`height` pixels, four bytes each (red,
    /// green, blue, alpha; sRGB, not premultiplied), row by row. Returns
    /// `None` when the bytes do not match the size. The image lives until
    /// [`remove_image`](Self::remove_image).
    pub fn add_image(&mut self, width: u32, height: u32, rgba: Vec<u8>) -> Option<crate::draw::ImageId> {
        (width > 0 && height > 0 && rgba.len() == width as usize * height as usize * 4)
            .then(|| self.draw.add_image(ImageData { width, height, rgba, version: 0 }))
    }

    /// Replaces the pixels of image `id` (a video's next frame, a photo
    /// that finished loading); everything showing it shows the new ones.
    /// Returns `false` for an unknown id or bytes that do not match.
    pub fn replace_image(&mut self, id: ImageId, width: u32, height: u32, rgba: Vec<u8>) -> bool {
        width > 0
            && height > 0
            && rgba.len() == width as usize * height as usize * 4
            && self.draw.replace_image(id, ImageData { width, height, rgba, version: 0 })
    }

    /// Forgets image `id` and its pixels.
    pub fn remove_image(&mut self, id: ImageId) {
        self.draw.remove_image(id);
    }

    /// The size in pixels of image `id`.
    pub fn image_size(&self, id: ImageId) -> Option<Vec2> {
        self.draw.image(id).map(|d| Vec2::new(d.width as f32, d.height as f32))
    }

    /// Image `id` in a box of `size`, filling it as `fit` says, with the
    /// theme's rounded corners. It enters fading in and travels like any
    /// element. `alt` says what it shows, for screen readers (empty for a
    /// purely decorative image).
    pub fn image(&mut self, key: &str, id: ImageId, size: Vec2, fit: Fit, alt: &str) {
        let key_id = self.declare(key);
        let target = self.alloc(size);
        let (r, life) = self.enter(key_id, target, Smooth);
        if !alt.is_empty() {
            self.describe(key_id, Role::Image, alt, r, None, None);
        }
        let Some((w, h)) = self.draw.image(id).map(|d| (d.width as f32, d.height as f32)) else { return };
        let (rect, uv) = place(r, w, h, fit);
        let radius = (self.theme.radius * 0.6).min(rect.w * 0.5).min(rect.h * 0.5);
        let a = self.alpha * life;
        self.draw.image_cmd(rect, radius, id, uv, Color::WHITE.fade(a));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_fit_their_box() {
        let area = Rect::new(10.0, 20.0, 200.0, 100.0);
        // A square photo in a wide box.
        let (r, uv) = place(area, 50.0, 50.0, Fit::Cover);
        assert_eq!(r, area);
        assert_eq!(uv, Rect::new(0.0, 0.25, 1.0, 0.5));
        let (r, uv) = place(area, 50.0, 50.0, Fit::Contain);
        assert_eq!((r, uv), (Rect::new(60.0, 20.0, 100.0, 100.0), Rect::new(0.0, 0.0, 1.0, 1.0)));
        // A panorama in the same box: its sides are cut.
        let (_, uv) = place(area, 400.0, 100.0, Fit::Cover);
        assert_eq!(uv, Rect::new(0.25, 0.0, 0.5, 1.0));
        assert_eq!(place(area, 400.0, 100.0, Fit::Fill), (area, Rect::new(0.0, 0.0, 1.0, 1.0)));
    }
}
