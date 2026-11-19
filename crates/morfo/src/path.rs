//! Vector shapes: straight lines, Bézier curves and arcs, built in code or
//! read from SVG path data (`M 4 12 L 20 12`, as icon sets publish them),
//! filled or stroked into a [`DrawList`] at any size. They become the
//! polygons and lines every renderer already draws: nothing new to support.
//!
//! ```
//! # use morfo::{Color, DrawList, Path, Rect};
//! // A heart, 24×24 units, drawn into a 48-pixel square.
//! let heart = Path::svg("M12 21 C5 15 2 11 2 7.5 A4.5 4.5 0 0 1 12 6 A4.5 4.5 0 0 1 22 7.5 C22 11 19 15 12 21 Z").unwrap();
//! let mut list = DrawList::default();
//! heart.fill_in(&mut list, Rect::new(0.0, 0.0, 24.0, 24.0), Rect::new(10.0, 10.0, 48.0, 48.0), Color::hex(0xe6334d));
//! ```

use std::f32::consts::TAU;

use crate::draw::DrawList;
use crate::geom::{Color, Rect, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Seg {
    Move(Vec2),
    Line(Vec2),
    Quad(Vec2, Vec2),
    Cubic(Vec2, Vec2, Vec2),
    Close,
}

/// A shape made of contours of lines and curves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    segs: Vec<Seg>,
}

impl Path {
    pub fn new() -> Path {
        Path::default()
    }

    /// Starts a new contour at `p`.
    pub fn move_to(mut self, p: Vec2) -> Path {
        self.segs.push(Seg::Move(p));
        self
    }

    pub fn line_to(mut self, p: Vec2) -> Path {
        self.segs.push(Seg::Line(p));
        self
    }

    /// A quadratic Bézier curve pulled towards `c`, ending at `p`.
    pub fn quad_to(mut self, c: Vec2, p: Vec2) -> Path {
        self.segs.push(Seg::Quad(c, p));
        self
    }

    /// A cubic Bézier curve with control points `c1` and `c2`, ending at `p`.
    pub fn cubic_to(mut self, c1: Vec2, c2: Vec2, p: Vec2) -> Path {
        self.segs.push(Seg::Cubic(c1, c2, p));
        self
    }

    /// Closes the contour back to its start.
    pub fn close(mut self) -> Path {
        self.segs.push(Seg::Close);
        self
    }

    /// Reads SVG path data: the commands `M L H V C S Q T A Z`, absolute
    /// (upper case) or relative (lower case). `None` if it is malformed.
    pub fn svg(d: &str) -> Option<Path> {
        let mut p = Parser { s: d.as_bytes(), i: 0 };
        let mut segs = Vec::new();
        let (mut cur, mut start) = (Vec2::ZERO, Vec2::ZERO);
        // The last control point, for the smooth curves S and T.
        let mut last_c: Option<(u8, Vec2)> = None;
        let mut cmd = 0u8;
        loop {
            p.skip();
            if p.i >= p.s.len() {
                break;
            }
            let b = p.s[p.i];
            if b.is_ascii_alphabetic() {
                cmd = b;
                p.i += 1;
            } else if cmd == 0 {
                return None;
            } else if cmd == b'M' || cmd == b'm' {
                // Numbers after a move are lines.
                cmd = if cmd == b'M' { b'L' } else { b'l' };
            }
            let rel = cmd.is_ascii_lowercase();
            let base = if rel { cur } else { Vec2::ZERO };
            let pt = |p: &mut Parser| -> Option<Vec2> { Some(Vec2::new(p.num()?, p.num()?) + base) };
            let mut c_used = None;
            match cmd.to_ascii_uppercase() {
                b'M' => {
                    cur = pt(&mut p)?;
                    start = cur;
                    segs.push(Seg::Move(cur));
                }
                b'L' => {
                    cur = pt(&mut p)?;
                    segs.push(Seg::Line(cur));
                }
                b'H' => {
                    cur = Vec2::new(p.num()? + base.x, cur.y);
                    segs.push(Seg::Line(cur));
                }
                b'V' => {
                    cur = Vec2::new(cur.x, p.num()? + base.y);
                    segs.push(Seg::Line(cur));
                }
                b'C' => {
                    let (c1, c2, e) = (pt(&mut p)?, pt(&mut p)?, pt(&mut p)?);
                    segs.push(Seg::Cubic(c1, c2, e));
                    c_used = Some((b'C', c2));
                    cur = e;
                }
                b'S' => {
                    let c1 = match last_c {
                        Some((b'C', c)) => cur * 2.0 - c,
                        _ => cur,
                    };
                    let (c2, e) = (pt(&mut p)?, pt(&mut p)?);
                    segs.push(Seg::Cubic(c1, c2, e));
                    c_used = Some((b'C', c2));
                    cur = e;
                }
                b'Q' => {
                    let (c, e) = (pt(&mut p)?, pt(&mut p)?);
                    segs.push(Seg::Quad(c, e));
                    c_used = Some((b'Q', c));
                    cur = e;
                }
                b'T' => {
                    let c = match last_c {
                        Some((b'Q', c)) => cur * 2.0 - c,
                        _ => cur,
                    };
                    let e = pt(&mut p)?;
                    segs.push(Seg::Quad(c, e));
                    c_used = Some((b'Q', c));
                    cur = e;
                }
                b'A' => {
                    let (rx, ry, rot) = (p.num()?, p.num()?, p.num()?);
                    let (large, sweep) = (p.flag()?, p.flag()?);
                    let e = pt(&mut p)?;
                    arc(&mut segs, cur, rx, ry, rot.to_radians(), large, sweep, e);
                    cur = e;
                }
                b'Z' => {
                    segs.push(Seg::Close);
                    cur = start;
                }
                _ => return None,
            }
            last_c = c_used;
        }
        Some(Path { segs })
    }

    /// The contours as points, curves cut into straight pieces no more than
    /// about `tolerance` units off the true curve, each point passed
    /// through `map`.
    fn contours(&self, tolerance: f32, map: impl Fn(Vec2) -> Vec2) -> Vec<(Vec<Vec2>, bool)> {
        let mut out: Vec<(Vec<Vec2>, bool)> = Vec::new();
        let (mut cur, mut start) = (Vec2::ZERO, Vec2::ZERO);
        let mut open: Vec<Vec2> = Vec::new();
        let steps = |len: f32| ((len / tolerance.max(1e-3)).sqrt().ceil() as usize).clamp(1, 64);
        for &seg in &self.segs {
            match seg {
                Seg::Move(p) => {
                    if open.len() > 1 {
                        out.push((std::mem::take(&mut open), false));
                    }
                    open.clear();
                    open.push(map(p));
                    (cur, start) = (p, p);
                }
                Seg::Line(p) => {
                    if open.is_empty() {
                        open.push(map(cur));
                    }
                    open.push(map(p));
                    cur = p;
                }
                Seg::Quad(c, p) => {
                    if open.is_empty() {
                        open.push(map(cur));
                    }
                    let n = steps(((c - cur).length() + (p - c).length()) * 0.5);
                    for i in 1..=n {
                        let t = i as f32 / n as f32;
                        let u = 1.0 - t;
                        open.push(map(cur * (u * u) + c * (2.0 * u * t) + p * (t * t)));
                    }
                    cur = p;
                }
                Seg::Cubic(c1, c2, p) => {
                    if open.is_empty() {
                        open.push(map(cur));
                    }
                    let n = steps(((c1 - cur).length() + (c2 - c1).length() + (p - c2).length()) * 0.75);
                    for i in 1..=n {
                        let t = i as f32 / n as f32;
                        let u = 1.0 - t;
                        open.push(map(cur * (u * u * u) + c1 * (3.0 * u * u * t) + c2 * (3.0 * u * t * t) + p * (t * t * t)));
                    }
                    cur = p;
                }
                Seg::Close => {
                    if open.len() > 1 {
                        let first = open[0];
                        out.push((std::mem::take(&mut open), true));
                        // A new contour after Z starts where this one did.
                        open.push(first);
                        cur = start;
                    }
                }
            }
        }
        if open.len() > 1 {
            out.push((open, false));
        }
        out
    }

    /// The transform that takes `view` (path units) onto `into` (pixels),
    /// keeping proportions and centering.
    fn fit(view: Rect, into: Rect) -> (f32, Vec2) {
        let k = (into.w / view.w.max(1e-6)).min(into.h / view.h.max(1e-6));
        let off = Vec2::new(into.x + (into.w - view.w * k) * 0.5 - view.x * k, into.y + (into.h - view.h * k) * 0.5 - view.y * k);
        (k, off)
    }

    /// Fills the shape (non-zero winding: contours turning the other way
    /// cut holes), its `view` box fitted into `into`.
    pub fn fill_in(&self, list: &mut DrawList, view: Rect, into: Rect, color: Color) {
        let (k, off) = Self::fit(view, into);
        self.fill_with(list, 0.25 / k.max(1e-6), |p| p * k + off, color);
    }

    /// Strokes the outline, `width` pixels wide with round ends, its `view`
    /// box fitted into `into`.
    pub fn stroke_in(&self, list: &mut DrawList, view: Rect, into: Rect, width: f32, color: Color) {
        let (k, off) = Self::fit(view, into);
        self.stroke_with(list, 0.25 / k.max(1e-6), |p| p * k + off, width, color);
    }

    /// Fills the shape as it is, in pixels.
    pub fn fill(&self, list: &mut DrawList, color: Color) {
        self.fill_with(list, 0.25, |p| p, color);
    }

    /// Strokes the outline as it is, in pixels.
    pub fn stroke(&self, list: &mut DrawList, width: f32, color: Color) {
        self.stroke_with(list, 0.25, |p| p, width, color);
    }

    fn fill_with(&self, list: &mut DrawList, tolerance: f32, map: impl Fn(Vec2) -> Vec2, color: Color) {
        let contours = self.contours(tolerance, map);
        // One polygon: each contour is reached from the first point and left
        // the same way, so the two passes of the seam cancel out. The ends
        // of a seam are doubled, which is how renderers tell it from an
        // edge (a doubled point draws nothing).
        let mut pts: Vec<Vec2> = Vec::new();
        for (c, _) in &contours {
            if c.len() < 3 {
                continue;
            }
            let anchor = pts.first().copied();
            if let Some(a) = anchor {
                // a, a, c₀, c₀, … contour …, c₀, c₀, a, a
                if pts[pts.len() - 2..] != [a, a] {
                    pts.push(a);
                }
                pts.push(c[0]);
            }
            pts.extend_from_slice(c);
            pts.push(c[0]);
            if let Some(a) = anchor {
                pts.push(c[0]);
                pts.push(a);
                pts.push(a);
            }
        }
        if pts.len() >= 3 {
            list.polygon(&pts, color);
        }
    }

    fn stroke_with(&self, list: &mut DrawList, tolerance: f32, map: impl Fn(Vec2) -> Vec2, width: f32, color: Color) {
        for (c, closed) in self.contours(tolerance, map) {
            for w in c.windows(2) {
                list.line(w[0], w[1], width, color);
            }
            if closed {
                list.line(c[c.len() - 1], c[0], width, color);
            }
        }
    }
}

/// An elliptical arc from `from` to `to` (SVG's endpoint form), as cubic
/// curves.
#[allow(clippy::too_many_arguments)]
fn arc(segs: &mut Vec<Seg>, from: Vec2, rx: f32, ry: f32, rot: f32, large: bool, sweep: bool, to: Vec2) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-6 || ry < 1e-6 || (from - to).length() < 1e-6 {
        segs.push(Seg::Line(to));
        return;
    }
    let (s, c) = rot.sin_cos();
    // The center, by the SVG specification's conversion.
    let d = (from - to) * 0.5;
    let p = Vec2::new(c * d.x + s * d.y, -s * d.x + c * d.y);
    let l = (p.x * p.x) / (rx * rx) + (p.y * p.y) / (ry * ry);
    if l > 1.0 {
        rx *= l.sqrt();
        ry *= l.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * p.y * p.y - ry * ry * p.x * p.x).max(0.0);
    let den = rx * rx * p.y * p.y + ry * ry * p.x * p.x;
    let mut q = (num / den.max(1e-12)).sqrt();
    if large == sweep {
        q = -q;
    }
    let cp = Vec2::new(q * rx * p.y / ry, -q * ry * p.x / rx);
    let mid = (from + to) * 0.5;
    let center = Vec2::new(c * cp.x - s * cp.y + mid.x, s * cp.x + c * cp.y + mid.y);
    let angle = |v: Vec2| v.y.atan2(v.x);
    let a0 = angle(Vec2::new((p.x - cp.x) / rx, (p.y - cp.y) / ry));
    let mut da = angle(Vec2::new((-p.x - cp.x) / rx, (-p.y - cp.y) / ry)) - a0;
    if sweep && da < 0.0 {
        da += TAU;
    } else if !sweep && da > 0.0 {
        da -= TAU;
    }
    // At most a quarter turn per cubic.
    let n = (da.abs() / (TAU / 4.0)).ceil().max(1.0) as usize;
    let step = da / n as f32;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let on = |a: f32| {
        let (sa, ca) = a.sin_cos();
        Vec2::new(c * rx * ca - s * ry * sa, s * rx * ca + c * ry * sa) + center
    };
    let tangent = |a: f32| {
        let (sa, ca) = a.sin_cos();
        Vec2::new(-c * rx * sa - s * ry * ca, -s * rx * sa + c * ry * ca)
    };
    for i in 0..n {
        let (t0, t1) = (a0 + step * i as f32, a0 + step * (i + 1) as f32);
        let end = if i + 1 == n { to } else { on(t1) };
        segs.push(Seg::Cubic(on(t0) + tangent(t0) * k, on(t1) - tangent(t1) * k, end));
    }
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn skip(&mut self) {
        while self.i < self.s.len() && (self.s[self.i].is_ascii_whitespace() || self.s[self.i] == b',') {
            self.i += 1;
        }
    }

    fn num(&mut self) -> Option<f32> {
        self.skip();
        let start = self.i;
        let mut seen_dot = false;
        let mut seen_e = false;
        while self.i < self.s.len() {
            let b = self.s[self.i];
            let first = self.i == start;
            let after_e = self.i > start && matches!(self.s[self.i - 1], b'e' | b'E');
            match b {
                b'0'..=b'9' => {}
                b'-' | b'+' if first || after_e => {}
                b'.' if !seen_dot && !seen_e => seen_dot = true,
                b'e' | b'E' if !seen_e && !first => seen_e = true,
                _ => break,
            }
            self.i += 1;
        }
        std::str::from_utf8(&self.s[start..self.i]).ok()?.parse().ok()
    }

    /// An arc flag: one digit, possibly with no separator after it.
    fn flag(&mut self) -> Option<bool> {
        self.skip();
        let b = *self.s.get(self.i)?;
        self.i += 1;
        match b {
            b'0' => Some(false),
            b'1' => Some(true),
            _ => None,
        }
    }
}

impl DrawList {
    /// A dashed line: `dash` pixels drawn, `gap` left out, from `from`.
    pub fn dashed_line(&mut self, from: Vec2, to: Vec2, width: f32, dash: f32, gap: f32, color: Color) {
        let len = (to - from).length();
        if len < 1e-3 || dash <= 0.0 {
            return;
        }
        let dir = (to - from) * (1.0 / len);
        let period = dash + gap.max(0.0);
        let mut at = 0.0;
        // Never more than a few thousand dashes for one line.
        let period = period.max(len / 4096.0);
        while at < len {
            let end = (at + dash).min(len);
            self.line(from + dir * at, from + dir * end, width, color);
            at += period;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::DrawCmd;

    #[test]
    fn svg_data_is_read() {
        let p = Path::svg("M2 2 L22 2 l0 20 H2 v-20 Z").unwrap();
        assert_eq!(p.segs.len(), 6);
        assert_eq!(p.segs[2], Seg::Line(Vec2::new(22.0, 22.0)), "relative to the current point");
        assert_eq!(p.segs[3], Seg::Line(Vec2::new(2.0, 22.0)));
        // Numbers packed the way minifiers write them.
        let q = Path::svg("M.5-1.5.5.5l-1e1,2").unwrap();
        assert_eq!(q.segs, vec![Seg::Move(Vec2::new(0.5, -1.5)), Seg::Line(Vec2::new(0.5, 0.5)), Seg::Line(Vec2::new(-9.5, 2.5))]);
        assert!(Path::svg("M 1 2 X 3").is_none());
        assert!(Path::svg("3 4").is_none());
    }

    #[test]
    fn arcs_end_where_they_should_and_bulge() {
        // A half circle of radius 10 from (0,10) to (20,10), sweeping up.
        let p = Path::svg("M0 10 A10 10 0 0 1 20 10").unwrap();
        let c = p.contours(0.05, |v| v);
        let pts = &c[0].0;
        assert_eq!(*pts.last().unwrap(), Vec2::new(20.0, 10.0));
        let top = pts.iter().map(|v| v.y).fold(f32::MAX, f32::min);
        assert!((top - 0.0).abs() < 0.1, "it rises to the top of the circle: {top}");
        for v in pts {
            assert!(((*v - Vec2::new(10.0, 10.0)).length() - 10.0).abs() < 0.05, "on the circle: {v:?}");
        }
    }

    #[test]
    fn fills_and_strokes_become_polygons_and_lines() {
        let ring = Path::svg("M0 0 H10 V10 H0 Z M3 3 V7 H7 V3 Z").unwrap();
        let mut list = DrawList::default();
        ring.fill_in(&mut list, Rect::new(0.0, 0.0, 10.0, 10.0), Rect::new(0.0, 0.0, 100.0, 100.0), Color::WHITE);
        ring.stroke(&mut list, 1.0, Color::WHITE);
        list.flatten();
        let polys = list.commands().iter().filter(|c| matches!(c, DrawCmd::Polygon { .. })).count();
        let lines = list.commands().iter().filter(|c| matches!(c, DrawCmd::Line { .. })).count();
        assert_eq!((polys, lines), (1, 8), "both contours in one polygon; four sides each");
        let DrawCmd::Polygon { start, len, .. } = list.commands()[0] else { panic!() };
        assert!(list.points(start, len).iter().any(|p| *p == Vec2::new(70.0, 70.0)), "scaled ten times");
    }

    #[test]
    fn dashes_cover_the_line() {
        let mut list = DrawList::default();
        list.dashed_line(Vec2::ZERO, Vec2::new(100.0, 0.0), 2.0, 6.0, 4.0, Color::WHITE);
        list.flatten();
        assert_eq!(list.commands().len(), 10);
    }
}
