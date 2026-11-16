//! Not doing the work for what cannot be seen: inside a scroll area, cards
//! far outside the view are left unbuilt and keep the room they last
//! took, and a list of equal rows only ever builds the ones in view.

use crate::geom::Vec2;
use crate::id::Id;
use crate::ui::{Dir, Key, Ui};

/// What is known about a container that may be left unbuilt.
#[derive(Default, Clone, Copy)]
pub(crate) struct Dormant {
    /// The width and theme sizes its size was measured with.
    stamp: u64,
    /// Frames in a row it has been built with that stamp (its size needs a
    /// few to be final).
    built: u8,
    /// It holds the focus or a press, or declares something that lives
    /// outside it (a dialog, a notice): it must keep being built.
    busy: bool,
    /// It was left unbuilt last frame.
    asleep: bool,
}

/// What [`Ui::wake`] hands to [`Ui::built`].
pub(crate) struct Waking {
    busy: bool,
    entrance: bool,
    waking: bool,
}

impl Ui {
    /// Whether cards far outside the view of their scroll area are left
    /// unbuilt (on by default). An unbuilt card costs nothing and keeps the
    /// size it had when it was last seen, so content that changes size
    /// while out of view only takes its new room when it comes near again;
    /// its elements are not found by [`find`](Self::find) meanwhile. Cards
    /// holding the focus, a press, a dialog or a notice are always built,
    /// and everything is while a screen reader listens, a menu or dialog is
    /// open, Tab is pressed or a [`scroll_to`](Self::scroll_to) is pending.
    pub fn set_lazy(&mut self, on: bool) {
        self.lazy = on;
    }

    /// The width and the theme sizes a container is laid out with: when
    /// they change, the size it had is no longer its size.
    pub(crate) fn layout_stamp(&self, width: f32) -> u64 {
        let t = &self.theme;
        let mut h = width.to_bits() as u64;
        for v in [t.font, t.pad, t.gap, t.control, t.radius] {
            h = (h ^ v.to_bits() as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
            h ^= h >> 31;
        }
        h | 1
    }

    /// The size of container `id` if it can be left unbuilt this frame:
    /// far from the view of its scroll area, of a size already known for
    /// this layout, and with nothing in it that must go on.
    pub(crate) fn rest(&mut self, id: Id, stamp: u64) -> Option<Vec2> {
        if !self.lazy
            || self.a11y_on
            || self.scroll_request.is_some()
            || self.open.is_some()
            || self.collect_heroes
            || !self.blockers_prev.is_empty()
            || self.keys.contains(&Key::Tab)
        {
            return None;
        }
        let view = self.scrolls.last()?.view;
        let size = *self.sizes.get(&id)?;
        let at = self.peek(size);
        let d = self.dormant.get_mut(&id)?;
        if d.stamp != stamp || d.built < 3 || d.busy {
            return None;
        }
        let margin = (view.h * 0.5).clamp(120.0, 400.0);
        if at.y + size.y < view.y - margin || at.y > view.bottom() + margin {
            d.asleep = true;
            Some(size)
        } else {
            None
        }
    }

    /// Starts building container `id`. If it was left unbuilt until now,
    /// what it holds appears as it was, with no entrance.
    pub(crate) fn wake(&mut self, id: Id) -> Waking {
        let waking = !self.scrolls.is_empty() && self.dormant.get(&id).is_some_and(|d| d.asleep);
        let outer = Waking { busy: std::mem::replace(&mut self.busy, false), entrance: self.skip_entrance, waking: self.waking };
        self.skip_entrance |= waking;
        self.waking |= waking;
        outer
    }

    /// Container `id` has been built with `stamp`.
    pub(crate) fn built(&mut self, id: Id, stamp: u64, outer: Waking) {
        if !self.scrolls.is_empty() {
            let d = self.dormant.entry(id).or_default();
            if d.stamp != stamp {
                d.stamp = stamp;
                d.built = 0;
            }
            d.built = d.built.saturating_add(1);
            d.busy = self.busy;
            d.asleep = false;
        }
        self.busy |= outer.busy;
        self.skip_entrance = outer.entrance;
        self.waking = outer.waking;
    }

    /// A scroll area `height` tall over `count` rows, each `row_height`
    /// tall, that only builds the rows in view: its cost is that of what is
    /// seen, whether it holds a hundred rows or a hundred thousand.
    /// `f(ui, i)` builds row `i`, laid out as a row centered in its height
    /// and as wide as the list; its keys are namespaced under the row. Rows
    /// that scroll into view appear as they are, with no entrance. Rows out
    /// of view do not exist: Tab and [`find`](Self::find) only reach the
    /// ones in view. Positions are exact up to a few tens of thousands of
    /// rows; far beyond that, rows may sit a fraction of a pixel off.
    pub fn virtual_list(&mut self, key: &str, count: usize, row_height: f32, height: f32, mut f: impl FnMut(&mut Ui, usize)) {
        let id = self.id(key);
        let seen = self.nodes.get(&id).is_some_and(|n| n.present(self.frame));
        self.scroll(key, height, |ui| {
            let Some(c) = ui.scrolls.last().copied() else { return };
            let origin = ui.peek(Vec2::ZERO);
            let width = ui.available_width();
            let step = row_height.max(1.0);
            let first = (((c.view.y - origin.y) / step).floor().max(0.0) as usize).min(count);
            let last = (((c.view.bottom() - origin.y) / step).ceil().max(0.0) as usize).min(count);
            let entrance = ui.skip_entrance;
            ui.skip_entrance |= seen;
            let gap = ui.theme.gap;
            for i in first..last {
                let rid = id.with(i as u64);
                let inner = ui.sizes.get(&rid).map_or(0.0, |s| s.y);
                let at = Vec2::new(origin.x, origin.y + i as f32 * step + ((step - inner) * 0.5).max(0.0));
                ui.next_width = Some(width);
                ui.group(rid, Dir::Row, Vec2::ZERO, gap, Some(at), |ui| ui.within(rid, |ui| f(ui, i)));
            }
            ui.skip_entrance = entrance;
            // The room of every row, built or not.
            ui.space(count as f32 * step);
        });
    }
}
