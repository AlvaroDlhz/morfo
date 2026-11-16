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
    Column,
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
}

impl Ui {
    /// Lays children out left to right, vertically centered.
    pub fn row(&mut self, f: impl FnOnce(&mut Ui)) {
        let id = self.auto_id();
        let gap = self.theme.gap;
        let (_, size) = self.group(id, Dir::Row, Vec2::ZERO, gap, None, f);
        self.advance(size);
    }

    /// Lays children out top to bottom.
    pub fn column(&mut self, f: impl FnOnce(&mut Ui)) {
        let id = self.auto_id();
        let gap = self.theme.gap;
        let (_, size) = self.group(id, Dir::Column, Vec2::ZERO, gap, None, f);
        self.advance(size);
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
    pub fn reveal(&mut self, key: &str, open: bool, f: impl FnOnce(&mut Ui)) {
        let id = self.id(key);
        self.reveal_id(id, open, f);
    }

    /// Spring-driven visibility, `0..1`, for content of your own that should
    /// animate out as well as in: keep declaring it while the result is
    /// above zero, scaled or faded by it.
    pub fn presence(&mut self, key: &str, shown: bool) -> f32 {
        let id = self.id(key);
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
        let (alpha, clip, cull) = (self.alpha, self.clip, self.draw.cull.take());
        let below = self.draw.set_layer(layer);
        self.alpha = 1.0;
        self.clip = None;
        self.layouts.push(Layout { id, dir: Dir::Column, origin, cursor: 0.0, gap: self.theme.gap, cross: 0.0, prev_cross: 0.0, count: 0, width });
        f(self);
        self.layouts.truncate(depth);
        self.draw.set_layer(below);
        self.draw.cull = cull;
        self.alpha = alpha;
        self.clip = clip;
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
        }
    }

    pub(crate) fn advance(&mut self, size: Vec2) {
        let l = self.layouts.last_mut().unwrap();
        let (main, cross) = match l.dir {
            Dir::Column => (size.y, size.x),
            Dir::Row => (size.x, size.y),
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
        let width = self.next_width.take().unwrap_or(parent.width - (origin.x - parent.origin.x) - 2.0 * inset.x).max(0.0);
        self.layouts.push(Layout {
            id,
            dir,
            origin: origin + inset,
            cursor: 0.0,
            gap,
            cross: 0.0,
            prev_cross: match dir {
                Dir::Row => prev.y - 2.0 * inset.y,
                Dir::Column => prev.x - 2.0 * inset.x,
            },
            count: 0,
            width,
        });
        f(self);
        let l = self.layouts.pop().unwrap();
        let main = (l.cursor - l.gap).max(0.0);
        let size = match dir {
            Dir::Row => Vec2::new(main, l.cross),
            Dir::Column => Vec2::new(l.cross, main),
        } + inset * 2.0;
        self.sizes.insert(id, size);
        (origin, size)
    }
}
