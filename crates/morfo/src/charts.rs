//! Charts: bars, lines, sparklines, heat maps and gauges.
//!
//! Every value and the scale itself are springs, so new data does not
//! replace a chart: the chart travels to it. Bars and series are keyed by
//! their labels, so reordering the data moves them instead of repainting.

use std::f32::consts::PI;
use std::fmt::Write;

use crate::a11y::Role;
use crate::anim::smoothstep;
use crate::anim::SpringKind::{Bouncy, Gentle, Smooth, Snappy};
use crate::draw::DrawCmd;
use crate::geom::{Color, Rect, Vec2};
use crate::id::Id;
use crate::theme::{Palette, ACCENT, BORDER, MUTED, RAISED, SHADOW, TEXT};
use crate::ui::{Node, Ui, LINE_HEIGHT};

/// Colors of the series after the first one, which takes the accent.
const SERIES: [u32; 5] = [0x22c3a6, 0xff7a59, 0xf5b83d, 0x3b82f6, 0xe5484d];
/// Rising and falling, for trends.
pub(crate) const UP: u32 = 0x22c3a6;
pub(crate) const DOWN: u32 = 0xe5484d;
const WARN: u32 = 0xf5b83d;

/// Color of series `i`: the accent first, then a fixed sequence.
pub(crate) fn series_color(pal: &Palette, i: usize) -> Color {
    if i == 0 {
        pal[ACCENT]
    } else {
        Color::hex(SERIES[(i - 1) % SERIES.len()])
    }
}

/// The smallest round number (1, 2, 2.5 or 5 times a power of ten) that is
/// at least `v`: where an axis should end.
pub(crate) fn nice_ceil(v: f32) -> f32 {
    if v.is_nan() || v <= 0.0 || !v.is_finite() {
        return 1.0;
    }
    let p = 10f32.powf(v.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0].into_iter().map(|m| m * p).find(|&m| m >= v * 0.9999).unwrap_or(10.0 * p)
}

/// An axis that covers `min..=max` with at most five round steps: its
/// start, end and step (start and end are multiples of the step).
pub(crate) fn nice_axis(min: f32, max: f32) -> (f32, f32, f32) {
    let (min, max) = (min.min(0.0), max.max(0.0));
    let step = nice_ceil((max - min) / 5.0);
    let lo = (min / step).floor() * step;
    let hi = ((max / step).ceil() * step).max(lo + step);
    (lo, hi, step)
}

/// A number as short text, Spanish style: `1250` → `1,3k`, `12.5` → `12,5`.
pub(crate) fn short_number(v: f32, decimal: char, out: &mut String) {
    out.clear();
    let a = v.abs();
    if a >= 10_000.0 {
        let _ = write!(out, "{:.0}k", v / 1000.0);
    } else if a >= 1000.0 {
        let _ = write!(out, "{:.1}k", v / 1000.0);
    } else if (v - v.round()).abs() < 0.05 {
        let _ = write!(out, "{:.0}", v);
    } else {
        let _ = write!(out, "{:.1}", v);
    }
    if decimal != '.' {
        if let Some(at) = out.find('.') {
            let mut buf = [0u8; 4];
            out.replace_range(at..at + 1, decimal.encode_utf8(&mut buf));
        }
    }
}

impl Ui {
    /// A small card above `anchor` with one line per entry; a non-transparent
    /// color puts a dot before its line. It slides from one anchor to the
    /// next and fades out when `shown` is `None`.
    pub(crate) fn chart_tip(&mut self, id: Id, shown: Option<(Vec2, Vec<(Color, String)>)>) {
        let tid = id.child("morfo-tip");
        let visible = shown.is_some();
        let anchor = match shown {
            Some((at, lines)) => {
                self.tips.insert(tid, lines);
                Some(at)
            }
            None => None,
        };
        let p = self.presence_id(tid.child("presence"), visible, Snappy).clamp(0.0, 1.0);
        if p <= 0.001 {
            self.tips.remove(&tid);
            return;
        }
        let Some(lines) = self.tips.get(&tid) else { return };
        let font = self.theme.font * 0.82;
        let lh = font * LINE_HEIGHT;
        let dot = 8.0;
        let widest = lines.iter().map(|(c, t)| self.text.width(t, font) + if c.a > 0.0 { dot + 6.0 } else { 0.0 }).fold(0.0, f32::max);
        let size = Vec2::new(widest + 20.0, lines.len() as f32 * lh + 12.0);
        let vp = self.viewport();
        let target = match anchor {
            Some(at) => {
                let x = (at.x - size.x * 0.5).min(vp.x - size.x - 6.0).max(6.0);
                let y = if at.y - size.y - 10.0 >= 6.0 { at.y - size.y - 10.0 } else { at.y + 14.0 };
                Rect::new(x, y, size.x, size.y)
            }
            None => self.nodes.get(&tid).map_or(Rect::default(), |n| n.rect.target()),
        };
        let lines = self.tips.get(&tid).cloned().unwrap_or_default();
        let (r, _) = self.enter(tid, target, Snappy);
        let below = self.draw.set_layer(2);
        let cull = self.draw.cull.take();
        let r = r.translate(Vec2::new(0.0, (1.0 - p) * 4.0));
        self.draw.push(DrawCmd::Shadow { rect: r, radius: 9.0, blur: 16.0, offset: Vec2::new(0.0, 6.0), color: self.pal[SHADOW].fade(p) });
        self.draw.rect_stroked(r, 9.0, self.pal[RAISED].fade(p), self.pal[BORDER].fade(p), 1.0);
        for (i, (color, text)) in lines.iter().enumerate() {
            let y = r.y + 6.0 + i as f32 * lh;
            let mut x = r.x + 10.0;
            if color.a > 0.0 {
                self.draw.rect(Rect::new(x, y + (lh - dot) * 0.5, dot, dot), dot, color.fade(p));
                x += dot + 6.0;
            }
            let ink = if i == 0 && lines.len() > 1 { self.pal[MUTED] } else { self.pal[TEXT] };
            self.draw.add_text(Vec2::new(x, y), font, ink.fade(p), text);
        }
        self.draw.cull = cull;
        self.draw.set_layer(below);
    }

    /// Springs the axis of chart `id` towards `(lo, hi)`; returns the
    /// current (animated) range.
    fn chart_scale(&mut self, id: Id, lo: f32, hi: f32) -> (f32, f32) {
        let k = self.steps[Gentle as usize];
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        let anim = n.e1.drive(hi, fresh, &k) | n.e2.drive(lo, fresh, &k);
        self.animating |= anim;
        let (lo, hi) = (n.e2.get(), n.e1.get());
        (lo, if hi - lo < 1e-6 { lo + 1.0 } else { hi })
    }

    /// Horizontal grid lines with their values along the left edge of
    /// `plot`; returns nothing, draws everything.
    /// `axis` is the target `(lo, hi, step)`; the range given before it, the
    /// current (animated) one. The lines sit on round values and slide while the
    /// scale changes.
    fn chart_grid(&mut self, plot: Rect, (lo, hi): (f32, f32), axis: (f32, f32, f32), axis_w: f32, font: f32, a: f32) {
        let dec = self.locale.decimal;
        let mut label = String::new();
        let (from, to, step) = axis;
        let count = ((to - from) / step).round() as usize;
        for q in 0..=count {
            let v = from + step * q as f32;
            let y = plot.bottom() - plot.h * ((v - lo) / (hi - lo));
            if y < plot.y - 1.0 || y > plot.bottom() + 1.0 {
                continue;
            }
            let base = v.abs() < step * 0.001;
            let color = if base { self.pal[BORDER] } else { self.pal[BORDER].with_alpha(0.55) };
            self.draw.line(Vec2::new(plot.x, y), Vec2::new(plot.right(), y), 1.0, color.fade(a));
            short_number(v, dec, &mut label);
            let w = self.text.width(&label, font);
            self.draw.add_text(Vec2::new(plot.x - axis_w + (axis_w - 10.0 - w), y - font * LINE_HEIGHT * 0.5), font, self.pal[MUTED].fade(a), &label);
        }
    }

    /// A bar chart of `values`, one bar per label. Bars spring to new values
    /// and the axis rescales smoothly; bars are keyed by their label, so
    /// reordering the data moves them sideways. Hovering shows the value.
    /// Returns the index of the bar clicked.
    pub fn bar_chart(&mut self, key: &str, labels: &[&str], values: &[f32], size: Vec2) -> Option<usize> {
        let dec = self.locale.decimal;
        let id = self.declare(key);
        let count = labels.len().min(values.len());
        let font = self.theme.font * 0.8;
        let lh = font * LINE_HEIGHT;
        let target = self.alloc(size);
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        let axis = nice_axis(0.0, values[..count].iter().fold(0.0f32, |m, &v| m.max(v)));
        let (_, hi) = self.chart_scale(id, 0.0, axis.1);
        let mut label = String::new();
        short_number(axis.1, dec, &mut label);
        let axis_w = self.text.width(&label, font) + 14.0;
        let plot = Rect::new(r.x + axis_w, r.y + lh * 0.5, (r.w - axis_w).max(1.0), (r.h - lh * 1.5 - 8.0).max(1.0));
        self.chart_grid(plot, (0.0, hi), axis, axis_w, font, a);

        let slot = plot.w / count.max(1) as f32;
        let bw = (slot * 0.62).min(64.0);
        let (mut clicked, mut tip) = (None, None);
        for i in 0..count {
            let bid = id.child(labels[i]);
            let h = plot.h * (values[i].max(0.0) / hi).min(1.08);
            let x = plot.x + slot * i as f32 + (slot - bw) * 0.5;
            let (br, bl) = self.enter(bid, Rect::new(x, plot.bottom() - h, bw, h), Smooth);
            let column = Rect::new(plot.x + slot * i as f32, plot.y, slot, plot.h + lh + 8.0);
            let hit = self.hit(bid, column);
            short_number(values[i], dec, &mut label);
            self.describe(bid, Role::Label, labels[i], column, Some(&label), None);
            if hit.clicked {
                clicked = Some(i);
                self.learn(bid, labels[i]);
            }
            let (hover, press, _) = self.drive3(bid, hit.hovered, hit.held, 0.0, Snappy);
            // New bars grow out of the baseline.
            let grown = br.h * smoothstep(bl);
            let bar = Rect::new(br.x, plot.bottom() - grown, br.w, grown).scaled(1.0 - 0.04 * press);
            let color = series_color(&self.pal, 0).mix(Color::WHITE, 0.16 * hover);
            self.draw.rect(bar, (bw * 0.18).min(7.0), color.fade(a * bl.clamp(0.0, 1.0)));
            let tw = self.text.width(labels[i], font);
            let ink = self.pal[MUTED].mix(self.pal[TEXT], hover);
            self.draw.add_text(Vec2::new(br.center().x - tw * 0.5, plot.bottom() + 8.0), font, ink.fade(a), labels[i]);
            if hit.hovered {
                tip = Some((Vec2::new(bar.center().x, bar.y), vec![(Color::TRANSPARENT, format!("{}: {label}", labels[i]))]));
            }
        }
        self.chart_tip(id, tip);
        clicked
    }

    /// A line chart: one line per series `(name, values)` over the shared
    /// `labels`, the first one with its area shaded. Points spring to new
    /// values, so switching datasets morphs the lines. Hovering shows a guide
    /// with every series' value there. Returns the index under the pointer.
    pub fn line_chart(&mut self, key: &str, labels: &[&str], series: &[(&str, &[f32])], size: Vec2) -> Option<usize> {
        let dec = self.locale.decimal;
        let id = self.declare(key);
        let count = labels.len();
        let font = self.theme.font * 0.8;
        let lh = font * LINE_HEIGHT;
        let target = self.alloc(size);
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        let all = series.iter().flat_map(|(_, v)| v.iter().take(count).copied());
        let (min, max) = all.fold((0.0f32, 0.0f32), |(lo, hi), v| (lo.min(v), hi.max(v)));
        let axis = nice_axis(min, max);
        let (lo, hi) = self.chart_scale(id, axis.0, axis.1);

        // Legend on top when there is more than one series.
        let legend = if series.len() > 1 { lh + 8.0 } else { 0.0 };
        let mut x = r.x;
        for (s, (name, _)) in series.iter().enumerate().take(if legend > 0.0 { series.len() } else { 0 }) {
            let c = series_color(&self.pal, s);
            self.draw.rect(Rect::new(x, r.y + (lh - 9.0) * 0.5, 9.0, 9.0), 9.0, c.fade(a));
            self.draw.add_text(Vec2::new(x + 15.0, r.y), font, self.pal[MUTED].fade(a), name);
            x += 15.0 + self.text.width(name, font) + 18.0;
        }
        let mut label = String::new();
        short_number(axis.1, dec, &mut label);
        let axis_w = self.text.width(&label, font).max(self.text.width("-0", font)) + 14.0;
        let plot = Rect::new(r.x + axis_w, r.y + legend + lh * 0.5, (r.w - axis_w - 8.0).max(1.0), (r.h - legend - lh * 1.5 - 8.0).max(1.0));
        self.chart_grid(plot, (lo, hi), axis, axis_w, font, a);

        let step = if count > 1 { plot.w / (count - 1) as f32 } else { 0.0 };
        let xs = |i: usize| if count > 1 { plot.x + step * i as f32 } else { plot.center().x };
        let ys = |v: f32| plot.bottom() - plot.h * ((v - lo) / (hi - lo));
        // Labels along the bottom, thinned out so they never overlap.
        let widest = labels.iter().map(|l| self.text.width(l, font)).fold(0.0, f32::max);
        let every = if step > 0.0 { ((widest + 12.0) / step).ceil().max(1.0) as usize } else { 1 };
        for (i, l) in labels.iter().enumerate().filter(|(i, _)| i % every == 0) {
            let w = self.text.width(l, font);
            let lx = (xs(i) - w * 0.5).clamp(plot.x - 4.0, plot.right() - w + 4.0);
            self.draw.add_text(Vec2::new(lx, plot.bottom() + 8.0), font, self.pal[MUTED].fade(a), l);
        }

        // The points, each one a spring.
        let k = self.steps[Smooth as usize];
        let frame = self.frame;
        let mut lines: Vec<Vec<Vec2>> = Vec::with_capacity(series.len());
        for (name, values) in series {
            let sid = id.child(name);
            let mut pts = Vec::with_capacity(count);
            for i in 0..count.min(values.len()) {
                let n = self.nodes.entry(sid.with(i as u64)).or_insert_with(Node::new);
                let fresh = n.touch(frame);
                self.animating |= n.aux.drive(values[i], fresh, &k);
                // On entry the line rises from the baseline with the chart.
                let base = lo.max(0.0).min(hi);
                pts.push(Vec2::new(xs(i), ys(base + (n.aux.get() - base) * smoothstep(life))));
            }
            lines.push(pts);
        }
        if let Some(first) = lines.first().filter(|p| p.len() > 1) {
            let floor = ys(lo.max(0.0).min(hi));
            let mut area = first.clone();
            area.push(Vec2::new(first[first.len() - 1].x, floor));
            area.push(Vec2::new(first[0].x, floor));
            self.draw.polygon(&area, series_color(&self.pal, 0).with_alpha(0.14 * a));
        }
        for (s, pts) in lines.iter().enumerate() {
            let c = series_color(&self.pal, s).fade(a);
            for w in pts.windows(2) {
                self.draw.line(w[0], w[1], 2.5, c);
            }
        }

        // Hover: a guide at the nearest label, a dot on each line, a card.
        let hit = self.hit_inert(id.child("plot"), plot.inset(-6.0));
        let at = self.input.pointer.filter(|_| hit.hovered && count > 0).map(|p| {
            if count > 1 {
                (((p.x - plot.x) / step).round().max(0.0) as usize).min(count - 1)
            } else {
                0
            }
        });
        let tip = at.map(|i| {
            self.draw.line(Vec2::new(xs(i), plot.y), Vec2::new(xs(i), plot.bottom()), 1.0, self.pal[MUTED].with_alpha(0.5 * a));
            let mut rows = vec![(Color::TRANSPARENT, labels[i].to_string())];
            let mut top = plot.bottom();
            for (s, (name, values)) in series.iter().enumerate() {
                let Some(&p) = lines[s].get(i) else { continue };
                let c = series_color(&self.pal, s);
                self.draw.rect(Rect::new(p.x - 6.0, p.y - 6.0, 12.0, 12.0), 12.0, self.pal[RAISED].fade(a));
                self.draw.rect(Rect::new(p.x - 4.0, p.y - 4.0, 8.0, 8.0), 8.0, c.fade(a));
                top = top.min(p.y);
                short_number(values[i], dec, &mut label);
                rows.push((c, if series.len() > 1 { format!("{name}: {label}") } else { label.clone() }));
            }
            (Vec2::new(xs(i), top), rows)
        });
        if self.a11y_on {
            let values = self.locale.values;
            let summary =
                series.iter().map(|(n, v)| format!("{n}: {}", crate::locale::Locale::fill(values, &[&v.len()]))).collect::<Vec<_>>().join(", ");
            self.describe(id, Role::Label, key, r, Some(&summary), None);
        }
        self.chart_tip(id, tip);
        at
    }

    /// A tiny line with no axes, to show a trend inside text, a table or a
    /// card: green when it ends above where it started, red when below.
    pub fn sparkline(&mut self, values: &[f32], width: f32) {
        let id = self.auto_id();
        let h = (self.theme.font * 1.6).round();
        let target = self.alloc(Vec2::new(width, h));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        let (min, max) = values.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        if values.is_empty() {
            return;
        }
        let (lo, hi) = self.chart_scale(id, min, if max - min < 1e-6 { min + 1.0 } else { max });
        let k = self.steps[Smooth as usize];
        let frame = self.frame;
        let inner = r.inset(3.0);
        let step = if values.len() > 1 { inner.w / (values.len() - 1) as f32 } else { 0.0 };
        let mut pts = Vec::with_capacity(values.len());
        for (i, &v) in values.iter().enumerate() {
            let n = self.nodes.entry(id.with(i as u64)).or_insert_with(Node::new);
            let fresh = n.touch(frame);
            self.animating |= n.aux.drive(v, fresh, &k);
            let t = ((n.aux.get() - lo) / (hi - lo)).clamp(-0.2, 1.2);
            pts.push(Vec2::new(inner.x + step * i as f32, inner.bottom() - inner.h * t));
        }
        let rising = values[values.len() - 1] >= values[0];
        let color = Color::hex(if rising { UP } else { DOWN }).fade(a);
        if pts.len() > 1 {
            let mut area = pts.clone();
            area.push(Vec2::new(inner.right(), inner.bottom()));
            area.push(Vec2::new(inner.x, inner.bottom()));
            self.draw.polygon(&area, color.with_alpha(0.12 * color.a));
            for w in pts.windows(2) {
                self.draw.line(w[0], w[1], 1.8, color);
            }
        }
        let end = pts[pts.len() - 1];
        self.draw.rect(Rect::new(end.x - 3.5, end.y - 3.5, 7.0, 7.0), 7.0, color);
    }

    /// A grid of cells colored by intensity: `values` row by row, one per
    /// `rows` × `cols`. Colors spring to new values; hovering lifts a cell
    /// and shows its value. Returns the `(row, column)` clicked.
    pub fn heatmap(&mut self, key: &str, rows: &[&str], cols: &[&str], values: &[f32]) -> Option<(usize, usize)> {
        let dec = self.locale.decimal;
        let id = self.declare(key);
        let font = self.theme.font * 0.78;
        let lh = font * LINE_HEIGHT;
        let gap = 3.0;
        let label_w = rows.iter().map(|l| self.text.width(l, font)).fold(0.0, f32::max) + 10.0;
        let ncols = cols.len().max(1);
        let cell = ((self.available_width() - label_w) / ncols as f32 - gap).clamp(8.0, 34.0);
        let size = Vec2::new(label_w + ncols as f32 * (cell + gap), lh + 4.0 + rows.len() as f32 * (cell + gap));
        let target = self.alloc(size);
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        let max = values.iter().fold(0.0f32, |m, &v| m.max(v)).max(1e-6);
        // Column labels, thinned out when they would overlap.
        let widest = cols.iter().map(|l| self.text.width(l, font)).fold(0.0, f32::max);
        let every = ((widest + 6.0) / (cell + gap)).ceil().max(1.0) as usize;
        let x0 = r.x + label_w;
        for (c, l) in cols.iter().enumerate().filter(|(c, _)| c % every == 0) {
            let w = self.text.width(l, font);
            self.draw.add_text(Vec2::new(x0 + c as f32 * (cell + gap) + (cell - w) * 0.5, r.y), font, self.pal[MUTED].fade(a), l);
        }
        let y0 = r.y + lh + 4.0;
        let (snappy, smooth) = (self.steps[Snappy as usize], self.steps[Smooth as usize]);
        let (low, high) = (self.pal[RAISED], self.pal[ACCENT]);
        let (mut clicked, mut tip) = (None, None);
        let mut label = String::new();
        let mut lifted = None;
        for (ri, row) in rows.iter().enumerate() {
            let y = y0 + ri as f32 * (cell + gap);
            self.draw.add_text(Vec2::new(r.x, y + (cell - lh) * 0.5), font, self.pal[MUTED].fade(a), row);
            for c in 0..cols.len() {
                let v = values.get(ri * cols.len() + c).copied().unwrap_or(0.0);
                let cid = id.with((ri * cols.len() + c) as u64);
                let rect = Rect::new(x0 + c as f32 * (cell + gap), y, cell, cell);
                let hit = self.hit_inert(cid, rect);
                if hit.clicked {
                    clicked = Some((ri, c));
                }
                let n = self.nodes.entry(cid).or_insert_with(Node::new);
                let fresh = n.touch(self.frame);
                n.rect.snap(rect);
                let t = (v / max).clamp(0.0, 1.0);
                let fill = low.mix(high, 0.12 + 0.88 * t * t.sqrt());
                let anim = n.fill.drive(fill, fresh, &smooth) | n.hover.drive(hit.hovered as u8 as f32, fresh, &snappy);
                let (fill, hover) = (n.fill.get(), n.hover.get());
                self.animating |= anim;
                if hover > 0.01 {
                    lifted = Some((rect, fill, hover));
                } else {
                    self.draw.rect(rect, 4.0, fill.fade(a));
                }
                if hit.hovered {
                    short_number(v, dec, &mut label);
                    tip = Some((Vec2::new(rect.center().x, rect.y), vec![(fill, format!("{row} · {}: {label}", cols[c]))]));
                }
            }
        }
        // The hovered cell is drawn last, raised above its neighbours.
        if let Some((rect, fill, hover)) = lifted {
            let big = rect.scaled(1.0 + 0.25 * hover);
            self.draw.push(DrawCmd::Shadow {
                rect: big,
                radius: 5.0,
                blur: 10.0,
                offset: Vec2::new(0.0, 3.0),
                color: self.pal[SHADOW].fade(a * hover),
            });
            self.draw.rect_stroked(big, 5.0, fill.fade(a), self.pal[TEXT].with_alpha(0.6 * hover * a), 1.5);
        }
        self.describe(id, Role::Table, key, r, None, None);
        self.chart_tip(id, tip);
        clicked
    }

    /// A half-circle gauge for a value in `0..=1`, with the percentage in
    /// the middle and `label` under it. The arc turns amber and then red as
    /// it nears the top; the value swings there with a little overshoot.
    pub fn gauge(&mut self, key: &str, label: &str, value: f32, diameter: f32) {
        let id = self.declare(key);
        let font = self.theme.font;
        let small = font * 0.85;
        let target = self.alloc(Vec2::new(diameter, diameter * 0.5 + 14.0 + small * LINE_HEIGHT));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        let k = self.steps[Bouncy as usize];
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        self.animating |= n.aux.drive(value.clamp(0.0, 1.0), fresh, &k);
        // While entering, the arc sweeps up from zero.
        let t = n.aux.get() * smoothstep(life);
        let width = (diameter * 0.09).max(6.0);
        let c = Vec2::new(r.center().x, r.y + diameter * 0.5);
        let radius = diameter * 0.5 - width * 0.5;
        self.draw.arc(c, radius, PI, 2.0 * PI, width, self.pal[BORDER].fade(a));
        let color = self.pal[ACCENT].mix(Color::hex(WARN), smoothstep((t - 0.6) / 0.2)).mix(Color::hex(DOWN), smoothstep((t - 0.82) / 0.12));
        if t > 0.002 {
            self.draw.arc(c, radius, PI, PI + PI * t.clamp(0.0, 1.03), width, color.fade(a));
        }
        // A small marker at the tip of the arc.
        let ang = PI + PI * t.clamp(0.0, 1.03);
        let tip = c + Vec2::new(ang.cos(), ang.sin()) * radius;
        let d = width * 1.15;
        self.draw.rect(Rect::new(tip.x - d * 0.5, tip.y - d * 0.5, d, d), d, Color::WHITE.fade(a));
        let mut text = String::new();
        let _ = write!(text, "{:.0} %", value.clamp(0.0, 1.0) * 100.0);
        let big = (diameter * 0.17).clamp(font, font * 2.4);
        let w = self.text.width(&text, big);
        self.draw.add_text(Vec2::new(c.x - w * 0.5, c.y - big * LINE_HEIGHT), big, self.pal[TEXT].fade(a), &text);
        let w = self.text.width(label, small);
        self.draw.add_text(Vec2::new(c.x - w * 0.5, c.y + 14.0), small, self.pal[MUTED].fade(a), label);
        self.describe(id, Role::Label, label, r, Some(&text), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axes_end_on_round_numbers() {
        assert_eq!(nice_ceil(0.0), 1.0);
        assert_eq!(nice_ceil(7.0), 10.0);
        assert_eq!(nice_ceil(10.0), 10.0);
        assert_eq!(nice_ceil(12.0), 20.0);
        assert_eq!(nice_ceil(230.0), 250.0);
        assert_eq!(nice_ceil(0.42), 0.5);
        assert_eq!(nice_axis(0.0, 210.0), (0.0, 250.0, 50.0));
        assert_eq!(nice_axis(0.0, 610.0), (0.0, 800.0, 200.0));
        assert_eq!(nice_axis(-5.0, 5.0), (-6.0, 6.0, 2.0));
        assert_eq!(nice_axis(0.0, 0.0), (0.0, 1.0, 1.0));
        let mut s = String::new();
        short_number(12.5, ',', &mut s);
        assert_eq!(s, "12,5");
        short_number(1250.0, ',', &mut s);
        assert_eq!(s, "1,2k");
        short_number(1250.0, '.', &mut s);
        assert_eq!(s, "1.2k");
        short_number(25_000.0, ',', &mut s);
        assert_eq!(s, "25k");
        short_number(40.0, ',', &mut s);
        assert_eq!(s, "40");
    }
}
