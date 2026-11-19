//! Transitions between views: when what a region shows changes (another
//! page, another step, another photo), the view that leaves and the one
//! that arrives move together in one of several choreographies, and the
//! elements both views share ([`Ui::hero`]) fly from where they were to
//! where they are now.

use crate::anim::smoothstep;
use crate::anim::SpringKind::Smooth;
use crate::draw::DrawCmd;
use crate::geom::{Color, Rect, Vec2};
use crate::id::Id;
use crate::morph::Snapshot;
use crate::ui::{Dir, Node, Ui};

/// How one view gives way to the next in [`Ui::transition`]. The ones that
/// move sideways follow the direction of travel: forward (a higher view
/// number) enters from the right, back from the left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Transition {
    /// The old view fades out as the new one fades in.
    Fade,
    /// Both views travel a full width, like the pages of a carousel.
    Slide,
    /// A short shift along the direction of travel with a cross-fade: the
    /// calm default for moving between pages.
    #[default]
    Shift,
    /// The new view slides in over the old one, which steps back and
    /// darkens; going back, the top one slides away to uncover the other.
    Push,
    /// The old view grows and fades as the new one grows into place (the
    /// other way round going back): going deeper, or out.
    Zoom,
    /// A card turning over: the old view folds to its edge and the new one
    /// unfolds from it.
    Flip,
    /// The old view rises and fades, the new one rises from below.
    Rise,
}

impl Transition {
    pub const ALL: [Transition; 7] =
        [Transition::Fade, Transition::Slide, Transition::Shift, Transition::Push, Transition::Zoom, Transition::Flip, Transition::Rise];
}

/// What a transition shows, and what it is leaving.
#[derive(Default)]
pub(crate) struct Swap {
    view: usize,
    /// `1` going forward, `-1` going back.
    dir: f32,
    now: Snapshot,
    was: Snapshot,
    /// Where the shared elements that flew away were in `was`.
    holes: Vec<Rect>,
}

/// Where and how a view is drawn at one moment of a transition.
#[derive(Clone, Copy)]
struct Pose {
    offset: Vec2,
    scale: Vec2,
    fade: f32,
}

impl Pose {
    const REST: Pose = Pose { offset: Vec2::ZERO, scale: Vec2::new(1.0, 1.0), fade: 1.0 };
    fn shift(dx: f32, dy: f32, fade: f32) -> Pose {
        Pose { offset: Vec2::new(dx, dy), fade, ..Pose::REST }
    }
    fn zoom(s: f32, fade: f32) -> Pose {
        Pose { scale: Vec2::new(s, s), fade, ..Pose::REST }
    }
    /// `r` moved and scaled about its center.
    fn apply(&self, r: Rect) -> Rect {
        let (w, h) = (r.w * self.scale.x, r.h * self.scale.y);
        Rect::new(r.x + (r.w - w) * 0.5 + self.offset.x, r.y + (r.h - h) * 0.5 + self.offset.y, w, h)
    }
}

/// How the old view and the new one are posed at progress `t`, whether the
/// new one is drawn under the old one, and how dark a veil covers the one
/// underneath.
fn choreography(style: Transition, t: f32, dir: f32, width: f32) -> (Pose, Pose, bool, f32) {
    let q = t.clamp(0.0, 1.0);
    let out = |span: f32| 1.0 - smoothstep(q / span);
    let into = |from: f32| smoothstep((q - from) / (1.0 - from));
    match style {
        Transition::Fade => (Pose::shift(0.0, 0.0, out(0.7)), Pose::shift(0.0, 0.0, into(0.15)), false, 0.0),
        Transition::Slide => {
            let w = width + 24.0;
            (Pose::shift(-dir * w * t, 0.0, 1.0), Pose::shift(dir * w * (1.0 - t), 0.0, 1.0), false, 0.0)
        }
        Transition::Shift => (Pose::shift(-dir * 36.0 * t, 0.0, out(0.5)), Pose::shift(dir * 36.0 * (1.0 - t), 0.0, into(0.3)), false, 0.0),
        Transition::Push if dir > 0.0 => {
            // The new view arrives on top; the old one steps back under it.
            (Pose::shift(-width * 0.3 * t, 0.0, 1.0), Pose::shift(width * (1.0 - t), 0.0, 1.0), false, 0.22 * q)
        }
        Transition::Push => {
            // Going back: the old view slides away and uncovers the new one.
            (Pose::shift(width * t, 0.0, 1.0), Pose::shift(-width * 0.3 * (1.0 - t), 0.0, 1.0), true, 0.22 * (1.0 - q))
        }
        Transition::Zoom => {
            let d = if dir > 0.0 { 1.0 } else { -1.0 };
            (Pose::zoom(1.0 + 0.08 * d * q, out(0.6)), Pose::zoom(1.0 - 0.08 * d * (1.0 - q), into(0.2)), false, 0.0)
        }
        Transition::Flip => {
            // The first half folds the old view, the second unfolds the new.
            let fold = |s: f32| Pose { scale: Vec2::new(s.max(0.0), 1.0 - 0.06 * (1.0 - s)), fade: smoothstep(s * 2.5), ..Pose::REST };
            let (old, new) = if q < 0.5 {
                (fold(1.0 - 2.0 * q), Pose { fade: 0.0, ..Pose::REST })
            } else {
                (Pose { fade: 0.0, ..Pose::REST }, fold(2.0 * q - 1.0))
            };
            (old, new, false, 0.0)
        }
        Transition::Rise => (Pose::shift(0.0, -dir * 24.0 * t, out(0.5)), Pose::shift(0.0, dir * 32.0 * (1.0 - t), into(0.25)), false, 0.0),
    }
}

impl Ui {
    /// A region that shows one of several views: `view` says which, and `f`
    /// builds it. When `view` changes, the view that was there leaves and
    /// the new one arrives in the style `style`, in the direction of travel
    /// (a higher number is forward). The old view is only a picture of how
    /// it last looked, so it cannot be clicked as it goes; the new one works
    /// from the first frame. Keys inside `f` are namespaced under `key`, and
    /// a key declared in two views is the same element: it travels from
    /// where it was to where it is.
    ///
    /// ```
    /// # let mut ui = morfo::Ui::new(morfo::Theme::dark());
    /// # let mut step = 0usize;
    /// # ui.begin(0.0, morfo::Vec2::new(800.0, 600.0));
    /// ui.transition("pasos", step, morfo::Transition::Push, |ui| match step {
    ///     0 => ui.label("Bienvenida"),
    ///     _ => ui.label("Tu perfil"),
    /// });
    /// # ui.end();
    /// ```
    pub fn transition(&mut self, key: &str, view: usize, style: Transition, f: impl FnOnce(&mut Ui)) {
        let id = self.declare(key);
        // With reduced motion every choreography is a cross-fade.
        let style = if self.reduced_motion() { Transition::Fade } else { style };
        let mut st = self.swaps.remove(&id).unwrap_or_default();
        let n = self.nodes.entry(id).or_insert_with(Node::new);
        let absent = n.touch(self.frame);
        let changed = !absent && st.view != view;
        if changed {
            st.dir = if view > st.view { 1.0 } else { -1.0 };
            std::mem::swap(&mut st.was, &mut st.now);
            n.aux.snap(0.0);
            n.aux.set(1.0);
        } else if absent {
            n.aux.snap(1.0);
        }
        st.view = view;

        // Shared elements declared while the view changes may take off;
        // while it changes, the transition is the new view's entrance.
        let in_motion = changed || !n.aux.is_resting();
        let entrance = self.skip_entrance;
        self.skip_entrance |= in_motion;
        let outer = std::mem::replace(&mut self.collect_heroes, changed);
        if changed {
            self.hero_launches.clear();
        }
        let mark = self.draw.mark();
        let gap = self.theme.gap;
        let (origin, size) = self.group(id, Dir::Column, Vec2::ZERO, gap, None, |ui| ui.within(id, f));
        self.collect_heroes = outer;
        self.skip_entrance = entrance;
        if changed {
            st.holes = std::mem::take(&mut self.hero_launches);
        }
        self.advance(size);
        let target = Rect::from_min_size(origin, size);
        let smooth = self.steps[Smooth as usize];
        let n = self.nodes.get_mut(&id).unwrap();
        let moving = n.aux.tick(&smooth);
        let t = n.aux.get();
        self.animating |= moving;
        st.now.capture(self.draw.cmds_from(mark), &self.draw, target);
        if !moving {
            self.swaps.insert(id, st);
            return;
        }

        let mut new = std::mem::take(&mut self.morph_buf);
        new.clear();
        self.draw.drain_from(mark, &mut new);
        let old = st.was.bounds;
        let width = target.w.max(old.w).max(1.0);
        let (pose_old, pose_new, new_below, veil) = choreography(style, t, st.dir, width);
        // Views that travel sideways are cut at the edges of the region
        // (with room for shadows), not at the window's.
        let clipped = matches!(style, Transition::Slide | Transition::Push);
        if clipped {
            let tall = target.h.max(old.h);
            self.draw.push(DrawCmd::PushClip { rect: Rect::new(target.x - 12.0, target.y - 16.0, width + 24.0, tall + 32.0), radius: 0.0 });
        }
        let a = self.alpha;
        let shade = |ui: &mut Ui, r: Rect| {
            if veil > 0.002 {
                ui.draw.rect(r.inset(-12.0), 0.0, Color::BLACK.with_alpha(veil * a));
            }
        };
        let old_to = pose_old.apply(old);
        let new_to = pose_new.apply(target);
        let holes = std::mem::take(&mut st.holes);
        // A folding card keeps what it holds inside its own edges.
        let fold = style == Transition::Flip;
        let open = |ui: &mut Ui, r: Rect| {
            if fold {
                ui.draw.push(DrawCmd::PushClip { rect: r.inset(-14.0), radius: 0.0 });
            }
        };
        let close = |ui: &mut Ui| {
            if fold {
                ui.draw.push(DrawCmd::PopClip);
            }
        };
        if new_below {
            self.emit_mapped(&new, None, target, new_to, pose_new.fade);
            shade(self, new_to);
            self.replay(&st.was, old_to, pose_old.fade, None, &holes);
        } else {
            open(self, old_to);
            self.replay(&st.was, old_to, pose_old.fade, None, &holes);
            close(self);
            shade(self, old_to);
            open(self, new_to);
            self.emit_mapped(&new, None, target, new_to, pose_new.fade);
            close(self);
        }
        if clipped {
            self.draw.push(DrawCmd::PopClip);
        }
        st.holes = holes;
        self.morph_buf = new;
        self.swaps.insert(id, st);
    }

    /// An element that two views share: a photo in a gallery and the same
    /// photo, large, in its detail view. Declare it with the same `key` in
    /// both; when a [`transition`](Self::transition) goes from one view to
    /// the other, it flies from where it was to where it is now, growing or
    /// shrinking on the way, above both views, while its old copy is left
    /// out of the view that leaves. Outside a transition it does the same
    /// when it moves far in one go. `key` is global: it does not depend on
    /// the scope it is declared in.
    pub fn hero(&mut self, key: &str, f: impl FnOnce(&mut Ui)) {
        self.busy = true;
        let id = Id::new("morfo-hero").child(key);
        let prev = self.sizes.get(&id).copied().unwrap_or(Vec2::ZERO);
        let at = self.peek(prev);
        let mark = self.draw.mark();
        // In flight it may cross the edge of a scroll area: nothing of it
        // is culled.
        let cull = self.draw.cull.take();
        // Content that arrives with it is already travelling: no entrance.
        let continues = self.nodes.get(&id).is_some_and(|n| n.present(self.frame));
        let entrance = self.skip_entrance;
        self.skip_entrance |= continues;
        let gap = self.theme.gap;
        let (origin, size) = self.group(id, Dir::Column, Vec2::ZERO, gap, Some(at), f);
        self.skip_entrance = entrance;
        self.draw.cull = cull;
        self.advance(size);
        let target = Rect::from_min_size(origin, size);
        let (smooth, frame) = (self.steps[Smooth as usize], self.frame);
        // With reduced motion nothing flies: the new place simply shows it.
        let (collect, in_scroll) = (self.collect_heroes && !self.reduced_motion(), !self.scrolls.is_empty() || self.reduced_motion());
        let shift = self.shift;
        let n = self.nodes.entry(id).or_insert_with(Node::new);
        if n.latch {
            n.reframe(frame, shift);
        }
        let absent = n.touch(frame);
        let last = n.rect.target();
        let far = (last.x - target.x).abs().max((last.y - target.y).abs()).max((last.w - target.w).abs()).max((last.h - target.h).abs());
        // Scrolling moves things far too, but that is not a journey.
        let launch = !absent && far > 2.0 && (collect || (!in_scroll && far > 48.0));
        if absent {
            n.rect.snap(target);
            n.latch = false;
        } else if launch {
            n.latch = true;
            let from = n.rect.get();
            n.rect.set(target);
            self.hero_launches.push(from);
        } else if n.latch {
            n.rect.set(target);
        } else {
            n.rect.snap(target);
        }
        let n = self.nodes.get_mut(&id).unwrap();
        if n.latch && !n.rect.tick(&smooth) {
            n.latch = false;
        }
        if !n.latch {
            return;
        }
        let r = n.rect.get();
        self.animating = true;
        // In flight: what it drew at its place is drawn instead where it is
        // now, above everything else.
        let mut cmds = Vec::new();
        self.draw.drain_from(mark, &mut cmds);
        let below = self.draw.set_layer(1);
        let cull = self.draw.cull.take();
        self.emit_mapped(&cmds, None, target, r, 1.0);
        self.draw.cull = cull;
        self.draw.set_layer(below);
    }
}
