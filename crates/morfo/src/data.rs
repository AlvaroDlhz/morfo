//! Data components: a sortable table and a list reordered by dragging.

use std::cmp::Ordering;

use crate::anim::SpringKind::{Smooth, Snappy};
use crate::buttons::{draw_icon, Icon};
use crate::draw::DrawCmd;
use crate::geom::{Color, Rect, Vec2};
use crate::theme::{ACCENT, BORDER, MUTED, RAISED, SURFACE, TEXT};
use crate::ui::{Ui, LINE_HEIGHT};

/// The number in a cell such as "1250 €" or "12,5 %", if it holds one.
fn number(s: &str) -> Option<f64> {
    let t: String = s.chars().filter(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-')).collect();
    if t.is_empty() {
        return None;
    }
    t.replace(',', ".").parse().ok()
}

impl Ui {
    /// A table. Clicking a header sorts by that column (ascending,
    /// descending, then back to the original order) and the rows travel to
    /// their new places. Columns holding numbers sort numerically and align
    /// right. Returns the index (into `rows`) of the row clicked.
    pub fn table(&mut self, key: &str, headers: &[&str], rows: &[&[&str]], sort: &mut Option<(usize, bool)>) -> Option<usize> {
        let id = self.id(key);
        let cols = headers.len();
        let (font, pad) = (self.theme.font * 0.95, 14.0);
        let row_h = (self.theme.control * 0.95).round();
        let numeric: Vec<bool> = (0..cols).map(|c| !rows.is_empty() && rows.iter().all(|r| r.get(c).is_some_and(|v| number(v).is_some()))).collect();
        let mut widths: Vec<f32> = (0..cols)
            .map(|c| {
                let cells = rows.iter().filter_map(|r| r.get(c)).map(|v| self.text.width(v, font)).fold(0.0, f32::max);
                cells.max(self.text.width(headers[c], font) + 22.0) + pad * 2.0
            })
            .collect();
        let width = self.available_width();
        let used: f32 = widths.iter().sum();
        if let Some(last) = widths.last_mut() {
            *last += (width - used).max(0.0);
        }

        // Display order.
        let mut order: Vec<usize> = (0..rows.len()).collect();
        if let Some((c, desc)) = *sort {
            let cell = |i: usize| rows[i].get(c).copied().unwrap_or("");
            order.sort_by(|&a, &b| {
                let o = match (number(cell(a)), number(cell(b))) {
                    (Some(x), Some(y)) if numeric[c] => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
                    _ => cell(a).to_lowercase().cmp(&cell(b).to_lowercase()),
                };
                if desc {
                    o.reverse()
                } else {
                    o
                }
            });
        }

        let target = self.alloc(Vec2::new(width, row_h * (rows.len() + 1) as f32));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        let radius = self.theme.radius * 0.7;
        self.draw.rect_stroked(r, radius, self.pal[SURFACE].fade(a), self.pal[BORDER].fade(a), 1.0);
        let header = Rect::new(r.x, r.y, r.w, row_h);
        self.draw.rect(header.inset(1.0), radius - 1.0, self.pal[RAISED].fade(a));
        self.draw.rect(Rect::new(r.x, header.bottom() - 1.0, r.w, 1.0), 0.0, self.pal[BORDER].fade(a));

        let ty = |rect: Rect| rect.y + (rect.h - font * LINE_HEIGHT) * 0.5;
        let mut x = r.x;
        for (c, title) in headers.iter().enumerate() {
            let cell = Rect::new(x, header.y, widths[c], row_h);
            let hid = id.child("header").with(c as u64);
            self.enter(hid, cell, Snappy);
            let hit = self.hit(hid, cell);
            self.describe(hid, crate::a11y::Role::Button, title, cell, None, sort.map(|(s, _)| s == c));
            if hit.clicked {
                *sort = match *sort {
                    Some((s, false)) if s == c => Some((c, true)),
                    Some((s, true)) if s == c => None,
                    _ => Some((c, false)),
                };
                self.profile.record(hid, self.now);
            }
            let active = sort.is_some_and(|(s, _)| s == c);
            let (hover, _, on) = self.drive3(hid, hit.hovered, hit.held, active as u8 as f32, Smooth);
            let ink = self.pal[MUTED].mix(self.pal[TEXT], (hover + on).min(1.0));
            let tw = self.text.width(title, font);
            let tx = if numeric[c] { cell.right() - pad - tw } else { cell.x + pad };
            self.draw.add_text(Vec2::new(tx, ty(cell)), font, ink.fade(a), title);
            if let Some((_, desc)) = sort.filter(|(s, _)| *s == c) {
                let ix = if numeric[c] { tx - 12.0 } else { tx + tw + 12.0 };
                let icon = if desc { Icon::ArrowDown } else { Icon::ArrowUp };
                draw_icon(&mut self.draw, icon, Vec2::new(ix, cell.center().y), font * 0.8 * on.min(1.0), self.pal[ACCENT].fade(a * on.min(1.0)));
            }
            x += widths[c];
        }

        let mut clicked = None;
        for (pos, &i) in order.iter().enumerate() {
            let rid = id.child("row").with(i as u64);
            let target = Rect::new(r.x, r.y + row_h * (pos + 1) as f32, r.w, row_h);
            let (rr, rlife) = self.enter(rid, target, Smooth);
            let hit = self.hit(rid, rr);
            self.describe(rid, crate::a11y::Role::Row, rows[i].first().copied().unwrap_or(""), rr, None, None);
            if hit.clicked {
                clicked = Some(i);
                self.profile.record(rid, self.now);
            }
            let (hover, _, _) = self.drive3(rid, hit.hovered, hit.held, 0.0, Snappy);
            let ra = a * rlife;
            let zebra = if pos % 2 == 1 { 0.03 } else { 0.0 };
            let fill = self.pal[TEXT].with_alpha(zebra).mix(self.pal[ACCENT].with_alpha(0.12), hover);
            self.draw.rect(rr.inset(1.0), 6.0, fill.fade(ra));
            let mut x = rr.x;
            for (c, w) in widths.iter().enumerate() {
                let v = rows[i].get(c).copied().unwrap_or("");
                let cell = Rect::new(x, rr.y, *w, row_h);
                let tx = if numeric[c] { cell.right() - pad - self.text.width(v, font) } else { cell.x + pad };
                let ink = if c == 0 { self.pal[TEXT] } else { self.pal[TEXT].mix(self.pal[MUTED], 0.35) };
                self.draw.add_text(Vec2::new(tx, ty(cell)), font, ink.fade(ra), v);
                x += w;
            }
        }
        clicked
    }

    /// A list whose items are reordered by dragging: the item follows the
    /// pointer and the others make room. `label` gives each item's text,
    /// which is also its identity (keep it unique). Returns `true` when the
    /// order changed.
    pub fn reorderable<T>(&mut self, key: &str, items: &mut [T], label: impl Fn(&T) -> &str) -> bool {
        let id = self.id(key);
        let count = items.len();
        let (font, row_h, gap) = (self.theme.font, (self.theme.control * 1.05).round(), 6.0);
        let step = row_h + gap;
        let width = self.available_width();
        let target = self.alloc(Vec2::new(width, (count as f32 * step - gap).max(0.0)));
        let (r, life) = self.enter(id, target, Smooth);
        let pointer = self.input.pointer;
        let over = pointer.is_some_and(|p| self.pointer_in(r) && r.contains(p));

        // Grab on press, drop on release.
        let n = self.nodes.get_mut(&id).unwrap();
        let mut changed = false;
        if !n.open && self.input.pressed && over && self.active.is_none() && count > 0 {
            let p = pointer.unwrap();
            let i = (((p.y - r.y) / step).floor().max(0.0) as usize).min(count - 1);
            n.open = true;
            n.index = i;
            n.stamp = (p.y - (r.y + i as f32 * step)) as f64;
            self.active = Some(id);
        }
        let n = self.nodes.get_mut(&id).unwrap();
        let (dragging, from, grab) = (n.open, n.index.min(count.saturating_sub(1)), n.stamp as f32);
        let slot = match (dragging, pointer) {
            (true, Some(p)) => (((p.y - grab - r.y) / step).round().max(0.0) as usize).min(count.saturating_sub(1)),
            _ => from,
        };
        if dragging && !self.input.down {
            n.open = false;
            if slot != from {
                if slot > from {
                    items[from..=slot].rotate_left(1);
                } else {
                    items[slot..=from].rotate_right(1);
                }
                changed = true;
                self.profile.record(id, self.now);
            }
        }
        let dragging = dragging && self.input.down;

        let a = self.alpha * life;
        let mut lifted = None;
        for (i, item) in items.iter().enumerate() {
            let text = label(item);
            let rid = id.child(text);
            // Where it goes while another one is being moved.
            let pos = if !dragging || i == from {
                i
            } else {
                let without = if i > from { i - 1 } else { i };
                if without >= slot {
                    without + 1
                } else {
                    without
                }
            };
            let target = Rect::new(r.x, r.y + pos as f32 * step, r.w, row_h);
            let (rr, rl) = self.enter(rid, target, Smooth);
            self.describe(rid, crate::a11y::Role::ListItem, text, rr, None, None);
            if dragging && i == from {
                let y = pointer.map_or(rr.y, |p| p.y - grab);
                let held = Rect::new(r.x, y, r.w, row_h);
                self.nodes.get_mut(&rid).unwrap().rect.snap(held);
                lifted = Some((held, text));
                continue;
            }
            let hovered = !dragging && pointer.is_some_and(|p| self.pointer_in(rr) && rr.contains(p));
            let (hover, _, _) = self.drive3(rid, hovered, false, 0.0, Snappy);
            self.row_card(rr, text, font, hover, a * rl, false);
        }
        if let Some((rect, text)) = lifted {
            // The moved item floats above the rest.
            self.row_card(rect.scaled(1.02), text, font, 1.0, a, true);
            self.animating = true;
        }
        changed
    }

    fn row_card(&mut self, r: Rect, text: &str, font: f32, hover: f32, a: f32, lifted: bool) {
        let radius = self.theme.radius * 0.6;
        if lifted {
            self.draw.push(DrawCmd::Shadow { rect: r, radius, blur: 22.0, offset: Vec2::new(0.0, 10.0), color: Color::BLACK.with_alpha(0.3 * a) });
        }
        let stroke = self.pal[BORDER].mix(self.pal[ACCENT], if lifted { 0.8 } else { 0.0 });
        self.draw.rect_stroked(r, radius, self.pal[RAISED].mix(self.pal[TEXT], 0.04 * hover).fade(a), stroke.fade(a), 1.0);
        let handle = Vec2::new(r.x + 20.0, r.center().y);
        draw_icon(&mut self.draw, Icon::Menu, handle, font * 0.85, self.pal[MUTED].mix(self.pal[TEXT], hover).fade(a));
        self.draw.add_text(Vec2::new(r.x + 42.0, r.y + (r.h - font * LINE_HEIGHT) * 0.5), font, self.pal[TEXT].fade(a), text);
    }
}
