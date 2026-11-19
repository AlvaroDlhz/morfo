//! The immediate-mode front end with retained, spring-driven state.
//!
//! Each frame the application simply *declares* what should exist. Every
//! declared element owns a persistent [`Node`] keyed by its [`Id`]; the layout
//! only produces *targets* and the node's springs carry the real geometry and
//! colors there. So reordering, resizing, restyling or re-theming anything is
//! automatically a fluid morph, with no transition code anywhere.

use crate::adapt::{Profile, Prominence};
use crate::anim::{Motion, MotionPreference, Spring, SpringKind, SpringStep};
use crate::draw::{DrawCmd, DrawList};
use crate::geom::{Color, Rect, Vec2};
use crate::id::{Id, IdMap};
pub(crate) use crate::layout::{Dir, Layout};
use crate::theme::{Palette, Theme, BG};

/// How far beyond its edges a finger still hits an element.
const TOUCH_SLOP: f32 = 6.0;
/// How long a press must last, without moving, to be a long press.
pub(crate) const LONG_PRESS: f64 = 0.5;
/// How far a finger may move before a press becomes a drag.
pub(crate) const DRAG_START: f32 = 8.0;

/// Line box height as a multiple of the font size.
pub const LINE_HEIGHT: f32 = 1.3;

/// Text metrics provider; implemented by rendering backends.
pub trait TextMeasure {
    /// Width in logical pixels of `text` on one line at font size `size`.
    fn width(&self, text: &str, size: f32) -> f32;

    /// Like [`width`](Self::width), in `style` (bold, italic, monospaced).
    /// Metrics without those faces measure every style alike.
    fn width_styled(&self, text: &str, size: f32, style: crate::draw::TextStyle) -> f32 {
        let _ = style;
        self.width(text, size)
    }
}

/// Hash of a string, eight bytes at a time: cheap enough to run on every
/// text of every frame.
pub(crate) fn hash_str(s: &str) -> u64 {
    const K: u64 = 0x9e37_79b9_7f4a_7c15;
    let bytes = s.as_bytes();
    let mut h = (bytes.len() as u64 ^ 0x51_7c_c1_b7_27_22_0a_95).wrapping_mul(K);
    let (chunks, rest) = bytes.as_chunks::<8>();
    for c in chunks {
        h = (h ^ u64::from_le_bytes(*c)).wrapping_mul(K);
        h ^= h >> 32;
    }
    let mut v = 0u64;
    for (i, &b) in rest.iter().enumerate() {
        v |= (b as u64) << (8 * i);
    }
    h = (h ^ v).wrapping_mul(K);
    h ^ (h >> 29)
}

/// The backend's text metrics behind a cache: an interface measures the
/// same few hundred strings frame after frame, and measuring can be costly
/// (font tables, or a call into the browser).
pub(crate) struct TextCache {
    inner: Box<dyn TextMeasure>,
    /// `false` for metrics cheaper than a cache lookup.
    cached: bool,
    widths: std::cell::RefCell<IdMap<f32>>,
}

impl TextCache {
    fn new(inner: Box<dyn TextMeasure>, cached: bool) -> TextCache {
        TextCache { inner, cached, widths: Default::default() }
    }

    /// Width of `text` on one line at font size `size`.
    pub(crate) fn width(&self, text: &str, size: f32) -> f32 {
        self.width_styled(text, size, crate::draw::TextStyle::REGULAR)
    }

    /// Width of `text` on one line at font size `size`, in `style`.
    pub(crate) fn width_styled(&self, text: &str, size: f32, style: crate::draw::TextStyle) -> f32 {
        if !self.cached || text.is_empty() {
            return self.inner.width_styled(text, size, style);
        }
        let key = Id(hash_str(text) ^ (size.to_bits() as u64 ^ (style.0 as u64) << 40).wrapping_mul(0xff51_afd7_ed55_8ccd));
        if let Some(&w) = self.widths.borrow().get(&key) {
            return w;
        }
        let w = self.inner.width_styled(text, size, style);
        let mut widths = self.widths.borrow_mut();
        // Text that keeps changing (a clock, a counter) must not pile up.
        if widths.len() > 16_384 {
            widths.clear();
        }
        widths.insert(key, w);
        w
    }
}

/// The metrics themselves, uncached: for callers that measure many
/// throwaway pieces (every prefix of a line being wrapped).
impl std::ops::Deref for TextCache {
    type Target = dyn TextMeasure;
    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
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
#[non_exhaustive]
pub enum Key {
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
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
    /// Takes the focused text field back one edit (Ctrl+Z).
    Undo,
    /// Brings back what was undone (Ctrl+Shift+Z, Ctrl+Y).
    Redo,
}

/// A direction on screen, for moving the focus with a gamepad, a remote
/// or arrow keys (see [`Ui::move_focus`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
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
    /// Held still for half a second (a finger asking for more: a context
    /// menu, a selection). Reported once; the release that follows is not
    /// a click.
    pub long_pressed: bool,
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
    /// The pointer is a finger.
    pub touch: bool,
    /// The finger lifted: the pointer goes away after this frame.
    pub lift: bool,
    /// Where and when the current press began.
    pub press_at: Option<Vec2>,
    pub press_time: f64,
    /// The long press of the current press was already reported.
    pub long_fired: bool,
    /// This press came right after another at the same spot.
    pub double: bool,
    /// Where and when the previous press was.
    pub last_press: (f64, Vec2),
    /// The pointer's vertical speed, smoothed (logical pixels a second).
    pub velocity: f32,
    pub last_y: Option<(f32, f64)>,
}

pub(crate) struct Hit {
    pub hovered: bool,
    pub held: bool,
    pub clicked: bool,
    /// Held still for [`LONG_PRESS`] seconds (reported once).
    pub long_pressed: bool,
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

    /// Whether the node was declared in the previous frame (or already in
    /// this one).
    pub fn present(&self, frame: u64) -> bool {
        self.seen + 1 >= frame
    }

    /// Carries the node along when its frame of reference moved by `dy` this
    /// frame (see `Ui::shift`). Call it once per frame, before placing it.
    pub fn reframe(&mut self, frame: u64, dy: f32) {
        if dy != 0.0 && self.present(frame) {
            self.rect.translate(Vec2::new(0.0, dy));
        }
    }

    /// First frame of this node's life: style values should snap, not animate.
    pub fn fresh(&self) -> bool {
        self.age <= 1
    }

    /// Makes a node that is about to be declared for the first time start
    /// at `from` and travel to its place, instead of appearing there (a tag
    /// flying out of the field it was typed in).
    pub fn launch(&mut self, frame: u64, from: Rect) {
        self.rect.snap(from);
        self.life.snap(0.0);
        self.age = 2;
        self.seen = frame;
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
    pub(crate) text: TextCache,
    /// How each paragraph was last broken into lines, and for what.
    pub(crate) wraps: IdMap<crate::text::Wrapped>,
    pub(crate) now: f64,
    pub(crate) frame: u64,
    pub(crate) animating: bool,
    viewport: Vec2,
    /// Areas of upper layers that swallow the pointer: this frame's, and last
    /// frame's (the ones hit-testing can already rely on).
    pub(crate) blockers: Vec<(u8, Rect)>,
    pub(crate) blockers_prev: Vec<(u8, Rect)>,
    /// The menu currently open, if any.
    pub(crate) open: Option<Id>,
    pub(crate) menu_more: bool,
    /// Set by [`Ui::close`]: the enclosing floating panel should close.
    pub(crate) dismiss: bool,
    pub(crate) focus: Option<Id>,
    pub(crate) focus_claimed: bool,
    pub(crate) focusables: Vec<Id>,
    /// A move of the focus asked for, done when the frame ends.
    focus_move: Option<Direction>,
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
    /// Containers being described, innermost last.
    pub(crate) a11y_parents: Vec<Id>,
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
    /// The words and formats of the components ([`Ui::set_locale`]).
    pub(crate) locale: crate::locale::Locale,
    /// How much the interface may move ([`Ui::set_motion`]).
    pub(crate) motion: MotionPreference,
    /// This frame nothing animates: instant mode, or no motion at all.
    pub(crate) still: bool,
    /// Scroll areas declared so far; their offset lives in their node.
    pub(crate) scroll_ids: Vec<Id>,
    /// Offsets from [`Ui::restore_state`], applied once their area has
    /// measured its content: the offset and for how many more frames.
    pub(crate) scroll_restore: IdMap<(f32, u8)>,
    pub(crate) notices: Vec<crate::overlay::Notice>,
    /// The next text field is a password ([`Ui::password_field`]).
    pub(crate) masking: bool,
    /// Each text field's undo history.
    pub(crate) histories: IdMap<crate::edit::History>,
    /// Shortcut fields declared this frame (they take every key).
    pub(crate) capture_ids: Vec<Id>,
    /// A key pressed with Ctrl, given by the host with [`Ui::chord`].
    pub(crate) chord: Option<String>,
    /// Cards being dragged on a board.
    pub(crate) drags: IdMap<crate::panes::Drag>,
    /// What each chart's hover card says, kept while it fades out.
    pub(crate) tips: IdMap<Vec<(Color, String)>>,
    /// Scroll areas being laid out, innermost last.
    pub(crate) scrolls: Vec<crate::scrolling::ScrollCtx>,
    /// Elements that came into view so far this frame (they enter one after
    /// another).
    pub(crate) reveal_batch: u32,
    /// Key (unscoped) of the anchor a scroll area should travel to, set by
    /// [`Ui::scroll_to`].
    pub(crate) scroll_request: Option<Id>,
    /// What each transition showed, and what it is leaving.
    pub(crate) swaps: IdMap<crate::transition::Swap>,
    /// Where the shared elements that took off this frame were.
    pub(crate) hero_launches: Vec<Rect>,
    /// A transition changed view this frame: shared elements may take off.
    pub(crate) collect_heroes: bool,
    /// Room taken at the top of the scroll view by pinned headers.
    pub(crate) pin_inset: f32,
    /// Seconds since the previous frame.
    pub(crate) dt: f32,
    /// Pinching between frames, and this frame's.
    pinch_pending: Option<(f32, Vec2)>,
    pinch: Option<(f32, Vec2)>,
    /// The scroll area a finger is dragging, and its offset when it began.
    pub(crate) drag_scroll: Option<(Id, f32)>,
    /// What is being dragged: from where, what it carries, and where in it
    /// the pointer holds it.
    pub(crate) dragging: Option<(Id, u64, Vec2)>,
    /// The scroll area holding the element that has the focus, and whether
    /// the focus just moved there from the keyboard (it scrolls into view).
    pub(crate) focus_in: Option<Id>,
    pub(crate) focus_jumped: bool,
    /// Keys declared this frame and the last, with the element each one
    /// became (for [`Ui::find`]).
    declared: Vec<(Id, Id)>,
    /// The inspector: on, and the keys elements were declared with.
    inspector: bool,
    inspected: IdMap<String>,
    declared_prev: Vec<(Id, Id)>,
    /// Cards far outside the view are left unbuilt ([`Ui::set_lazy`]).
    pub(crate) lazy: bool,
    pub(crate) dormant: IdMap<crate::lazy::Dormant>,
    /// The container being built holds the focus or a press, or declares
    /// something that lives outside it: it cannot be left unbuilt.
    pub(crate) busy: bool,
    /// The container being built was unbuilt until now: what it holds is
    /// not new, it shows as it would have been.
    pub(crate) waking: bool,
    /// When adaptive components may rearrange ([`Ui::set_adaptation`]).
    pub(crate) adaptation: crate::adapt::Adaptation,
    /// Frames left in which an on-request rearrangement may happen.
    pub(crate) adapt_frames: u8,
    /// Text size relative to the theme's ([`Ui::set_text_scale`]).
    text_scale: f32,
    /// Inside [`Ui::enabled`]`(false, …)`: nothing responds.
    pub(crate) disabled: bool,
    /// Variant of the buttons that do not name one ([`Ui::with_variant`]).
    pub(crate) style_variant: Option<crate::widgets::Variant>,
    /// New elements appear at once instead of entering (inside a shared
    /// element, which already travels).
    pub(crate) skip_entrance: bool,
    /// How far, this frame, the frame of reference of what is being laid
    /// out moved vertically on screen (the content of a scroll area as it
    /// scrolls). Elements move with it rigidly instead of springing after it.
    pub(crate) shift: f32,
    /// What lies behind the content being laid out, as a palette index: the
    /// window's background, a card's surface, a panel's.
    pub(crate) ground: usize,
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
            text: TextCache::new(Box::new(ApproxText), false),
            wraps: IdMap::default(),
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
            focus_move: None,
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
            a11y_parents: Vec::new(),
            access_clicks: Vec::new(),
            a11y_on: false,
            labels: IdMap::default(),
            transients: IdMap::default(),
            base_theme: theme,
            density: Motion::new(1.0),
            auto_density: false,
            instant: false,
            locale: crate::locale::Locale::ENGLISH,
            motion: MotionPreference::Full,
            still: false,
            scroll_ids: Vec::new(),
            scroll_restore: IdMap::default(),
            notices: Vec::new(),
            capture_ids: Vec::new(),
            histories: IdMap::default(),
            masking: false,
            chord: None,
            drags: IdMap::default(),
            tips: IdMap::default(),
            scrolls: Vec::new(),
            reveal_batch: 0,
            scroll_request: None,
            swaps: IdMap::default(),
            hero_launches: Vec::new(),
            collect_heroes: false,
            pin_inset: 0.0,
            dt: 0.0,
            pinch_pending: None,
            pinch: None,
            drag_scroll: None,
            dragging: None,
            focus_in: None,
            focus_jumped: false,
            declared: Vec::new(),
            inspector: false,
            inspected: IdMap::default(),
            declared_prev: Vec::new(),
            lazy: true,
            adaptation: crate::adapt::Adaptation::Live,
            adapt_frames: 4,
            text_scale: 1.0,
            dormant: IdMap::default(),
            busy: false,
            waking: false,
            disabled: false,
            style_variant: None,
            skip_entrance: false,
            shift: 0.0,
            ground: BG,
        }
    }

    // ----- configuration ---------------------------------------------------

    /// Installs the backend's text metrics.
    pub fn set_text_measure(&mut self, text: Box<dyn TextMeasure>) {
        self.text = TextCache::new(text, true);
        self.wraps.clear();
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
        self.adapt_frames = 4;
    }

    /// When the interface may rearrange itself after its user (see
    /// [`Adaptation`](crate::adapt::Adaptation)). `Live` by default.
    pub fn set_adaptation(&mut self, adaptation: crate::adapt::Adaptation) {
        self.adaptation = adaptation;
    }

    pub fn adaptation(&self) -> crate::adapt::Adaptation {
        self.adaptation
    }

    /// With [`Adaptation::OnRequest`](crate::adapt::Adaptation::OnRequest):
    /// rearrange now, after what has been learned (a "Fit to my use"
    /// button). It also happens once when a profile is set.
    pub fn adapt_now(&mut self) {
        self.adapt_frames = 4;
    }

    /// Whether adaptive components may rearrange this frame.
    pub(crate) fn may_rearrange(&self) -> bool {
        use crate::adapt::Adaptation as A;
        match self.adaptation {
            A::Live => true,
            A::OnRequest => self.adapt_frames > 0,
            _ => false,
        }
    }

    /// Text at `scale` times the theme's size (`2.0` for 200 %), for people
    /// who need it larger (WCAG 1.4.4): everything reflows, and controls
    /// grow to fit their text.
    pub fn set_text_scale(&mut self, scale: f32) {
        self.text_scale = scale.clamp(0.5, 4.0);
        self.wraps.clear();
    }

    pub fn text_scale(&self) -> f32 {
        self.text_scale
    }

    // ----- input -----------------------------------------------------------

    /// Moves the keyboard focus to the nearest control in `dir` on screen,
    /// as a gamepad's pad, a TV remote or a host's arrow keys would: the
    /// nearest along that way, with controls in line ahead of those off to
    /// the side. With nothing focused, the first control takes it.
    pub fn move_focus(&mut self, dir: Direction) {
        self.focus_move = Some(dir);
    }

    fn focus_toward(&mut self, dir: Direction) {
        let from = self.focus.filter(|f| self.focusables.contains(f)).and_then(|f| self.rect_of(f));
        let next = match from {
            None => self.focusables.first().copied(),
            Some(r) => {
                let c = r.center();
                let mut best: Option<(f32, Id)> = None;
                for &id in &self.focusables {
                    if Some(id) == self.focus {
                        continue;
                    }
                    let Some(q) = self.rect_of(id) else { continue };
                    // How far ahead it is, and how far off to the side.
                    let (ahead, side) = match dir {
                        Direction::Up => (r.y - q.bottom(), (q.center().x - c.x).abs()),
                        Direction::Down => (q.y - r.bottom(), (q.center().x - c.x).abs()),
                        Direction::Left => (r.x - q.right(), (q.center().y - c.y).abs()),
                        Direction::Right => (q.x - r.right(), (q.center().y - c.y).abs()),
                    };
                    let center_ahead = match dir {
                        Direction::Up => c.y - q.center().y,
                        Direction::Down => q.center().y - c.y,
                        Direction::Left => c.x - q.center().x,
                        Direction::Right => q.center().x - c.x,
                    };
                    if center_ahead <= 1.0 || ahead < -r.w.min(r.h) * 0.5 {
                        continue;
                    }
                    let score = ahead.max(0.0) + side * 2.0;
                    if best.is_none_or(|(s, _)| score < s) {
                        best = Some((score, id));
                    }
                }
                best.map(|(_, id)| id)
            }
        };
        if let Some(id) = next {
            self.focus = Some(id);
            self.cursor = usize::MAX;
            self.anchor = None;
            self.keyboard_focus = true;
            self.focus_jumped = true;
        }
    }

    /// The mouse moved to `pos`, in logical pixels.
    pub fn pointer_moved(&mut self, pos: Vec2) {
        self.input.pointer = Some(self.mirror_point(pos));
    }

    /// Where a point on screen is in the layout, and the other way round:
    /// the same point, unless the interface is written right to left and
    /// shown as in a mirror.
    pub(crate) fn mirror_point(&self, p: Vec2) -> Vec2 {
        if self.locale.rtl {
            Vec2::new(self.viewport.x - p.x, p.y)
        } else {
            p
        }
    }

    /// For text being edited, which a mirror moves without flipping: `x`
    /// (from the pointer, mirrored) as it is inside the unflipped `zone`.
    pub(crate) fn upright_x(&self, x: f32, zone: Rect) -> f32 {
        if self.locale.rtl {
            zone.x + zone.right() - x
        } else {
            x
        }
    }

    /// A rectangle inside an upright `zone`, given so that mirroring it
    /// puts it where the zone's unflipped contents are.
    pub(crate) fn upright_rect(&self, r: Rect, zone: Rect) -> Rect {
        if self.locale.rtl {
            Rect::new(zone.x + zone.right() - r.x - r.w, r.y, r.w, r.h)
        } else {
            r
        }
    }

    /// [`mirror_point`](Self::mirror_point) for a rectangle.
    pub(crate) fn mirror_rect(&self, r: Rect) -> Rect {
        if self.locale.rtl {
            Rect::new(self.viewport.x - r.x - r.w, r.y, r.w, r.h)
        } else {
            r
        }
    }

    pub fn pointer_left(&mut self) {
        self.input.pointer = None;
    }

    pub fn pointer_pressed(&mut self) {
        self.input.down = true;
        self.input.pressed = true;
        self.input.press_at = self.input.pointer;
        let at = self.input.pointer.unwrap_or_default();
        let (t, p) = self.input.last_press;
        self.input.double = self.now - t < 0.4 && (at - p).length() < 6.0;
        self.input.last_press = (self.now, at);
        self.input.press_time = self.now;
        self.input.long_fired = false;
        self.input.velocity = 0.0;
        self.input.last_y = None;
    }

    pub fn pointer_released(&mut self) {
        self.input.down = false;
        self.input.released = true;
    }

    /// A finger touched the screen at `at`. One finger at a time acts as
    /// the pointer, without hover.
    pub fn touch_started(&mut self, at: Vec2) {
        self.input.touch = true;
        self.pointer_moved(at);
        self.pointer_pressed();
    }

    /// Two fingers moved apart (`scale` > 1) or together around `center`
    /// since the last call: a pinch, for [`pinch`](Self::pinch) to read.
    pub fn pinch_moved(&mut self, scale: f32, center: Vec2) {
        let center = self.mirror_point(center);
        let (s, _) = self.pinch_pending.unwrap_or((1.0, center));
        self.pinch_pending = Some((s * scale, center));
    }

    /// The pinch since the last frame: how much the fingers spread (`1`:
    /// not at all) and where. For a zoomable map or photo of your own.
    pub fn pinch(&self) -> Option<(f32, Vec2)> {
        self.pinch
    }

    /// The finger moved.
    pub fn touch_moved(&mut self, at: Vec2) {
        self.pointer_moved(at);
    }

    /// The finger left the screen at `at`, or the system took the touch
    /// over (`cancelled`: nothing is clicked).
    pub fn touch_ended(&mut self, at: Vec2, cancelled: bool) {
        self.pointer_moved(at);
        if cancelled {
            self.input.down = false;
            self.active = None;
        } else {
            self.pointer_released();
        }
        // With no finger there is no hover.
        self.input.lift = true;
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

    /// Whether a shortcut field is waiting for a key combination: while it
    /// is, the host should hand it Ctrl+key with [`chord`](Self::chord)
    /// instead of running its own shortcuts.
    pub fn wants_keys(&self) -> bool {
        self.focus.is_some_and(|f| self.capture_ids.contains(&f))
    }

    /// A key pressed together with Ctrl (`"k"`, `"F5"`), which a host does
    /// not send as text. Only a focused shortcut field uses it.
    pub fn chord(&mut self, key: &str) {
        self.chord = Some(key.to_string());
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
        self.ime_rect.map(|r| self.mirror_rect(r))
    }

    /// The language and formats of what the components show by themselves
    /// (month names, "More…", numbers…). English by default.
    pub fn set_locale(&mut self, locale: crate::locale::Locale) {
        self.locale = locale;
        // Paragraphs measured with other words are measured again.
        self.wraps.clear();
    }

    pub fn locale(&self) -> &crate::locale::Locale {
        &self.locale
    }

    /// How much the interface may move (see [`MotionPreference`]). Start it
    /// from the system's setting; changing it takes effect at once.
    pub fn set_motion(&mut self, motion: MotionPreference) {
        self.motion = motion;
    }

    pub fn motion(&self) -> MotionPreference {
        self.motion
    }

    /// Whether motion that only decorates should be left out (anything but
    /// [`MotionPreference::Full`]): for animations of your own.
    pub fn reduced_motion(&self) -> bool {
        self.motion != MotionPreference::Full || self.instant
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
        self.still = self.instant || self.motion == MotionPreference::Off;
        let dt = if self.still { 1e3 } else { ((now - self.now) as f32).clamp(0.0, 0.05) };
        self.now = now;
        self.dt = dt;
        self.frame += 1;
        self.viewport = viewport;
        let springs = match self.motion {
            MotionPreference::Reduced => Spring::REDUCED,
            _ => [Spring::SNAPPY, Spring::SMOOTH, Spring::GENTLE, Spring::BOUNCY],
        };
        self.steps = springs.map(|s| SpringStep::new(s, dt));
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
        if self.text_scale != 1.0 {
            // Larger text needs taller controls to fit in.
            self.theme.font *= self.text_scale;
            self.theme.control = self.theme.control.max(self.theme.font * 2.6);
        }
        self.adapt_frames = self.adapt_frames.saturating_sub(1);
        self.pal = self.palette.get();
        self.draw.clear();
        self.draw.rtl = self.locale.rtl;
        std::mem::swap(&mut self.blockers, &mut self.blockers_prev);
        self.blockers.clear();
        self.focusables.clear();
        self.text_ids.clear();
        self.capture_ids.clear();
        self.access.clear();
        self.a11y_parents.clear();
        self.ime_rect = None;
        self.focus_rect = None;
        self.focus_changed = false;
        self.focus_claimed = false;
        self.dismiss = false;
        self.wake = f64::INFINITY;
        self.top_inset = 0.0;
        self.scrolls.clear();
        self.reveal_batch = 0;
        self.pin_inset = 0.0;
        self.collect_heroes = false;
        self.shift = 0.0;
        self.ground = BG;
        self.pinch = self.pinch_pending.take();
        // The pointer's speed, for flinging a scroll area.
        if let (true, Some(p)) = (self.input.down, self.input.pointer) {
            if let Some((y, at)) = self.input.last_y {
                let dt = (now - at) as f32;
                if dt > 0.0 {
                    let v = (p.y - y) / dt;
                    self.input.velocity = self.input.velocity * 0.4 + v * 0.6;
                }
            }
            self.input.last_y = Some((p.y, now));
        }
        self.disabled = false;
        self.busy = false;
        self.waking = false;
        self.style_variant = None;
        std::mem::swap(&mut self.declared, &mut self.declared_prev);
        self.declared.clear();
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
        if self.inspector {
            self.draw_inspector();
        }
        self.draw.flatten();
        if self.locale.rtl {
            // Laid out left to right, shown as in a mirror.
            let (w, text) = (self.viewport.x, &self.text);
            self.draw.mirror(w, |s, size, style| text.width_styled(s, size, style));
            for n in &mut self.access {
                n.rect = Rect::new(w - n.rect.x - n.rect.w, n.rect.y, n.rect.w, n.rect.h);
            }
        }
        // Keyboard focus: a press outside every field drops it, Tab cycles it.
        if self.input.pressed && !self.focus_claimed {
            self.focus = None;
        }
        self.focus_jumped = false;
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
            self.focus_jumped = true;
        }
        let moved = self.focus_move.take().map(|dir| self.focus_toward(dir)).is_some();
        // State changed by input is only declared on the next frame.
        let again = moved
            || self.input.pressed
            || self.input.released
            || self.input.wheel != 0.0
            || !self.keys.is_empty()
            || !self.typed.is_empty()
            || !self.pasted.is_empty()
            || self.chord.is_some()
            || !self.access_clicks.is_empty();
        self.input.pressed = false;
        self.input.released = false;
        self.input.wheel = 0.0;
        if std::mem::take(&mut self.input.lift) {
            self.input.pointer = None;
        }
        self.keys.clear();
        self.typed.clear();
        self.pasted.clear();
        self.chord = None;
        self.access_clicks.clear();
        if !self.input.down {
            self.active = None;
            self.drag_scroll = None;
            self.dragging = None;
        }
        if self.frame.is_multiple_of(256) {
            if let Some(age) = self.profile.max_age {
                self.profile.prune(self.now, age);
            }
            let frame = self.frame;
            self.nodes.retain(|_, n| n.seen + 256 >= frame);
            let nodes = &self.nodes;
            self.morphs.retain(|id, _| nodes.contains_key(id));
            self.swaps.retain(|id, _| nodes.contains_key(id));
            self.wraps.retain(|id, _| nodes.contains_key(id));
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
    /// Containers with a key (`card`, `dialog`, `scroll`…) already do this
    /// for their content.
    pub fn scope(&mut self, key: &str, f: impl FnOnce(&mut Ui)) {
        let id = self.scope.child(key);
        self.within(id, f);
    }

    /// Runs `f` with keys namespaced under `id`: the content of a component
    /// belongs to it, so the same component can be repeated without its
    /// insides colliding.
    pub(crate) fn within<R>(&mut self, id: Id, f: impl FnOnce(&mut Ui) -> R) -> R {
        let saved = std::mem::replace(&mut self.scope, id);
        let out = f(self);
        self.scope = saved;
        out
    }

    /// Id of `key` in the current scope, for the element being declared
    /// with it (remembered for [`find`](Self::find)).
    pub(crate) fn declare(&mut self, key: &str) -> Id {
        let id = self.scope.child(key);
        self.declared.push((Id::new(key), id));
        if self.inspector && !self.inspected.contains_key(&id) {
            self.inspected.insert(id, key.to_string());
        }
        id
    }

    /// Turns the inspector on or off: the element under the pointer is
    /// outlined, with its key and size, and a corner shows how many
    /// elements live and how many shapes the frame has. For finding out
    /// why something is where it is.
    pub fn set_inspector(&mut self, on: bool) {
        self.inspector = on;
        if !on {
            self.inspected = IdMap::default();
        }
    }

    fn draw_inspector(&mut self) {
        let frame = self.frame;
        let shapes = self.draw.len();
        let p = self.input.pointer;
        // The smallest element under the pointer.
        let under = p.and_then(|p| {
            self.nodes
                .iter()
                .filter(|(_, n)| n.present(frame))
                .map(|(id, n)| (*id, n.rect.get()))
                .filter(|(_, r)| r.contains(p) && r.w * r.h > 0.0)
                .min_by(|a, b| (a.1.w * a.1.h).total_cmp(&(b.1.w * b.1.h)))
        });
        let layer = self.draw.set_layer(crate::draw::LAYERS - 1);
        let (font, warn) = (12.0, Color::hex(0xff3d7f));
        let tag = |ui: &mut Ui, at: Vec2, text: &str| {
            let w = ui.text.width(text, font) + 12.0;
            let r = Rect::new(at.x, at.y, w, font * LINE_HEIGHT + 6.0);
            ui.draw.rect(r, 4.0, Color::rgba(0.0, 0.0, 0.0, 0.85));
            ui.draw.add_code(Vec2::new(r.x + 6.0, r.y + 3.0), font, Color::WHITE, text);
        };
        if let Some((id, r)) = under {
            self.draw.rect_stroked(r, 2.0, warn.with_alpha(0.08), warn, 1.5);
            let name = self.inspected.get(&id).cloned().unwrap_or_else(|| format!("{id:?}"));
            let text = format!("{name}  {:.0}×{:.0} at {:.0},{:.0}", r.w, r.h, r.x, r.y);
            let y = if r.y > 24.0 { r.y - font * LINE_HEIGHT - 8.0 } else { r.bottom() + 2.0 };
            tag(self, Vec2::new(r.x, y), &text);
        }
        let stats = format!("{} elements · {} shapes", self.nodes.len(), shapes);
        let w = self.text.width(&stats, font) + 12.0;
        tag(self, Vec2::new(self.viewport.x - w - 8.0, self.viewport.y - font * LINE_HEIGHT - 14.0), &stats);
        self.draw.set_layer(layer);
    }

    /// The element declared with `key` in the last frame, wherever it was
    /// nested: inside a card, a scroll area or a dialog its id is not
    /// `ui.id(key)` but that of its container's `.child(key)`. If several
    /// elements used `key`, it is the first one; [`find_all`](Self::find_all)
    /// gives them all, in order.
    pub fn find(&self, key: &str) -> Option<Id> {
        self.find_all(key).into_iter().next()
    }

    /// Every element declared with `key` in the last frame, in order.
    /// Called while building a frame, what this frame has declared so far
    /// comes first.
    pub fn find_all(&self, key: &str) -> Vec<Id> {
        let k = Id::new(key);
        let mut out: Vec<Id> = self.declared.iter().filter(|(d, _)| *d == k).map(|(_, id)| *id).collect();
        for &(d, id) in &self.declared_prev {
            if d == k && !out.contains(&id) {
                out.push(id);
            }
        }
        out
    }

    /// On-screen rectangle of an element, if it exists.
    pub fn rect_of(&self, id: Id) -> Option<Rect> {
        self.layout_rect(id).map(|r| self.mirror_rect(r))
    }

    /// Where an element is in the layout (on screen unless mirrored).
    pub(crate) fn layout_rect(&self, id: Id) -> Option<Rect> {
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
        if self.disabled {
            return false;
        }
        let layer = self.draw.layer() as u8;
        // A fingertip is broad: touch targets reach a little further.
        let rect = if self.input.touch { rect.inset(-TOUCH_SLOP) } else { rect };
        self.input.pointer.is_some_and(|p| {
            rect.contains(p) && self.clip.is_none_or(|c| c.contains(p)) && !self.blockers_prev.iter().any(|(l, r)| *l > layer && r.contains(p))
        })
    }

    /// Pointer and keyboard interaction for an element at `rect`. The
    /// element becomes focusable: Tab reaches it, and Enter or Space
    /// activate it while it has the focus.
    pub(crate) fn hit(&mut self, id: Id, rect: Rect) -> Hit {
        if self.disabled {
            // Disabled: not reachable with Tab, and it lets go of the focus.
            if self.focus == Some(id) {
                self.focus = None;
            }
            return Hit { hovered: false, held: false, clicked: false, released: false, long_pressed: false };
        }
        self.focusables.push(id);
        if self.focus == Some(id) {
            self.busy = true;
        }
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
            self.focus_in = self.scrolls.last().map(|c| c.id);
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
        self.busy |= mine;
        // Held still long enough: a long press (not a click when released).
        let still = self.input.press_at.zip(self.input.pointer).is_none_or(|(a, p)| (p - a).length() < DRAG_START);
        let long = mine && self.input.down && still && !self.input.long_fired && self.now - self.input.press_time >= LONG_PRESS;
        if long {
            self.input.long_fired = true;
        }
        if mine && self.input.down && still && !self.input.long_fired {
            self.wake_at(self.input.press_time + LONG_PRESS);
        }
        Hit {
            hovered: over && (self.active.is_none() || mine) && !(self.input.touch && !self.input.down && !self.input.released),
            held: mine && self.input.down,
            clicked: mine && over && self.input.released && !self.input.long_fired,
            released: mine && self.input.released,
            long_pressed: long,
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
