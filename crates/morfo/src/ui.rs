//! The immediate-mode front end with retained, spring-driven state.
//!
//! Each frame the application simply *declares* what should exist. Every
//! declared element owns a persistent [`Node`] keyed by its [`Id`]; the layout
//! only produces *targets* and the node's springs carry the real geometry and
//! colors there. So reordering, resizing, restyling or re-theming anything is
//! automatically a fluid morph, with no transition code anywhere.

use crate::adapt::{Profile, Prominence};
use crate::anim::{Motion, Spring, SpringKind, SpringStep};
use crate::draw::{DrawCmd, DrawList};
use crate::geom::{Color, Rect, Vec2};
use crate::id::{Id, IdMap};
pub(crate) use crate::layout::{Dir, Layout};
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
    Up,
    Down,
    /// Activates the focused control, like Enter.
    Space,
    SelectAll,
    Copy,
    Cut,
}

/// Modifier keys held down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
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
    /// Extra widget-specific springs.
    pub e1: Motion<f32>,
    pub e2: Motion<f32>,
    /// Widget-specific moment in time (hover start, last edit...).
    pub stamp: f64,
    /// Widget-specific state flags.
    pub open: bool,
    pub latch: bool,
    pub index: usize,
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
            e1: Motion::new(0.0),
            e2: Motion::new(0.0),
            stamp: 0.0,
            open: false,
            latch: false,
            index: 0,
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
    pub(crate) morphs: IdMap<crate::morph::Morph>,
    pub(crate) top_inset: f32,
    /// Text owned by widgets themselves (the command palette's query).
    pub(crate) strings: IdMap<String>,
    /// Inner width for the next group, overriding what the parent leaves.
    pub(crate) next_width: Option<f32>,
    pub(crate) morph_buf: Vec<DrawCmd>,
    pub(crate) mods: Modifiers,
    /// Other end of the selection in the focused text field (byte offset).
    pub(crate) anchor: Option<usize>,
    pub(crate) pasted: String,
    pub(crate) clipboard_out: Option<String>,
    pub(crate) preedit: String,
    pub(crate) ime_rect: Option<Rect>,
    /// The focus was last moved with the keyboard: show the focus ring.
    pub(crate) keyboard_focus: bool,
    pub(crate) focus_rect: Option<Rect>,
    /// Focus before this frame's press (text fields place their caret from it).
    pub(crate) focus_before: Option<Id>,
    /// The focus moved to a new element during this frame's press.
    pub(crate) focus_changed: bool,
    pub(crate) text_ids: Vec<Id>,
    pub(crate) access: Vec<crate::a11y::AccessNode>,
    pub(crate) access_clicks: Vec<Id>,
    pub(crate) a11y_on: bool,
    /// Human-readable names of the elements the profile learned about.
    pub(crate) labels: IdMap<String>,
    /// Last content of each transient element, for its exit animation.
    pub(crate) transients: IdMap<crate::morph::Snapshot>,
    /// The theme as set; `theme` is it with the current density applied.
    pub(crate) base_theme: Theme,
    pub(crate) density: Motion<f32>,
    pub(crate) auto_density: bool,
    /// Every spring jumps to its target (see [`Ui::set_instant`]).
    pub(crate) instant: bool,
    /// Scroll areas declared so far; their offset lives in their node.
    pub(crate) scroll_ids: Vec<Id>,
    /// Offsets from [`Ui::restore_state`], applied once their area has
    /// measured its content: the offset and for how many more frames.
    pub(crate) scroll_restore: IdMap<(f32, u8)>,
    pub(crate) notices: Vec<crate::overlay::Notice>,
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
            morphs: IdMap::default(),
            top_inset: 0.0,
            strings: IdMap::default(),
            next_width: None,
            morph_buf: Vec::new(),
            mods: Modifiers::default(),
            anchor: None,
            pasted: String::new(),
            clipboard_out: None,
            preedit: String::new(),
            ime_rect: None,
            keyboard_focus: false,
            focus_rect: None,
            focus_before: None,
            focus_changed: false,
            text_ids: Vec::new(),
            access: Vec::new(),
            access_clicks: Vec::new(),
            a11y_on: false,
            labels: IdMap::default(),
            transients: IdMap::default(),
            base_theme: theme,
            density: Motion::new(1.0),
            auto_density: false,
            instant: false,
            scroll_ids: Vec::new(),
            scroll_restore: IdMap::default(),
            notices: Vec::new(),
        }
    }

    // ----- configuration ---------------------------------------------------

    /// Installs the backend's text metrics.
    pub fn set_text_measure(&mut self, text: Box<dyn TextMeasure>) {
        self.text = text;
    }

    /// Switches theme. Colors morph; nothing jumps.
    pub fn set_theme(&mut self, theme: Theme) {
        self.base_theme = theme;
        self.theme = theme.with_density(self.density.get());
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
        self.focus.is_some_and(|f| self.text_ids.contains(&f))
    }

    /// Modifier keys currently held (Shift extends text selections).
    pub fn set_modifiers(&mut self, mods: Modifiers) {
        self.mods = mods;
    }

    /// Text pasted from the clipboard. Unlike [`text_input`](Self::text_input)
    /// it keeps line breaks, for multi-line fields.
    pub fn paste(&mut self, text: &str) {
        self.pasted.extend(text.chars().filter(|&c| c != '\r'));
    }

    /// Text the user copied or cut this frame; put it on the system
    /// clipboard.
    pub fn take_clipboard(&mut self) -> Option<String> {
        self.clipboard_out.take()
    }

    /// Text being composed with an input method (accents, CJK...), shown
    /// at the caret until it is committed through
    /// [`text_input`](Self::text_input). Empty to clear it.
    pub fn ime_preedit(&mut self, text: &str) {
        self.preedit.clear();
        self.preedit.push_str(text);
    }

    /// Where the caret of the focused text field is, so the host can place
    /// the input method's candidate window next to it.
    pub fn ime_area(&self) -> Option<Rect> {
        self.ime_rect
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
        let dt = if self.instant { 1e3 } else { ((now - self.now) as f32).clamp(0.0, 0.05) };
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
        // Adaptive density: the more someone uses the interface, the more
        // compact it gets — gradually, as everything else.
        let target = if self.auto_density {
            let e = self.profile.experience(now);
            1.0 - 0.2 * e / (e + 60.0)
        } else {
            1.0
        };
        self.density.set(target);
        self.animating |= self.density.tick(&self.steps[SpringKind::Gentle as usize]);
        self.theme = self.base_theme.with_density(self.density.get());
        self.pal = self.palette.get();
        self.draw.clear();
        std::mem::swap(&mut self.blockers, &mut self.blockers_prev);
        self.blockers.clear();
        self.focusables.clear();
        self.text_ids.clear();
        self.access.clear();
        self.ime_rect = None;
        self.focus_rect = None;
        self.focus_changed = false;
        self.focus_claimed = false;
        self.dismiss = false;
        self.wake = f64::INFINITY;
        self.top_inset = 0.0;
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
            fill_from: None,
        });
    }

    /// Ends the frame. Paint [`draw_list`](Self::draw_list) afterwards.
    pub fn end(&mut self) -> Frame {
        self.draw_notices();
        self.draw_toasts();
        self.draw_focus_ring();
        self.draw.flatten();
        // Keyboard focus: a press outside every field drops it, Tab cycles it.
        if self.input.pressed && !self.focus_claimed {
            self.focus = None;
        }
        if self.keys.contains(&Key::Tab) && !self.focusables.is_empty() {
            let n = self.focusables.len();
            let at = self.focus.and_then(|f| self.focusables.iter().position(|&x| x == f));
            let next = match (at, self.mods.shift) {
                (Some(i), false) => (i + 1) % n,
                (Some(i), true) => (i + n - 1) % n,
                (None, false) => 0,
                (None, true) => n - 1,
            };
            self.focus = Some(self.focusables[next]);
            self.cursor = usize::MAX;
            self.anchor = None;
            self.keyboard_focus = true;
        }
        // State changed by input is only declared on the next frame.
        let again = self.input.pressed
            || self.input.released
            || self.input.wheel != 0.0
            || !self.keys.is_empty()
            || !self.typed.is_empty()
            || !self.pasted.is_empty()
            || !self.access_clicks.is_empty();
        self.input.pressed = false;
        self.input.released = false;
        self.input.wheel = 0.0;
        self.keys.clear();
        self.typed.clear();
        self.pasted.clear();
        self.access_clicks.clear();
        if !self.input.down {
            self.active = None;
        }
        if self.frame.is_multiple_of(256) {
            let frame = self.frame;
            self.nodes.retain(|_, n| n.seen + 256 >= frame);
            let nodes = &self.nodes;
            self.morphs.retain(|id, _| nodes.contains_key(id));
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
        self.learn(id, key);
    }

    /// Like [`record`](Self::record), with the name people know it by
    /// (shown in [`habits`](Self::habits)).
    pub fn record_named(&mut self, key: &str, name: &str) {
        let id = self.id(key);
        self.learn(id, name);
    }

    /// Whether the pointer is over `rect` as far as the current layer is
    /// concerned: inside the clip and not covered by a panel above.
    pub(crate) fn pointer_in(&self, rect: Rect) -> bool {
        let layer = self.draw.layer() as u8;
        self.input.pointer.is_some_and(|p| {
            rect.contains(p) && self.clip.is_none_or(|c| c.contains(p)) && !self.blockers_prev.iter().any(|(l, r)| *l > layer && r.contains(p))
        })
    }

    /// Pointer and keyboard interaction for an element at `rect`. The
    /// element becomes focusable: Tab reaches it, and Enter or Space
    /// activate it while it has the focus.
    pub(crate) fn hit(&mut self, id: Id, rect: Rect) -> Hit {
        self.focusables.push(id);
        let mut hit = self.hit_inert(id, rect);
        if self.input.pressed && self.active == Some(id) && self.focus != Some(id) {
            self.focus_before = self.focus;
            self.focus_changed = true;
            self.focus = Some(id);
            self.anchor = None;
            self.keyboard_focus = false;
        }
        if self.active == Some(id) {
            self.focus_claimed = true;
        }
        if self.focus == Some(id) {
            self.focus_rect = Some(rect);
            if self.keys.iter().any(|k| matches!(k, Key::Enter | Key::Space)) {
                hit.clicked = true;
            }
        }
        if self.access_clicks.contains(&id) {
            hit.clicked = true;
        }
        hit
    }

    /// Like [`hit`](Self::hit), for surfaces that only catch the pointer
    /// (panel backgrounds): never focusable.
    pub(crate) fn hit_inert(&mut self, id: Id, rect: Rect) -> Hit {
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

    /// Ring around the element focused with the keyboard. It is one shape
    /// that springs from element to element as the focus moves.
    fn draw_focus_ring(&mut self) {
        let ring = Id::new("morfo-focus-ring");
        let target = match (self.keyboard_focus, self.focus_rect, self.focus) {
            (true, Some(r), Some(f)) if !self.text_ids.contains(&f) => r.inset(-4.0),
            _ => return,
        };
        let (k, life) = (self.steps[SpringKind::Bouncy as usize], self.steps[SpringKind::Smooth as usize]);
        let n = self.nodes.entry(ring).or_insert_with(Node::new);
        self.animating |= n.begin(self.frame, target, &k, &life);
        let (r, a) = (n.rect.get(), n.life.get().clamp(0.0, 1.0));
        let below = self.draw.set_layer(2);
        let cull = self.draw.cull.take();
        let radius = (r.h * 0.5).min(18.0);
        self.draw.rect_stroked(r, radius, Color::TRANSPARENT, self.pal[crate::theme::ACCENT].with_alpha(0.9 * a), 2.0);
        self.draw.cull = cull;
        self.draw.set_layer(below);
    }
}
