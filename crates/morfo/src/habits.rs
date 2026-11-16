//! The adaptive layer at the level of the interface: habits with names, the
//! next step predicted, an adaptive home screen, adaptive density, and exit
//! animations for anything.

use crate::adapt::{Prominence, Reason};
use crate::anim::smoothstep;
use crate::anim::SpringKind::{Smooth, Snappy};
use crate::buttons::Icon;
use crate::geom::{Rect, Vec2};
use crate::id::Id;
use crate::ui::{Dir, Ui};
use crate::widgets::{Size, Variant};

/// Something the profile learned, as a person would read it.
#[derive(Clone, Debug, PartialEq)]
pub struct Habit {
    pub id: Id,
    /// The element's label (or key) as last seen.
    pub label: String,
    pub score: f32,
    pub count: u32,
    pub reason: Option<Reason>,
}

impl Ui {
    /// Records a use of `id` and remembers its name for [`habits`](Self::habits).
    pub(crate) fn learn(&mut self, id: Id, label: &str) {
        self.profile.record(id, self.now);
        if !self.profile.frozen && !label.is_empty() && self.labels.get(&id).is_none_or(|l| l != label) {
            self.labels.insert(id, label.to_string());
        }
    }

    /// What the interface has learned about this person, most-used first,
    /// with names and the reason each one ranks where it does. For a
    /// "your habits" screen where people can see, forget or freeze it.
    pub fn habits(&self) -> Vec<Habit> {
        self.profile
            .entries(self.now)
            .into_iter()
            .map(|(id, score, count)| Habit {
                id,
                label: self.labels.get(&id).cloned().unwrap_or_else(|| format!("{:04x}", id.0 & 0xffff)),
                score,
                count,
                reason: self.profile.explain(id, self.now),
            })
            .collect()
    }

    /// Name the interface knows element `id` by.
    pub fn label_of(&self, id: Id) -> Option<&str> {
        self.labels.get(&id).map(String::as_str)
    }

    /// Why element `id` ranks where it does (see [`Reason`]).
    pub fn why(&self, id: Id) -> Option<Reason> {
        self.profile.explain(id, self.now)
    }

    /// Puts `text` on the clipboard (through [`take_clipboard`](Self::take_clipboard)).
    pub fn copy_to_clipboard(&mut self, text: &str) {
        self.clipboard_out = Some(text.to_string());
    }

    /// The step this person usually takes next, offered as a button once
    /// the pattern is clear: after "Nuevo" they always "Abrir"? Then
    /// "Abrir" appears right away. `items` are `(key, label)` of the group
    /// `source` (as in [`adaptive_actions`](Self::adaptive_actions)).
    /// Returns the index of the item when clicked.
    pub fn predicted_action(&mut self, key: &str, title: &str, source: &str, items: &[(&str, &str)]) -> Option<usize> {
        let id = self.id(key);
        let src = self.id(source);
        let guess = self.profile.predict(self.now).and_then(|(next, _)| items.iter().position(|(k, _)| src.child(k) == next));
        // Keep showing what was predicted while it animates away.
        let n = self.nodes.entry(id).or_insert_with(crate::ui::Node::new);
        if let Some(i) = guess {
            n.index = i;
        }
        let shown = n.index.min(items.len().saturating_sub(1));
        let mut clicked = None;
        self.reveal_id(id.child("reveal"), guess.is_some(), |ui| {
            ui.row(|ui| {
                ui.muted(title);
                let (item, label) = items[shown];
                if ui.button_icon_id(id.child(item), Icon::ArrowRight, label, Variant::Soft, Size::Sm).clicked {
                    ui.learn(src.child(item), label);
                    clicked = Some(shown);
                }
            });
        });
        clicked
    }

    /// An adaptive home screen: tiles ordered by how much this person uses
    /// them, the most used ones twice as wide. `tiles` are `(key, label)`;
    /// `f(ui, i, prominence)` draws the content of tile `i` and returns
    /// `true` when it was used (opened, clicked...). Nothing moves while the
    /// pointer is over the grid.
    pub fn adaptive_dashboard(&mut self, key: &str, tiles: &[(&str, &str)], columns: usize, mut f: impl FnMut(&mut Ui, usize, Prominence) -> bool) {
        let group = self.id(key);
        let cols = columns.max(1);
        let mut order = self.orders.remove(&group).unwrap_or_default();
        if order.len() != tiles.len() {
            order.clear();
            order.extend((0..tiles.len() as u32).map(|i| (i, Prominence::Regular)));
        }
        let now = self.now;
        let busy = self.rect_of(group).zip(self.input.pointer).is_some_and(|(r, p)| r.contains(p));
        if !busy {
            self.profile.arrange(&mut order, now, 0.2, |(i, _)| group.child(tiles[i as usize].0));
            for (i, level) in order.iter_mut() {
                *level = self.profile.prominence(group.child(tiles[*i as usize].0), now);
            }
        }
        // Pack the tiles into rows: a featured tile takes two columns.
        let span = |level: Prominence| if level == Prominence::Featured && cols > 1 { 2 } else { 1 };
        let mut rows: Vec<Vec<(u32, Prominence)>> = vec![Vec::new()];
        let mut used = 0;
        for &(i, level) in &order {
            let s = span(level);
            if used + s > cols {
                rows.push(Vec::new());
                used = 0;
            }
            rows.last_mut().unwrap().push((i, level));
            used += s;
        }
        let gap = self.theme.gap;
        let cell = ((self.available_width() - gap * (cols - 1) as f32) / cols as f32).max(0.0);
        let mut used_tiles = Vec::new();
        let id = self.auto_id();
        // What the tiles' own controls record is the tile's use, not theirs.
        let learning = self.profile.frozen;
        self.profile.frozen = true;
        let (origin, size) = self.group(id, Dir::Column, Vec2::ZERO, gap, None, |ui| {
            for row in &rows {
                ui.row_top(|ui| {
                    for &(i, level) in row {
                        let (tile, _) = tiles[i as usize];
                        let width = cell * span(level) as f32 + gap * (span(level) - 1) as f32;
                        let mut used = false;
                        ui.card_w(tile, width, |ui| used = f(ui, i as usize, level));
                        if used {
                            used_tiles.push(i as usize);
                        }
                    }
                });
            }
        });
        self.profile.frozen = learning;
        self.advance(size);
        self.enter(group, Rect::from_min_size(origin, size), Snappy);
        for i in used_tiles {
            self.learn(group.child(tiles[i].0), tiles[i].1);
        }
        self.orders.insert(group, order);
    }

    /// Lets the interface grow more compact as this person uses it more
    /// (spacing and control heights shrink up to 20 %, never the text).
    pub fn set_adaptive_density(&mut self, on: bool) {
        self.auto_density = on;
    }

    /// Current density: 1 is the theme as designed, lower is more compact.
    pub fn density(&self) -> f32 {
        self.density.get()
    }

    /// Content that animates out as well as in, for anything: while `shown`
    /// it is built as usual (fading and growing in); once it is not, its
    /// last appearance shrinks and fades away while what follows closes
    /// the gap — even though `f` is no longer called.
    pub fn transient(&mut self, key: &str, shown: bool, f: impl FnOnce(&mut Ui)) {
        let id = self.id(key);
        let p = self.presence_id(id.child("presence"), shown, Smooth).clamp(0.0, 1.0);
        let gap = self.theme.gap;
        if shown {
            let mark = self.draw.mark();
            let saved = self.alpha;
            self.alpha *= smoothstep(p);
            let (origin, size) = self.group(id, Dir::Column, Vec2::ZERO, gap, None, f);
            self.alpha = saved;
            let mut snap = self.transients.remove(&id).unwrap_or_default();
            snap.capture(self.draw.cmds_from(mark), &self.draw, Rect::from_min_size(origin, size));
            self.transients.insert(id, snap);
            self.advance(Vec2::new(size.x, (size.y + gap) * p - gap));
            return;
        }
        let Some(snap) = self.transients.remove(&id) else { return };
        if p <= 0.001 {
            return;
        }
        let size = snap.bounds.size();
        let origin = self.peek(size);
        let to = Rect::from_min_size(origin, size).scaled(0.94 + 0.06 * p);
        // It folds into its shrinking slot instead of overlapping what
        // moves up to take its place.
        let slot = Rect::new(origin.x - 40.0, origin.y - 8.0, self.available_width() + 80.0, size.y * p + 8.0);
        self.draw.push(crate::draw::DrawCmd::PushClip { rect: slot, radius: 0.0 });
        self.replay(&snap, to, smoothstep(p), None);
        self.draw.push(crate::draw::DrawCmd::PopClip);
        self.advance(Vec2::new(size.x, (size.y + gap) * p - gap));
        self.transients.insert(id, snap);
    }
}
