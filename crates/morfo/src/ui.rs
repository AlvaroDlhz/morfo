//! The immediate-mode front end with retained, spring-driven state.
//!
//! Each frame the application simply *declares* what should exist. Every
//! declared element owns a persistent [`Node`] keyed by its [`Id`]; the layout
//! only produces *targets* and the node's springs carry the real geometry and
//! colors there. So reordering, resizing, restyling or re-theming anything is
//! automatically a fluid morph, with no transition code anywhere.

use crate::adapt::{Profile, Prominence};
use crate::anim::{smoothstep, Motion, Spring, SpringKind, SpringStep};
use crate::draw::{DrawCmd, DrawList};
use crate::geom::{Color, Rect, Vec2};
use crate::id::{Id, IdMap};
use crate::theme::{Palette, Theme, BG};

/// Line box height as a multiple of the font size.
pub const LINE_HEIGHT: f32 = 1.3;

/// Text metrics provider; implemented by rendering backends.
pub trait TextMeasure {
    /// Width in logical pixels of `text` on one line at font size `size`.
    fn width(&self, text: &str, size: f32) -> f32;
}

/// Font-less estimate, good enough for tests and headless use.
pub struct ApproxText;

impl TextMeasure for ApproxText {
    fn width(&self, text: &str, size: f32) -> f32 {
        text.chars().count() as f32 * size * 0.54
    }
}

/// Result of [`Ui::end`].
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// `true` while something is still moving: request another frame. When
    /// `false` the interface is fully at rest and the host can sleep until the
    /// next input event (or until [`wake_in`](Self::wake_in)).
    pub animating: bool,
    /// Seconds until the interface needs a frame on its own, with no input:
    /// a tooltip about to appear, a toast about to leave, a blinking caret.
    pub wake_in: Option<f32>,
}

/// Keys the interface understands. Printable text goes through
/// [`Ui::text_input`] instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Enter,
    Escape,
    Tab,
}

/// What happened to an interactive element this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Response {
    pub clicked: bool,
    pub hovered: bool,
    pub held: bool,
    /// On-screen (animated) rectangle.
    pub rect: Rect,
}

#[derive(Default)]
pub(crate) struct Input {
    pub pointer: Option<Vec2>,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
    /// Pending wheel movement, in logical pixels (positive = content down).
    pub wheel: f32,
}

pub(crate) struct Hit {
    pub hovered: bool,
    pub held: bool,
    pub clicked: bool,
    /// The pointer was released after a press that started on this element.
    pub released: bool,
}

/// Persistent animated state of one element.
pub(crate) struct Node {
    pub rect: Motion<Rect>,
    pub fill: Motion<Color>,
    pub stroke: Motion<Color>,
    pub ink: Motion<Color>,
    pub radius: Motion<f32>,
    pub hover: Motion<f32>,
    pub press: Motion<f32>,
    /// Widget-specific value (toggle position, slider value, font size...).
    pub aux: Motion<f32>,
    /// 0 → 1 entrance.
    pub life: Motion<f32>,
    /// Widget-specific moment in time (hover start, last edit...).
    pub stamp: f64,
    seen: u64,
    age: u32,
}

impl Node {
    pub fn new() -> Node {
        Node {
            rect: Motion::new(Rect::default()),
            fill: Motion::new(Color::TRANSPARENT),
            stroke: Motion::new(Color::TRANSPARENT),
            ink: Motion::new(Color::TRANSPARENT),
            radius: Motion::new(0.0),
            hover: Motion::new(0.0),
            press: Motion::new(0.0),
            aux: Motion::new(0.0),
            life: Motion::new(0.0),
            stamp: 0.0,
            seen: 0,
            age: 0,
        }
    }

    /// Marks the node as declared this frame without placing it. Returns
    /// `true` if it is new or was absent last frame.
    pub fn touch(&mut self, frame: u64) -> bool {
        let absent = self.seen + 1 < frame;
        self.seen = frame;
        absent
    }

    /// Marks the node as declared this frame and moves its rect toward
    /// `target`. Returns whether it is still animating.
    pub fn begin(&mut self, frame: u64, target: Rect, k: &SpringStep, life: &SpringStep) -> bool {
        if self.seen + 1 < frame {
            // New, or back after being absent: (re)enter.
            self.age = 0;
            self.life.snap(0.0);
        }
        self.seen = frame;
        // Layout needs a couple of frames to know its own sizes; don't
        // animate from those provisional positions.
        if self.age < 2 {
            self.rect.snap(target);
        } else {
            self.rect.set(target);
        }
        self.age = self.age.saturating_add(1);
        self.life.set(1.0);
        self.rect.tick(k) | self.life.tick(life)
    }

    /// First frame of this node's life: style values should snap, not animate.
    pub fn fresh(&self) -> bool {
        self.age <= 1
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Dir {
    Row,
    Column,
}

pub(crate) struct Layout {
    pub id: Id,
    dir: Dir,
    pub origin: Vec2,
    pub cursor: f32,
    pub gap: f32,
    cross: f32,
    prev_cross: f32,
    count: u64,
    pub width: f32,
}

pub struct Ui {
    pub(crate) nodes: IdMap<Node>,
    pub(crate) sizes: IdMap<Vec2>,
    pub(crate) orders: IdMap<Vec<(u32, Prominence)>>,
    pub(crate) layouts: Vec<Layout>,
    pub(crate) scope: Id,
    pub(crate) alpha: f32,
    pub(crate) clip: Option<Rect>,
    pub(crate) steps: [SpringStep; 4],
    palette: Motion<Palette>,
    pub(crate) pal: Palette,
    pub(crate) theme: Theme,
    pub(crate) draw: DrawList,
    pub(crate) input: Input,
    pub(crate) active: Option<Id>,
    pub(crate) profile: Profile,
    pub(crate) text: Box<dyn TextMeasure>,
    pub(crate) now: f64,
    pub(crate) frame: u64,
    pub(crate) animating: bool,
    viewport: Vec2,
    /// Areas of upper layers that swallow the pointer: this frame's, and last
    /// frame's (the ones hit-testing can already rely on).
    pub(crate) blockers: Vec<(u8, Rect)>,
    blockers_prev: Vec<(u8, Rect)>,
    /// The menu currently open, if any.
    pub(crate) open: Option<Id>,
    pub(crate) menu_more: bool,
    /// Set by [`Ui::close`]: the enclosing floating panel should close.
    pub(crate) dismiss: bool,
    pub(crate) focus: Option<Id>,
    pub(crate) focus_claimed: bool,
    pub(crate) focusables: Vec<Id>,
    /// Caret position (byte offset) in the focused text field.
    pub(crate) cursor: usize,
    pub(crate) keys: Vec<Key>,
    pub(crate) typed: String,
    pub(crate) scratch: String,
    wake: f64,
    pub(crate) toasts: Vec<crate::overlay::Toast>,
    pub(crate) toast_seq: u64,
}

impl Ui {
    pub fn new(theme: Theme) -> Ui {
        Ui {
            nodes: IdMap::default(),
            sizes: IdMap::default(),
            orders: IdMap::default(),
            layouts: Vec::new(),
            scope: Id::ROOT,
            alpha: 1.0,
            clip: None,
            steps: [SpringStep::default(); 4],
            palette: Motion::new(theme.palette),
            pal: theme.palette,
            theme,
            draw: DrawList::default(),
            input: Input::default(),
            active: None,
            profile: Profile::new(),
            text: Box::new(ApproxText),
            now: 0.0,
            frame: 1,
            animating: false,
            viewport: Vec2::ZERO,
            blockers: Vec::new(),
            blockers_prev: Vec::new(),
            open: None,
            menu_more: false,
            dismiss: false,
            focus: None,
            focus_claimed: false,
            focusables: Vec::new(),
            cursor: 0,
            keys: Vec::new(),
            typed: String::new(),
            scratch: String::new(),
            wake: f64::INFINITY,
            toasts: Vec::new(),
            toast_seq: 0,
        }
    }

    // ----- configuration ---------------------------------------------------

    /// Installs the backend's text metrics.
    pub fn set_text_measure(&mut self, text: Box<dyn TextMeasure>) {
        self.text = text;
    }

    /// Switches theme. Colors morph; nothing jumps.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.palette.set(theme.palette);
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Current (animated) window background color.
    pub fn background(&self) -> Color {
        self.pal[BG]
    }

    /// The usage model that personalizes this interface.
    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn profile_mut(&mut self) -> &mut Profile {
        &mut self.profile
    }

    /// Restores a saved profile: the interface morphs to that user's shape.
    pub fn set_profile(&mut self, profile: Profile) {
        self.profile = profile;
        // Arrangements are re-derived from the new user's habits.
        self.orders.clear();
    }

    // ----- input -----------------------------------------------------------

    pub fn pointer_moved(&mut self, pos: Vec2) {
        self.input.pointer = Some(pos);
    }

    pub fn pointer_left(&mut self) {
        self.input.pointer = None;
    }

    pub fn pointer_pressed(&mut self) {
        self.input.down = true;
        self.input.pressed = true;
    }

    pub fn pointer_released(&mut self) {
        self.input.down = false;
        self.input.released = true;
    }

    /// Wheel or trackpad scroll, in logical pixels; positive moves the
    /// content down (towards its start).
    pub fn scroll_wheel(&mut self, dy: f32) {
        self.input.wheel += dy;
    }

    /// A non-printable key was pressed.
    pub fn key_pressed(&mut self, key: Key) {
        self.keys.push(key);
    }

    /// Printable text was typed (or pasted). Control characters are ignored.
    pub fn text_input(&mut self, text: &str) {
        self.typed.extend(text.chars().filter(|c| !c.is_control()));
    }

    /// Whether a text field has the keyboard focus.
    pub fn has_focus(&self) -> bool {
        self.focus.is_some()
    }

    /// Size of the area being laid out, in logical pixels.
    pub fn viewport(&self) -> Vec2 {
        self.viewport
    }

    /// Asks for a frame at time `at` even if there is no input by then.
    pub(crate) fn wake_at(&mut self, at: f64) {
        self.wake = self.wake.min(at);
    }

    // ----- frame -----------------------------------------------------------

    /// Starts a frame. `now` is any monotonically increasing time in seconds
    /// (use Unix time if the [`Profile`] is persisted between sessions).
    pub fn begin(&mut self, now: f64, viewport: Vec2) {
        // Springs are exact for any step, but after sleeping at rest a huge
        // step would skip the first visible frames of the next animation.
        let dt = ((now - self.now) as f32).clamp(0.0, 0.05);
        self.now = now;
        self.frame += 1;
        self.viewport = viewport;
        self.steps = [
            SpringStep::new(Spring::SNAPPY, dt),
            SpringStep::new(Spring::SMOOTH, dt),
            SpringStep::new(Spring::GENTLE, dt),
            SpringStep::new(Spring::BOUNCY, dt),
        ];
        self.animating = self.palette.tick(&self.steps[SpringKind::Gentle as usize]);
        self.pal = self.palette.get();
        self.draw.clear();
        std::mem::swap(&mut self.blockers, &mut self.blockers_prev);
        self.blockers.clear();
        self.focusables.clear();
        self.focus_claimed = false;
        self.dismiss = false;
        self.wake = f64::INFINITY;
        self.layouts.clear();
        self.scope = Id::ROOT;
        self.alpha = 1.0;
        self.clip = None;
        let pad = self.theme.pad * 1.5;
        self.layouts.push(Layout {
            id: Id::ROOT,
            dir: Dir::Column,
            origin: Vec2::splat(pad),
            cursor: 0.0,
            gap: self.theme.gap,
            cross: 0.0,
            prev_cross: 0.0,
            count: 0,
            width: (viewport.x - 2.0 * pad).max(0.0),
        });
    }

    /// Ends the frame. Paint [`draw_list`](Self::draw_list) afterwards.
    pub fn end(&mut self) -> Frame {
        self.draw_toasts();
        self.draw.flatten();
        // Keyboard focus: a press outside every field drops it, Tab cycles it.
        if self.input.pressed && !self.focus_claimed {
            self.focus = None;
        }
        if self.keys.contains(&Key::Tab) && !self.focusables.is_empty() {
            let at = self.focus.and_then(|f| self.focusables.iter().position(|&x| x == f));
            self.focus = Some(self.focusables[at.map_or(0, |i| (i + 1) % self.focusables.len())]);
            self.cursor = usize::MAX;
        }
        // State changed by input is only declared on the next frame.
        let again = self.input.pressed
            || self.input.released
            || self.input.wheel != 0.0
            || !self.keys.is_empty()
            || !self.typed.is_empty();
        self.input.pressed = false;
        self.input.released = false;
        self.input.wheel = 0.0;
        self.keys.clear();
        self.typed.clear();
        if !self.input.down {
            self.active = None;
        }
        if self.frame.is_multiple_of(256) {
            let frame = self.frame;
            self.nodes.retain(|_, n| n.seen + 256 >= frame);
        }
        let wake_in = self.wake.is_finite().then(|| (self.wake - self.now).max(0.0) as f32);
        Frame { animating: self.animating || again, wake_in }
    }

    pub fn draw_list(&self) -> &DrawList {
        &self.draw
    }

    /// Number of live elements (for diagnostics).
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    // ----- identity --------------------------------------------------------

    /// Id of `key` in the current scope.
    pub fn id(&self, key: &str) -> Id {
        self.scope.child(key)
    }

    /// Runs `f` with all keys namespaced under `key` (for repeated groups).
    pub fn scope(&mut self, key: &str, f: impl FnOnce(&mut Ui)) {
        let saved = self.scope;
        self.scope = saved.child(key);
        f(self);
        self.scope = saved;
    }

    /// On-screen rectangle of an element, if it exists.
    pub fn rect_of(&self, id: Id) -> Option<Rect> {
        self.nodes.get(&id).map(|n| n.rect.get())
    }

    // ----- adaptation ------------------------------------------------------

    /// How much this user relies on element `key` (`0..1`).
    pub fn affinity(&self, key: &str) -> f32 {
        self.profile.affinity(self.id(key), self.now)
    }

    /// Visual weight element `key` has earned for this user.
    pub fn prominence(&mut self, key: &str) -> Prominence {
        let id = self.id(key);
        self.profile.prominence(id, self.now)
    }

    /// Counts an interaction with `key` that the library cannot see by itself
    /// (a keyboard shortcut, a command run from elsewhere...).
    pub fn record(&mut self, key: &str) {
        let id = self.id(key);
        self.profile.record(id, self.now);
    }

    // ----- layout ----------------------------------------------------------

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
        });
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
    pub(crate) fn group(
        &mut self,
        id: Id,
        dir: Dir,
        inset: Vec2,
        gap: f32,
        origin: Option<Vec2>,
        f: impl FnOnce(&mut Ui),
    ) -> (Vec2, Vec2) {
        let prev = self.sizes.get(&id).copied().unwrap_or(Vec2::ZERO);
        let origin = origin.unwrap_or_else(|| self.peek(prev));
        let parent = self.layouts.last().unwrap();
        let width = (parent.width - (origin.x - parent.origin.x) - 2.0 * inset.x).max(0.0);
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

    /// Whether the pointer is over `rect` as far as the current layer is
    /// concerned: inside the clip and not covered by a panel above.
    pub(crate) fn pointer_in(&self, rect: Rect) -> bool {
        let layer = self.draw.layer() as u8;
        self.input.pointer.is_some_and(|p| {
            rect.contains(p)
                && self.clip.is_none_or(|c| c.contains(p))
                && !self.blockers_prev.iter().any(|(l, r)| *l > layer && r.contains(p))
        })
    }

    pub(crate) fn hit(&mut self, id: Id, rect: Rect) -> Hit {
        let over = self.pointer_in(rect);
        if self.input.pressed && over && self.active.is_none() {
            self.active = Some(id);
        }
        let mine = self.active == Some(id);
        Hit {
            hovered: over && (self.active.is_none() || mine),
            held: mine && self.input.down,
            clicked: mine && over && self.input.released,
            released: mine && self.input.released,
        }
    }
}
