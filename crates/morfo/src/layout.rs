//! Layout: rows and columns, groups, reveals, presence and the
//! bookkeeping that turns declared sizes into target rectangles.

use crate::anim::{smoothstep, SpringKind};
use crate::draw::DrawCmd;
use crate::geom::{Rect, Vec2};
use crate::id::Id;
use crate::ui::{Node, Ui};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Dir {
    Row,
    /// A row whose children align to its top instead of its middle.
    RowTop,
    Column,
    /// Cells of equal width, `n` per row, filled left to right.
    Grid(u16),
}

/// Horizontal placement for [`Ui::align`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

pub(crate) struct Layout {
    pub(crate) id: Id,
    pub(crate) dir: Dir,
    pub(crate) origin: Vec2,
    pub(crate) cursor: f32,
    pub(crate) gap: f32,
    pub(crate) cross: f32,
    pub(crate) prev_cross: f32,
    pub(crate) count: u64,
    pub(crate) width: f32,
    /// Where the content after a [`Ui::fill`] starts, in this row.
    pub(crate) fill_from: Option<f32>,
}

impl Ui {
    /// Lays children out left to right, vertically centered.
    pub fn row(&mut self, f: impl FnOnce(&mut Ui)) {
        let id = self.auto_id();
        let gap = self.theme.gap;
        let (_, size) = self.group(id, Dir::Row, Vec2::ZERO, gap, None, f);
        self.advance(size);
    }

    /// Lays children out left to right, aligned to the top.
    pub fn row_top(&mut self, f: impl FnOnce(&mut Ui)) {
        let id = self.auto_id();
        let gap = self.theme.gap;
        let (_, size) = self.group(id, Dir::RowTop, Vec2::ZERO, gap, None, f);
        self.advance(size);
    }

    /// Lays children out top to bottom.
    pub fn column(&mut self, f: impl FnOnce(&mut Ui)) {
        let id = self.auto_id();
        let gap = self.theme.gap;
        let (_, size) = self.group(id, Dir::Column, Vec2::ZERO, gap, None, f);
        self.advance(size);
    }

    /// Lays children out in a grid of `columns` equal-width cells, row by
    /// row; each row is as tall as its tallest cell. Inside a cell,
    /// [`available_width`](Self::available_width) is the cell's width.
    pub fn grid(&mut self, columns: usize, f: impl FnOnce(&mut Ui)) {
        let id = self.auto_id();
        let (gap, cols) = (self.theme.gap, columns.clamp(1, u16::MAX as usize));
        let cell = ((self.available_width() - gap * (cols - 1) as f32) / cols as f32).max(0.0);
        self.next_width = Some(cell);
        let (_, size) = self.group(id, Dir::Grid(cols as u16), Vec2::ZERO, gap, None, f);
        self.advance(size);
    }

    /// Splits the available width into columns sized by `weights` (`[1.0,
    /// 2.0]` makes the second twice as wide) and lays out column `i` with
    /// `f(ui, i)`. Columns are top-aligned.
    pub fn columns(&mut self, weights: &[f32], mut f: impl FnMut(&mut Ui, usize)) {
        let id = self.auto_id();
        let gap = self.theme.gap;
        let avail = self.available_width();
        let total: f32 = weights.iter().sum::<f32>().max(1e-3);
        let free = (avail - gap * weights.len().saturating_sub(1) as f32).max(0.0);
        let prev = self.sizes.get(&id).copied().unwrap_or(Vec2::ZERO);
        let origin = self.peek(prev);
        let (mut x, mut height) = (origin.x, 0.0f32);
        for (i, w) in weights.iter().enumerate() {
            let width = free * w / total;
            self.next_width = Some(width);
            let (_, size) = self.group(id.with(i as u64), Dir::Column, Vec2::ZERO, gap, Some(Vec2::new(x, origin.y)), |ui| f(ui, i));
            height = height.max(size.y);
            x += width + gap;
        }
        let size = Vec2::new(avail, height);
        self.sizes.insert(id, size);
        self.advance(size);
    }

    /// Places `f`'s content at the start, center or end of the available
    /// width (it takes a frame to know the content's width).
    pub fn align(&mut self, align: Align, f: impl FnOnce(&mut Ui)) {
        let id = self.auto_id();
        let avail = self.available_width();
        let prev = self.sizes.get(&id).map_or(0.0, |s| s.x);
        let factor = match align {
            Align::Start => 0.0,
            Align::Center => 0.5,
            Align::End => 1.0,
        };
        let origin = self.peek(Vec2::ZERO) + Vec2::new(((avail - prev) * factor).max(0.0), 0.0);
        self.next_width = Some(avail);
        let gap = self.theme.gap;
        let (_, size) = self.group(id, Dir::Column, Vec2::ZERO, gap, Some(origin), f);
        self.advance(Vec2::new(avail, size.y));
    }

    /// Flexible space in a row: pushes everything declared after it to the
    /// end of the row (a toolbar's right side, a dialog's buttons). It
    /// learns how wide that tail is from the previous frame.
    pub fn fill(&mut self) {
        let l = self.layouts.last().unwrap();
        let tail = self.sizes.get(&l.id.child("morfo-fill")).map_or(0.0, |s| s.x);
        let l = self.layouts.last_mut().unwrap();
        let room = (l.width - l.cursor - tail).max(0.0);
        l.cursor += room;
        l.fill_from = Some(l.cursor);
    }

    /// Empty space along the current direction.
    pub fn space(&mut self, amount: f32) {
        let l = self.layouts.last_mut().unwrap();
        l.cursor += amount;
    }

    /// Width left for children in the current container.
    pub fn available_width(&self) -> f32 {
        self.layouts.last().unwrap().width
    }

    /// Content that fluidly opens and closes, pushing its siblings along.
    /// While closed it costs nothing: `f` is not even called.
    /// Keys inside `f` are namespaced under `key`.
    pub fn reveal(&mut self, key: &str, open: bool, f: impl FnOnce(&mut Ui)) {
        let id = self.declare(key);
        self.reveal_id(id, open, |ui| ui.within(id, f));
    }

    /// Spring-driven visibility, `0..1`, for content of your own that should
    /// animate out as well as in: keep declaring it while the result is
    /// above zero, scaled or faded by it.
    pub fn presence(&mut self, key: &str, shown: bool) -> f32 {
        let id = self.declare(key);
        self.presence_id(id, shown, SpringKind::Smooth)
    }

    pub(crate) fn presence_id(&mut self, id: Id, shown: bool, kind: SpringKind) -> f32 {
        let k = self.steps[kind as usize];
        let n = self.nodes.entry(id).or_insert_with(Node::new);
        if n.touch(self.frame) {
            n.aux.snap(0.0);
        }
        n.aux.set(if shown { 1.0 } else { 0.0 });
        self.animating |= n.aux.tick(&k);
        n.aux.get()
    }

    /// Runs `f` laid out from `origin` in paint layer `layer`, outside the
    /// normal flow: no clip, no inherited opacity.
    pub(crate) fn detached(&mut self, layer: usize, id: Id, origin: Vec2, width: f32, f: impl FnOnce(&mut Ui)) {
        let depth = self.layouts.len();
        let (alpha, clip, cull, shift) = (self.alpha, self.clip, self.draw.cull.take(), std::mem::take(&mut self.shift));
        let below = self.draw.set_layer(layer);
        self.alpha = 1.0;
        self.clip = None;
        self.layouts.push(Layout {
            id,
            dir: Dir::Column,
            origin,
            cursor: 0.0,
            gap: self.theme.gap,
            cross: 0.0,
            prev_cross: 0.0,
            count: 0,
            width,
            fill_from: None,
        });
        f(self);
        self.layouts.truncate(depth);
        self.draw.set_layer(below);
        self.draw.cull = cull;
        self.alpha = alpha;
        self.clip = clip;
        self.shift = shift;
    }

    pub(crate) fn reveal_id(&mut self, id: Id, open: bool, f: impl FnOnce(&mut Ui)) {
        let k = self.steps[SpringKind::Gentle as usize];
        let n = self.nodes.entry(id).or_insert_with(Node::new);
        if n.touch(self.frame) {
            n.aux.snap(if open { 1.0 } else { 0.0 });
        }
        n.aux.set(if open { 1.0 } else { 0.0 });
        self.animating |= n.aux.tick(&k);
        let p = n.aux.get().clamp(0.0, 1.0);
        if p <= 0.001 && !open {
            return;
        }
        let prev = self.sizes.get(&id).copied().unwrap_or(Vec2::ZERO);
        let origin = self.peek(Vec2::new(prev.x, prev.y * p));
        // Leave room around the content for its shadows, opening with it.
        let visible = Rect::new(origin.x - 40.0, origin.y - 16.0 * p, self.available_width() + 80.0, (prev.y + 64.0) * p);
        let (saved_alpha, saved_clip) = (self.alpha, self.clip);
        self.alpha *= smoothstep(p);
        self.clip = Some(saved_clip.map_or(visible, |c| c.intersect(&visible)));
        self.draw.push(DrawCmd::PushClip { rect: visible, radius: 0.0 });
        let gap = self.theme.gap;
        let (_, size) = self.group(id, Dir::Column, Vec2::ZERO, gap, Some(origin), f);
        self.draw.push(DrawCmd::PopClip);
        self.alpha = saved_alpha;
        self.clip = saved_clip;
        if size.y > 0.0 {
            // Fold the gap into the animated height so siblings don't jump.
            let l = self.layouts.last().unwrap();
            let full = size.y + l.gap;
            self.advance(Vec2::new(size.x, full * p - l.gap));
        }
    }

    pub(crate) fn auto_id(&self) -> Id {
        let l = self.layouts.last().unwrap();
        l.id.with(l.count)
    }

    /// Where an item of `size` would be placed next.
    pub(crate) fn peek(&self, size: Vec2) -> Vec2 {
        let l = self.layouts.last().unwrap();
        match l.dir {
            Dir::Column => Vec2::new(l.origin.x, l.origin.y + l.cursor),
            Dir::Row => Vec2::new(l.origin.x + l.cursor, l.origin.y + ((l.prev_cross - size.y) * 0.5).max(0.0)),
            Dir::RowTop => Vec2::new(l.origin.x + l.cursor, l.origin.y),
            Dir::Grid(cols) => {
                let col = (l.count % cols as u64) as f32;
                Vec2::new(l.origin.x + col * (l.width + l.gap), l.origin.y + l.cursor)
            }
        }
    }

    pub(crate) fn advance(&mut self, size: Vec2) {
        let l = self.layouts.last_mut().unwrap();
        if let Dir::Grid(cols) = l.dir {
            // `cross` is the height of the row being filled.
            l.cross = l.cross.max(size.y);
            l.count += 1;
            if l.count.is_multiple_of(cols as u64) {
                l.cursor += l.cross + l.gap;
                l.cross = 0.0;
            }
            return;
        }
        let (main, cross) = match l.dir {
            Dir::Row | Dir::RowTop => (size.x, size.y),
            _ => (size.y, size.x),
        };
        l.cursor += main + l.gap;
        l.cross = l.cross.max(cross);
        l.count += 1;
    }

    /// Reserves space for an item; returns its *target* rectangle.
    pub(crate) fn alloc(&mut self, size: Vec2) -> Rect {
        let pos = self.peek(size);
        self.advance(size);
        Rect::from_min_size(pos, size)
    }

    /// Runs `f` inside a nested layout and returns its origin and content
    /// size. The caller decides how much space to consume in the parent.
    pub(crate) fn group(&mut self, id: Id, dir: Dir, inset: Vec2, gap: f32, origin: Option<Vec2>, f: impl FnOnce(&mut Ui)) -> (Vec2, Vec2) {
        let prev = self.sizes.get(&id).copied().unwrap_or(Vec2::ZERO);
        let origin = origin.unwrap_or_else(|| self.peek(prev));
        let parent = self.layouts.last().unwrap();
        // In a grid cell the room is the cell's width, wherever it starts.
        let room = if matches!(parent.dir, Dir::Grid(_)) { parent.width } else { parent.width - (origin.x - parent.origin.x) };
        let width = self.next_width.take().unwrap_or(room - 2.0 * inset.x).max(0.0);
        self.layouts.push(Layout {
            id,
            dir,
            origin: origin + inset,
            cursor: 0.0,
            gap,
            cross: 0.0,
            prev_cross: match dir {
                Dir::Row | Dir::RowTop => prev.y - 2.0 * inset.y,
                _ => prev.x - 2.0 * inset.x,
            },
            count: 0,
            width,
            fill_from: None,
        });
        f(self);
        let l = self.layouts.pop().unwrap();
        let main = (l.cursor - l.gap).max(0.0);
        if let Some(from) = l.fill_from {
            // Width of what follows the fill, for next frame's fill.
            self.sizes.insert(id.child("morfo-fill"), Vec2::new((main - from).max(0.0), 0.0));
        }
        let size = match dir {
            Dir::Row | Dir::RowTop => Vec2::new(main, l.cross),
            Dir::Column => Vec2::new(l.cross, main),
            Dir::Grid(cols) => {
                let open_row = if !l.count.is_multiple_of(cols as u64) { l.cross + l.gap } else { 0.0 };
                let used_cols = (l.count.min(cols as u64)) as f32;
                Vec2::new(used_cols * l.width + (used_cols - 1.0).max(0.0) * l.gap, (l.cursor + open_row - l.gap).max(0.0))
            }
        } + inset * 2.0;
        self.sizes.insert(id, size);
        (origin, size)
    }
}
