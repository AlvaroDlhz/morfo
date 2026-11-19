//! Motion driven by scrolling: content that enters as it comes into view
//! (one element after another when several arrive together), numbers that
//! count up when they are first seen, parallax, sections whose header stays
//! pinned until the next one pushes it away, values linked to how far the
//! content has scrolled, and smooth travel to an anchor.

use crate::a11y::Role;
use crate::anim::smoothstep;
use crate::anim::SpringKind::Smooth;
use crate::anim::{Spring, SpringStep};
use crate::draw::DrawCmd;
use crate::geom::{Rect, Vec2};
use crate::id::Id;
use crate::theme::{BORDER, SHADOW, TEXT};
use crate::ui::{Dir, Node, Ui, LINE_HEIGHT};

/// A scroll area while its content is laid out.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrollCtx {
    pub id: Id,
    /// What is visible of it, on screen.
    pub view: Rect,
    /// How far it has scrolled, and how far it can.
    pub offset: f32,
    pub max: f32,
    /// Where the top of its content is drawn this frame.
    pub top: f32,
}

/// How content enters when it scrolls into view ([`Ui::scroll_reveal`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Reveal {
    /// It fades in where it is.
    Fade,
    /// It rises a little as it fades in.
    #[default]
    Rise,
    /// It slides in from the left.
    Left,
    /// It slides in from the right.
    Right,
    /// It grows from slightly smaller.
    Grow,
    /// It is uncovered from left to right.
    Wipe,
}

impl Reveal {
    pub const ALL: [Reveal; 6] = [Reveal::Fade, Reveal::Rise, Reveal::Left, Reveal::Right, Reveal::Grow, Reveal::Wipe];
}

/// Delay between elements that come into view together.
const STAGGER: f64 = 0.07;
/// Longest wait, however many arrive at once.
const MAX_DELAY: f64 = 0.6;
/// How fast a number counts up: a long, unhurried spring.
const COUNT: Spring = Spring::new(1.2, 1.0);

impl Ui {
    /// The innermost scroll area being laid out, if any.
    fn scroll_ctx(&self) -> Option<ScrollCtx> {
        self.scrolls.last().copied()
    }

    /// The area elements are checked against to know whether they are seen:
    /// the innermost scroll area's view, or the whole window.
    pub(crate) fn view_rect(&self) -> Rect {
        self.scroll_ctx().map_or(Rect::from_min_size(Vec2::ZERO, self.viewport()), |c| c.view)
    }

    /// Progress, `0..1` on a spring, of something at `rect` that appears
    /// the first time it is in view and then stays. Things that come into
    /// view in the same frame start one after another.
    pub(crate) fn appear(&mut self, id: Id, rect: Rect) -> f32 {
        let view = self.view_rect();
        let (now, frame, instant, reduced) = (self.now, self.frame, self.still, self.reduced_motion());
        let k = self.steps[Smooth as usize];
        let (batch, waking) = (self.reveal_batch, self.waking);
        let n = self.nodes.entry(id).or_insert_with(Node::new);
        if n.touch(frame) {
            if waking {
                // Back from being left unbuilt: it had appeared already.
                n.index = 3;
                n.latch = true;
                n.stamp = 0.0;
                n.aux.snap(1.0);
            } else {
                n.index = 0;
                n.latch = false;
                n.aux.snap(0.0);
            }
        }
        n.index = n.index.saturating_add(1);
        // The layout needs a couple of frames to know where things are.
        let settled = n.index > 2 && rect.h > 0.0;
        let margin = (rect.h * 0.2).min(32.0);
        let visible = rect.y < view.bottom() - margin && rect.bottom() > view.y + margin;
        let mut arrived = false;
        if settled && visible && !n.latch {
            n.latch = true;
            // With reduced motion they do not wait for one another.
            n.stamp = if reduced { now } else { now + (batch as f64 * STAGGER).min(MAX_DELAY) };
            arrived = true;
        }
        let mut wake = None;
        if n.latch {
            if instant || now >= n.stamp {
                n.aux.set(1.0);
            } else {
                wake = Some(n.stamp);
            }
        }
        let anim = n.aux.tick(&k);
        let p = n.aux.get();
        self.animating |= anim;
        if arrived {
            self.reveal_batch += 1;
        }
        if let Some(at) = wake {
            self.wake_at(at);
        }
        p
    }

    /// Content that enters, in the style `style`, the first time it scrolls
    /// into view (or right away, if it is already in view when it appears).
    /// When several come into view at once they enter one after another, so
    /// wrapping each card of a grid in its own `scroll_reveal` cascades
    /// them. Inside a [`scroll`](Self::scroll) area it is that area's view
    /// that counts; elsewhere, the window. The layout never moves: only the
    /// drawing does, so nothing around it jumps.
    pub fn scroll_reveal(&mut self, key: &str, style: Reveal, f: impl FnOnce(&mut Ui)) {
        let id = self.declare(key);
        // With reduced motion every entrance is a fade, where it is.
        let style = if self.reduced_motion() { Reveal::Fade } else { style };
        let prev = self.sizes.get(&id).copied().unwrap_or(Vec2::ZERO);
        let at = self.peek(prev);
        let p = self.appear(id.child("morfo-aparece"), Rect::from_min_size(at, prev));
        let slot = (style == Reveal::Wipe).then(|| self.draw.reserve());
        let mark = self.draw.mark();
        let gap = self.theme.gap;
        let (origin, size) = self.group(id, Dir::Column, Vec2::ZERO, gap, None, |ui| ui.within(id, f));
        self.advance(size);
        if (p - 1.0).abs() < 1e-3 {
            return;
        }
        let bounds = Rect::from_min_size(origin, size);
        let (q, rest) = (p.clamp(0.0, 1.0), 1.0 - p);
        match style {
            Reveal::Fade => self.transform_from(mark, bounds, bounds, smoothstep(q)),
            Reveal::Rise => self.transform_from(mark, bounds, bounds.translate(Vec2::new(0.0, 28.0 * rest)), smoothstep(q * 1.3)),
            Reveal::Left => self.transform_from(mark, bounds, bounds.translate(Vec2::new(-56.0 * rest, 0.0)), smoothstep(q * 1.3)),
            Reveal::Right => self.transform_from(mark, bounds, bounds.translate(Vec2::new(56.0 * rest, 0.0)), smoothstep(q * 1.3)),
            Reveal::Grow => self.transform_from(mark, bounds, bounds.scaled(0.86 + 0.14 * p), smoothstep(q * 1.3)),
            Reveal::Wipe => {
                // A clip that widens over the content, with a soft edge of
                // fading in.
                let clip = Rect::new(bounds.x - 8.0, bounds.y - 8.0, (bounds.w + 16.0) * q, bounds.h + 16.0);
                if let Some(slot) = slot {
                    self.draw.set(slot, DrawCmd::PushClip { rect: clip, radius: 0.0 });
                    self.transform_from(mark, bounds, bounds, smoothstep(q * 2.0));
                    self.draw.push(DrawCmd::PopClip);
                }
            }
        }
    }

    /// A number in text of size `size` that counts up from zero the first
    /// time it scrolls into view, and glides to any new `value` afterwards.
    /// It shows `decimals` decimals, written as the
    /// [`locale`](Self::set_locale) writes numbers. Its width is that of the
    /// final number, so what follows it never moves.
    pub fn count_up(&mut self, key: &str, value: f32, decimals: usize, size: f32) {
        let id = self.declare(key);
        let last = self.locale.number(value as f64, decimals);
        let w = self.text.width(&last, size) + 2.0;
        let target = self.alloc(Vec2::new(w, size * LINE_HEIGHT));
        let (r, life) = self.enter(id, target, Smooth);
        self.describe(id, Role::Label, key, r, Some(&last), None);
        let p = self.appear(id.child("morfo-aparece"), target);
        let k = SpringStep::new(COUNT, self.dt);
        let n = self.nodes.get_mut(&id).unwrap();
        if n.fresh() {
            // New: it counts from zero. Back from being unbuilt: it is there.
            n.e1.snap(if self.waking { value } else { 0.0 });
        }
        let goal = if p > 0.0 { value } else { 0.0 };
        // Counting is decoration: with reduced motion the number is there.
        if self.motion != crate::anim::MotionPreference::Full || self.instant {
            n.e1.snap(goal);
        }
        let mut anim = n.e1.drive(goal, false, &k);
        // Once it reads as the final number, it is there.
        if anim && (n.e1.get() - goal).abs() < 0.5 * 0.1f32.powi(decimals as i32) {
            n.e1.snap(goal);
            anim = false;
        }
        let shown = n.e1.get();
        self.animating |= anim;
        let text = if anim { self.locale.number(shown as f64, decimals) } else { last };
        let a = self.alpha * life * smoothstep(p.clamp(0.0, 1.0) * 3.0);
        self.draw.add_text(r.min(), size, self.pal[TEXT].fade(a), &text);
    }

    /// A window `height` tall whose content moves at another speed than the
    /// page as it scrolls: the depth of a landscape seen from a train.
    /// `f(ui, i)` builds layer `i`, from back to front; each is laid out as
    /// wide as the window and centered on it, so make the back ones taller
    /// than the window to keep them covering it. `depths[i]` is how much
    /// layer `i` holds back: `0` moves with the page, `0.5` at half its
    /// speed, `1` stays put on screen; negative values race ahead.
    pub fn parallax(&mut self, key: &str, height: f32, depths: &[f32], mut f: impl FnMut(&mut Ui, usize)) {
        let id = self.declare(key);
        let width = self.available_width();
        let target = self.alloc(Vec2::new(width, height));
        let (r, life) = self.enter(id, target, Smooth);
        // How far the window is from the middle of what is seen. With
        // reduced motion the layers move with the page: no depth.
        let rel = if self.reduced_motion() { 0.0 } else { target.center().y - self.view_rect().center().y };
        let radius = self.theme.radius;
        let (clip, alpha) = (self.clip, self.alpha);
        self.clip = Some(clip.map_or(r, |c| c.intersect(&r)));
        self.alpha *= life;
        self.draw.push(DrawCmd::PushClip { rect: r, radius });
        let (outer, frame) = (self.shift, self.frame);
        for (i, &depth) in depths.iter().enumerate() {
            let lid = id.with(i as u64);
            let prev = self.sizes.get(&lid).copied().unwrap_or(Vec2::ZERO);
            let lag = -rel * depth;
            let origin = Vec2::new(r.x, r.y + (height - prev.y) * 0.5 + lag);
            // Each layer is its own frame of reference: the page's, plus
            // what the layer holds back.
            let n = self.nodes.entry(lid).or_insert_with(Node::new);
            let held = if n.touch(frame) { 0.0 } else { lag - n.stamp as f32 };
            n.stamp = lag as f64;
            self.shift = outer + held;
            self.next_width = Some(width);
            self.group(lid, Dir::Column, Vec2::ZERO, 0.0, Some(origin), |ui| ui.within(id, |ui| f(ui, i)));
        }
        self.shift = outer;
        self.draw.push(DrawCmd::PopClip);
        self.clip = clip;
        self.alpha = alpha;
    }

    /// A section whose `header` stays pinned to the top of the scroll area
    /// while the section's `body` scrolls under it, until the end of the
    /// section pushes it away (the letters of a contact list). Once pinned it
    /// sits on a plate of the color behind it (the window's, or the card's
    /// it is in) with a soft shadow. Sections
    /// can nest: an inner header pins below the outer one. The section is
    /// also an anchor for [`scroll_to`](Self::scroll_to) under `key`.
    pub fn sticky_section(&mut self, key: &str, header: impl FnOnce(&mut Ui), body: impl FnOnce(&mut Ui)) {
        let id = self.declare(key);
        let (hid, bid) = (id.child("morfo-cabecera"), id.child("morfo-cuerpo"));
        let natural = self.peek(Vec2::ZERO);
        self.anchor_at(key, natural.y, 0.0);
        let (head, total) = (self.sizes.get(&hid).map_or(0.0, |s| s.y), self.sizes.get(&id).map_or(0.0, |s| s.y));
        let ctx = self.scroll_ctx();
        let pin = self.pin_inset;
        // Where the header goes: at its place, or held at the top of the
        // view, but never below the end of its own section.
        let y = match ctx {
            Some(c) => (c.view.y + pin).min(natural.y + total - head).max(natural.y),
            None => natural.y,
        };
        let shift = y - natural.y;
        // The header moves with the content, minus what pinning holds back
        // (kept from last frame in its node): while pinned it stays put.
        let frame = self.frame;
        let hn = self.nodes.entry(hid).or_insert_with(Node::new);
        let held = if hn.touch(frame) { 0.0 } else { shift - hn.stamp as f32 };
        hn.stamp = shift as f64;
        let (width, gap) = (self.available_width(), self.theme.gap);
        let mark = self.draw.mark();
        self.next_width = Some(width);
        let outer = self.shift;
        self.shift = outer + held;
        let (_, hsize) = self.group(hid, Dir::Column, Vec2::ZERO, gap, Some(Vec2::new(natural.x, y)), |ui| ui.within(id, header));
        self.shift = outer;
        let head_len = self.draw.mark() - mark;
        // While pinned, the plate catches the pointer for what is under it.
        let plate = ctx.map(|c| Rect::new(c.view.x, y - 8.0, c.view.w, hsize.y + 14.0));
        if let (true, Some(plate)) = (shift > 0.0, plate) {
            self.hit_inert(id.child("morfo-placa"), plate);
        }
        self.next_width = Some(width);
        self.pin_inset = pin + hsize.y + 6.0;
        let (_, bsize) =
            self.group(bid, Dir::Column, Vec2::ZERO, gap, Some(Vec2::new(natural.x, natural.y + hsize.y + gap)), |ui| ui.within(id, body));
        self.pin_inset = pin;
        let size = Vec2::new(width, hsize.y + gap + bsize.y);
        self.sizes.insert(id, size);
        self.advance(size);
        let Some(plate) = plate.filter(|_| shift > 0.0) else { return };
        // Pinned: the header is drawn again after the body, so it covers it.
        let mut cmds = std::mem::take(&mut self.morph_buf);
        cmds.clear();
        self.draw.drain_from(mark, &mut cmds);
        for &c in &cmds[head_len..] {
            self.draw.push(c);
        }
        let a = self.alpha;
        let lift = (shift / 16.0).min(1.0);
        self.draw.push(DrawCmd::Shadow {
            rect: plate,
            radius: 0.0,
            blur: 14.0,
            offset: Vec2::new(0.0, 4.0),
            color: self.pal[SHADOW].fade(0.7 * lift * a),
        });
        self.draw.rect(plate, 0.0, self.pal[self.ground].fade(a));
        self.draw.rect(Rect::new(plate.x, plate.bottom() - 1.0, plate.w, 1.0), 0.0, self.pal[BORDER].fade(lift * a));
        for &c in &cmds[..head_len] {
            self.draw.push(c);
        }
        self.morph_buf = cmds;
    }

    /// An invisible mark at this point of the content, for
    /// [`scroll_to`](Self::scroll_to).
    pub fn anchor(&mut self, key: &str) {
        self.declare(key);
        let y = self.peek(Vec2::ZERO).y;
        self.anchor_at(key, y, 12.0);
    }

    /// If a scroll to the anchor `key` was asked for, sends the scroll area
    /// there, with `margin` above it.
    fn anchor_at(&mut self, key: &str, y: f32, margin: f32) {
        if self.scroll_request != Some(Id::new(key)) {
            return;
        }
        let Some(c) = self.scroll_ctx() else { return };
        self.scroll_request = None;
        let to = (y - c.top - self.pin_inset - margin).clamp(0.0, c.max);
        self.scroll_restore.remove(&c.id);
        if let Some(n) = self.nodes.get_mut(&c.id) {
            n.aux.set(to);
        }
        self.animating = true;
    }

    /// Glides the scroll area that holds the [`anchor`](Self::anchor) (or
    /// [`sticky_section`](Self::sticky_section)) `key` until it is at the
    /// top. The anchor is found by its own key wherever it is nested, so a
    /// button outside the scroll area can call it; with several anchors of
    /// the same key, the first one laid out wins.
    pub fn scroll_to(&mut self, key: &str) {
        self.scroll_request = Some(Id::new(key));
        self.animating = true;
    }

    /// How far the scroll area being laid out has scrolled, `0..1` (`0`
    /// outside a scroll area, or when everything fits): a reading progress.
    pub fn scroll_progress(&self) -> f32 {
        self.scroll_ctx().map_or(0.0, |c| if c.max > 0.0 { (c.offset / c.max).clamp(0.0, 1.0) } else { 0.0 })
    }

    /// `0..1` as the scroll area being laid out scrolls from `from` to `to`
    /// pixels: `scroll_linked(0.0, 120.0)` is `0` at the top and `1` once
    /// 120 px have gone by. For things that change with the scroll
    /// position: a button that appears, a header that darkens.
    pub fn scroll_linked(&self, from: f32, to: f32) -> f32 {
        self.scroll_ctx().map_or(0.0, |c| ((c.offset - from) / (to - from).max(1e-3)).clamp(0.0, 1.0))
    }
}
