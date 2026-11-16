//! Accessibility: every frame the interface describes what it shows, so a
//! host can expose it to screen readers (for example through AccessKit).

use crate::geom::Rect;
use crate::id::Id;
use crate::ui::{Key, Ui};

/// What kind of element an [`AccessNode`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
            self.access.push(AccessNode { id, role, label: label.to_string(), value: value.map(str::to_string), checked, rect });
        }
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
        self.anchor = None;
        self.keyboard_focus = true;
    }

    /// The element with the keyboard focus, if any.
    pub fn focused(&self) -> Option<Id> {
        self.focus
    }
}
