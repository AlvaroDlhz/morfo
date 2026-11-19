//! Testing an interface without a window: a [`Harness`] runs frames at a
//! steady clock, clicks and types into elements found by their keys, and
//! reads back what was drawn and what screen readers are told.
//!
//! ```
//! use morfo::testing::Harness;
//!
//! let mut count = 0;
//! let mut h = Harness::new();
//! let mut app = |ui: &mut morfo::Ui| {
//!     ui.label(&format!("Clicked {count} times"));
//!     if ui.button("more", "More").clicked {
//!         count += 1;
//!     }
//! };
//! h.settle(&mut app);
//! h.click("more", &mut app);
//! assert!(h.shows("Clicked 1 times"));
//! ```
//!
//! Text is measured approximately unless a real measure is set on
//! [`Harness::ui`]; layouts are the same either way, up to the widths of
//! words.

use crate::a11y::AccessNode;
use crate::draw::DrawCmd;
use crate::geom::Vec2;
use crate::id::Id;
use crate::theme::Theme;
use crate::ui::{Frame, Key, Ui};

/// An interface under test. See the [module](self).
pub struct Harness {
    /// The interface itself, for anything the harness does not cover.
    pub ui: Ui,
    /// The clock, in seconds; each frame moves it a sixtieth.
    pub time: f64,
    /// The size of the view, in logical pixels.
    pub view: Vec2,
}

impl Default for Harness {
    fn default() -> Self {
        Harness::new()
    }
}

impl Harness {
    /// An 800×600 view in the dark theme, describing itself for screen
    /// readers (so that [`node`](Self::node) works).
    pub fn new() -> Harness {
        Harness::with(Theme::dark(), Vec2::new(800.0, 600.0))
    }

    pub fn with(theme: Theme, view: Vec2) -> Harness {
        let mut ui = Ui::new(theme);
        ui.set_accessibility(true);
        Harness { ui, time: 0.0, view }
    }

    /// One frame of `app`.
    pub fn frame(&mut self, app: &mut impl FnMut(&mut Ui)) -> Frame {
        self.time += 1.0 / 60.0;
        self.ui.begin(self.time, self.view);
        app(&mut self.ui);
        self.ui.end()
    }

    /// Frames until everything is at rest. Panics if it never comes to rest
    /// (an animation that never ends is a bug worth knowing about).
    pub fn settle(&mut self, app: &mut impl FnMut(&mut Ui)) {
        for _ in 0..2000 {
            if !self.frame(app).animating {
                return;
            }
        }
        panic!("the interface never came to rest");
    }

    /// The element declared with `key` (anywhere: inside cards, dialogs,
    /// scroll areas). Panics if there is none, naming it.
    pub fn id(&self, key: &str) -> Id {
        self.ui.find(key).unwrap_or_else(|| panic!("no element with key {key:?}"))
    }

    /// Moves the pointer onto the element `key`, presses and releases,
    /// then lets the interface settle.
    pub fn click(&mut self, key: &str, app: &mut impl FnMut(&mut Ui)) {
        let id = self.id(key);
        let at = self.ui.rect_of(id).unwrap_or_else(|| panic!("{key:?} is not on screen")).center();
        self.ui.pointer_moved(at);
        self.frame(app);
        self.ui.pointer_pressed();
        self.frame(app);
        self.ui.pointer_released();
        self.frame(app);
        self.settle(app);
    }

    /// Clicks `key` (a text field) and types `text` into it.
    pub fn type_into(&mut self, key: &str, text: &str, app: &mut impl FnMut(&mut Ui)) {
        self.click(key, app);
        self.ui.text_input(text);
        self.frame(app);
        self.settle(app);
    }

    /// Presses `key` (Tab, Enter, an arrow…) and lets the interface settle.
    pub fn press(&mut self, key: Key, app: &mut impl FnMut(&mut Ui)) {
        self.ui.key_pressed(key);
        self.frame(app);
        self.settle(app);
    }

    /// Every line of text drawn in the last frame, in paint order.
    pub fn texts(&self) -> Vec<String> {
        let list = self.ui.draw_list();
        list.commands()
            .iter()
            .filter_map(|c| if let DrawCmd::Text { start, len, .. } = *c { Some(list.text(start, len).to_string()) } else { None })
            .collect()
    }

    /// Whether `text` was drawn in the last frame, as a whole line or part
    /// of one.
    pub fn shows(&self, text: &str) -> bool {
        self.texts().iter().any(|t| t.contains(text))
    }

    /// What screen readers are told about the element labelled `label`.
    pub fn node(&self, label: &str) -> Option<&AccessNode> {
        self.ui.access_nodes().iter().find(|n| n.label == label)
    }
}
