//! Arranging space: two panes with a draggable divider, and a board of
//! columns whose cards are dragged from one to another.

use crate::a11y::Role;
use crate::anim::SpringKind::{Smooth, Snappy};
use crate::draw::DrawCmd;
use crate::geom::{Rect, Vec2};
use crate::navigate::ellipsize;
use crate::theme::{ACCENT, BORDER, MUTED, RAISED, SURFACE, TEXT};
use crate::ui::{Dir, Ui, LINE_HEIGHT};

/// A card being dragged on a [`Ui::kanban`] board.
pub(crate) struct Drag {
    col: usize,
    index: usize,
    /// Where the card was grabbed, from its top-left corner.
    grab: Vec2,
}

impl Ui {
    /// Two panes side by side, `height` tall, split by a divider that can be
    /// dragged (or moved with the arrows once focused). `ratio` is the
    /// share of the first pane, kept within `0.15..=0.85`; its content
    /// reflows as it changes. `f(ui, i)` builds pane `i`. Returns `true`
    /// while the divider moves.
    pub fn splitter(&mut self, key: &str, ratio: &mut f32, height: f32, mut f: impl FnMut(&mut Ui, usize)) -> bool {
        const HANDLE: f32 = 14.0;
        let id = self.declare(key);
        let target = self.alloc(Vec2::new(self.available_width(), height));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        let span = (r.w - HANDLE).max(1.0);
        let hid = id.child("divisor");
        let before = *ratio;
        *ratio = ratio.clamp(0.15, 0.85);
        let shown_x = self.nodes.get(&hid).map_or(r.x + span * *ratio, |n| r.x + span * n.aux.get());
        let handle = Rect::new(shown_x, r.y, HANDLE, r.h);
        self.enter(hid, handle, Snappy);
        let hit = self.hit(hid, handle);
        self.describe_level(hid, Role::Slider, key, handle, *ratio);
        let pointer = self.input.pointer;
        let n = self.nodes.get_mut(&hid).unwrap();
        if let (true, Some(p)) = (hit.held, pointer) {
            if !n.latch {
                n.latch = true;
                n.stamp = (p.x - shown_x) as f64;
            }
            *ratio = ((p.x - n.stamp as f32 - r.x) / span).clamp(0.15, 0.85);
        } else {
            n.latch = false;
        }
        *ratio = (*ratio + self.arrows(hid) * 0.05).clamp(0.15, 0.85);
        if hit.released {
            self.learn(hid, key);
        }
        // Dragging follows the pointer exactly; anything else glides.
        let snappy = self.steps[Snappy as usize];
        let n = self.nodes.get_mut(&hid).unwrap();
        let first = n.fresh();
        let anim = n.aux.drive(*ratio, first || hit.held, &snappy) | n.hover.drive((hit.hovered || hit.held) as u8 as f32, first, &snappy);
        let (t, hover) = (n.aux.get(), n.hover.get());
        self.animating |= anim;

        let x = r.x + span * t;
        let panes = [Rect::new(r.x, r.y, x - r.x, r.h), Rect::new(x + HANDLE, r.y, r.right() - x - HANDLE, r.h)];
        let radius = self.theme.radius * 0.7;
        let inset = self.theme.pad * 0.8;
        for (i, pane) in panes.into_iter().enumerate() {
            self.draw.rect_stroked(pane, radius, self.pal[RAISED].with_alpha(0.5 * a), self.pal[BORDER].fade(a), 1.0);
            let (clip, cull) = (self.clip, self.draw.cull);
            let view = clip.map_or(pane, |c| c.intersect(&pane));
            self.clip = Some(view);
            self.draw.push(DrawCmd::PushClip { rect: pane, radius });
            self.next_width = Some((pane.w - inset * 2.0).max(0.0));
            let gap = self.theme.gap;
            self.group(id.with(i as u64), Dir::Column, Vec2::splat(inset), gap, Some(pane.min()), |ui| ui.within(id.with(i as u64), |ui| f(ui, i)));
            self.draw.push(DrawCmd::PopClip);
            self.clip = clip;
            self.draw.cull = cull;
        }
        // The grip: a pill that widens under the pointer.
        let grip_h = 36.0 + 10.0 * hover;
        let gw = 4.0 + 2.0 * hover;
        let c = Vec2::new(x + HANDLE * 0.5, r.center().y);
        let color = self.pal[MUTED].mix(self.pal[ACCENT], hover).with_alpha(0.5 + 0.5 * hover);
        self.draw.rect(Rect::new(c.x - gw * 0.5, c.y - grip_h * 0.5, gw, grip_h), gw, color.fade(a));
        *ratio != before || hit.held
    }

    /// A board of columns (`titles`) holding cards (`columns`, one list per
    /// title). Drag a card within its column or to another one: it follows
    /// the pointer, the cards it passes make room, and on release it settles
    /// into place. A card's text is its identity, so keep them unique.
    /// Returns `true` when a card moved.
    pub fn kanban(&mut self, key: &str, titles: &[&str], columns: &mut [Vec<String>]) -> bool {
        let id = self.declare(key);
        let ncols = titles.len().min(columns.len());
        if ncols == 0 {
            return false;
        }
        let font = self.theme.font * 0.95;
        let (gap, pad) = (12.0, 10.0);
        let card_h = (self.theme.control * 1.1).round();
        let step = card_h + 8.0;
        let head = font * LINE_HEIGHT + 18.0;
        let width = self.available_width();
        let col_w = ((width - gap * (ncols - 1) as f32) / ncols as f32).max(60.0);
        let lane_x = |c: usize, x0: f32| x0 + c as f32 * (col_w + gap);

        // Where the pointer is, and what is being dragged.
        let pointer = self.input.pointer;
        let origin = self.peek(Vec2::ZERO);
        let card_at = |c: usize, i: usize, o: Vec2| Rect::new(lane_x(c, o.x) + pad, o.y + head + i as f32 * step, col_w - pad * 2.0, card_h);
        if !self.drags.contains_key(&id) && self.input.pressed && self.active.is_none() {
            if let Some(p) = pointer {
                let found = (0..ncols).find_map(|c| (0..columns[c].len()).find(|&i| card_at(c, i, origin).contains(p)).map(|i| (c, i)));
                if let Some((col, index)) = found {
                    let rect = card_at(col, index, origin);
                    if self.pointer_in(rect) {
                        self.drags.insert(id, Drag { col, index, grab: p - rect.min() });
                        self.active = Some(id);
                    }
                }
            }
        }
        // While dragging: the column under the pointer and the slot in it.
        let hover = self.drags.get(&id).zip(pointer).map(|(d, p)| {
            let col = (0..ncols).min_by(|&a, &b| {
                let da = (lane_x(a, origin.x) + col_w * 0.5 - p.x).abs();
                let db = (lane_x(b, origin.x) + col_w * 0.5 - p.x).abs();
                da.total_cmp(&db)
            });
            let col = col.unwrap_or(d.col);
            let len = columns[col].len() - (col == d.col) as usize;
            let top = p.y - d.grab.y - origin.y - head;
            let slot = ((top / step).round().max(0.0) as usize).min(len);
            (col, slot)
        });
        let mut changed = false;
        if let Some(d) = self.drags.get(&id).filter(|_| !self.input.down) {
            let (from_col, from) = (d.col, d.index);
            if let Some((to_col, slot)) = hover {
                if from < columns[from_col].len() && (to_col != from_col || slot != from) {
                    let card = columns[from_col].remove(from);
                    self.learn(id.child(&card), &card);
                    columns[to_col].insert(slot.min(columns[to_col].len()), card);
                    changed = true;
                }
            }
            self.drags.remove(&id);
        }
        let drag = self.drags.get(&id).map(|d| (d.col, d.index, d.grab));

        // The lanes, as tall as the fullest one (plus room for a card coming in).
        let most =
            (0..ncols).map(|c| columns[c].len() + hover.is_some_and(|(hc, _)| hc == c && drag.is_some_and(|d| d.0 != c)) as usize).max().unwrap_or(0);
        let height = head + most.max(1) as f32 * step + pad;
        let target = self.alloc(Vec2::new(width, height));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        let small = font * 0.85;
        let mut lifted = None;
        for c in 0..ncols {
            let lane = Rect::new(lane_x(c, r.x), r.y, col_w, r.h);
            let target_lane = hover.is_some_and(|(hc, _)| hc == c) && drag.is_some();
            let lid = id.child("lane").with(c as u64);
            self.enter(lid, lane, Smooth);
            let (_, _, glow) = self.drive3(lid, false, false, target_lane as u8 as f32, Smooth);
            let stroke = self.pal[BORDER].mix(self.pal[ACCENT], glow.clamp(0.0, 1.0));
            self.draw.rect_stroked(lane, self.theme.radius * 0.7, self.pal[SURFACE].mix(self.pal[RAISED], 0.4).fade(a), stroke.fade(a), 1.0 + glow);
            self.draw.add_text(Vec2::new(lane.x + pad + 4.0, lane.y + 9.0), font, self.pal[TEXT].fade(a), titles[c]);
            let count = columns[c].len().to_string();
            let cw = self.text.width(&count, small) + 14.0;
            let pill = Rect::new(lane.right() - pad - cw, lane.y + 10.0, cw, small * LINE_HEIGHT + 2.0);
            self.draw.rect(pill, pill.h, self.pal[TEXT].with_alpha(0.08 * a));
            self.draw.add_text(Vec2::new(pill.x + 7.0, pill.y + 1.0), small, self.pal[MUTED].fade(a), &count);
            let mut pos = 0;
            for (i, text) in columns[c].iter().enumerate() {
                let cid = id.child(text);
                if drag.is_some_and(|(dc, di, _)| dc == c && di == i) {
                    let grab = drag.unwrap().2;
                    let at = pointer.map_or(card_at(c, i, r.min()).min(), |p| p - grab);
                    let held = Rect::new(at.x, at.y, col_w - pad * 2.0, card_h);
                    self.enter(cid, held, Smooth);
                    self.nodes.get_mut(&cid).unwrap().rect.snap(held);
                    lifted = Some((held, text.clone()));
                    continue;
                }
                // Make room where the dragged card would land.
                if hover.is_some_and(|(hc, slot)| hc == c && slot == pos) && drag.is_some() {
                    pos += 1;
                }
                let (cr, cl) = self.enter(cid, card_at(c, pos, r.min()), Smooth);
                self.describe(cid, Role::ListItem, text, cr, Some(titles[c]), None);
                let over = drag.is_none() && pointer.is_some_and(|p| self.pointer_in(cr) && cr.contains(p));
                let (h, _, _) = self.drive3(cid, over, false, 0.0, Snappy);
                let label = ellipsize(self, text, cr.w - 54.0, self.theme.font);
                self.row_card(cr, &label, self.theme.font, h, a * cl.clamp(0.0, 1.0), false);
                pos += 1;
            }
        }
        if let Some((rect, text)) = lifted {
            // The card in hand floats above everything else.
            let below = self.draw.set_layer(1);
            let cull = self.draw.cull.take();
            let label = ellipsize(self, &text, rect.w - 54.0, self.theme.font);
            self.row_card(rect.scaled(1.03), &label, self.theme.font, 1.0, a, true);
            self.draw.cull = cull;
            self.draw.set_layer(below);
            self.animating = true;
        }
        changed
    }
}
