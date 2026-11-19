//! Accessibility: every frame the interface describes what it shows, so a
//! host can expose it to screen readers (for example through AccessKit).

use crate::geom::Rect;
use crate::id::Id;
use crate::ui::{Key, Ui};

/// What kind of element an [`AccessNode`] is. New roles may be added in
/// later versions: match it with a fallback arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Role {
    Button,
    ToggleButton,
    Switch,
    CheckBox,
    RadioButton,
    Slider,
    TextInput,
    MultilineTextInput,
    ComboBox,
    ListItem,
    MenuItem,
    Tab,
    Link,
    Label,
    Heading,
    Table,
    Row,
    /// A picture; its label says what it shows.
    Image,
    /// Elements that belong together (a card).
    Group,
    /// A dialog or a side panel over the rest.
    Dialog,
    /// An area whose content scrolls.
    ScrollView,
    /// The row of tabs.
    TabList,
    /// The options a menu unfolds.
    Menu,
    /// A tree of items that open and close.
    Tree,
    TreeItem,
    /// How far a task has got; its value is a percentage.
    ProgressBar,
    /// A message announced when it appears (a toast, a notice) without
    /// taking the focus.
    Status,
}

/// One element as assistive technology should see it.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessNode {
    pub id: Id,
    pub role: Role,
    pub label: String,
    /// Current value, for inputs, sliders and selects.
    pub value: Option<String>,
    /// On/off state, for switches, check boxes and the like.
    pub checked: Option<bool>,
    /// On-screen rectangle in logical pixels.
    pub rect: Rect,
    /// Inside [`Ui::enabled`]`(false, …)`: shown, but it does not respond.
    pub disabled: bool,
    /// The container this element is inside (a card, a dialog, a list),
    /// `None` at the top: with it the description is a tree.
    pub parent: Option<Id>,
    /// Open or closed, for what unfolds (a section, a branch, a menu).
    pub expanded: Option<bool>,
    /// Announced when it appears or changes, without taking the focus.
    pub live: bool,
}

impl Ui {
    /// Turns the accessibility description on or off (off by default, so
    /// it costs nothing unless a screen reader is listening).
    pub fn set_accessibility(&mut self, on: bool) {
        self.a11y_on = on;
    }

    /// Adds an element to this frame's accessibility description.
    pub(crate) fn describe(&mut self, id: Id, role: Role, label: &str, rect: Rect, value: Option<&str>, checked: Option<bool>) {
        if self.a11y_on {
            let (disabled, parent) = (self.disabled, self.a11y_parents.last().copied());
            let live = role == Role::Status;
            self.access.push(AccessNode {
                id,
                role,
                label: label.to_string(),
                value: value.map(str::to_string),
                checked,
                rect,
                disabled,
                parent,
                expanded: None,
                live,
            });
        }
    }

    /// Says that the element described last is open or closed.
    pub(crate) fn describe_expanded(&mut self, open: bool) {
        if let Some(n) = self.access.last_mut().filter(|_| self.a11y_on) {
            n.expanded = Some(open);
        }
    }

    /// Runs `f` with what it describes inside container `id`, then
    /// describes the container itself (its rectangle is known by then).
    pub(crate) fn describe_group<R>(&mut self, id: Id, role: Role, label: &str, f: impl FnOnce(&mut Ui) -> R) -> R {
        if !self.a11y_on {
            return f(self);
        }
        self.a11y_parents.push(id);
        let out = f(self);
        self.a11y_parents.pop();
        let rect = self.layout_rect(id).unwrap_or_default();
        self.describe(id, role, label, rect, None, None);
        out
    }

    /// Like [`describe`](Self::describe) for a value in `0..=1`, read as a percentage.
    pub(crate) fn describe_level(&mut self, id: Id, role: Role, label: &str, rect: Rect, v: f32) {
        if self.a11y_on {
            let value = format!("{:.0} %", v * 100.0);
            self.describe(id, role, label, rect, Some(&value), None);
        }
    }

    /// Net arrow-key movement aimed at `id` this frame: Right/Up positive,
    /// Left/Down negative (0 unless it has the focus).
    pub(crate) fn arrows(&self, id: Id) -> f32 {
        if self.focus != Some(id) {
            return 0.0;
        }
        self.keys
            .iter()
            .map(|k| match k {
                Key::Right | Key::Up => 1.0,
                Key::Left | Key::Down => -1.0,
                _ => 0.0,
            })
            .sum()
    }

    /// What the interface showed in the last frame, in reading order.
    pub fn access_nodes(&self) -> &[AccessNode] {
        &self.access
    }

    /// Activates element `id` as a click would (from a screen reader).
    pub fn access_click(&mut self, id: Id) {
        self.access_clicks.push(id);
    }

    /// Moves the keyboard focus to element `id` (from a screen reader).
    pub fn access_focus(&mut self, id: Id) {
        self.focus = Some(id);
        self.focus_jumped = true;
        self.anchor = None;
        self.keyboard_focus = true;
    }

    /// The element with the keyboard focus, if any.
    pub fn focused(&self) -> Option<Id> {
        self.focus
    }
}
