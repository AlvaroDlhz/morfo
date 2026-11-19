//! Screen readers on the web. A canvas is a picture to them, so the
//! interface's description ([`Ui::access_nodes`]) is mirrored into hidden
//! DOM elements with ARIA roles and states, one per element, nested as the
//! interface nests them and laid exactly over what is drawn (touch screen
//! readers explore by position). They are transparent and let the pointer
//! through; focus and clicks from the reader go back to the interface.
//!
//! It costs nothing until it is needed: a visually hidden "Enable
//! accessibility" button comes first in the Tab order, and activating it
//! turns the mirror on (as does `?a11y` in the page's address).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use morfo::{AccessNode, Id, Role, Ui};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlElement};

/// The ARIA role for a morfo role (`None`: plain text).
fn aria(role: Role) -> Option<&'static str> {
    Some(match role {
        Role::Button | Role::ToggleButton => "button",
        Role::Switch => "switch",
        Role::CheckBox => "checkbox",
        Role::RadioButton => "radio",
        Role::Slider => "slider",
        Role::TextInput | Role::MultilineTextInput => "textbox",
        Role::ComboBox => "combobox",
        Role::ListItem => "listitem",
        Role::MenuItem => "menuitem",
        Role::Tab => "tab",
        Role::Link => "link",
        Role::Heading => "heading",
        Role::Table => "table",
        Role::Row => "row",
        Role::Image => "img",
        Role::Group => "group",
        Role::Dialog => "dialog",
        Role::ScrollView => "region",
        Role::TabList => "tablist",
        Role::Menu => "menu",
        Role::Tree => "tree",
        Role::TreeItem => "treeitem",
        Role::ProgressBar => "progressbar",
        Role::Status => "status",
        _ => return None,
    })
}

/// Whether people act on an element of this role (it takes the focus).
fn actionable(role: Role) -> bool {
    !matches!(
        role,
        Role::Label
            | Role::Heading
            | Role::Image
            | Role::Group
            | Role::Dialog
            | Role::ScrollView
            | Role::TabList
            | Role::Menu
            | Role::Tree
            | Role::Table
            | Role::ProgressBar
            | Role::Status
    )
}

/// What the reader asked for since the last frame.
#[derive(Default)]
pub(crate) struct Requests {
    pub enable: bool,
    pub focus: Option<Id>,
    pub click: Option<Id>,
}

/// The mirror of the interface in the DOM.
pub(crate) struct Mirror {
    document: Document,
    pub root: HtmlElement,
    /// The "Enable accessibility" button.
    pub enable: HtmlElement,
    /// Each element, and the state it was last given (to touch the DOM
    /// only when something changed).
    nodes: HashMap<Id, (HtmlElement, String, Option<Id>)>,
    pub requests: Rc<RefCell<Requests>>,
    pub on: bool,
}

impl Mirror {
    /// Sets the mirror up next to `canvas`, with its "Enable accessibility"
    /// button.
    pub fn new(document: &Document, canvas: &web_sys::HtmlCanvasElement, label: &str) -> Result<Mirror, JsValue> {
        let root = document.create_element("div")?.dyn_into::<HtmlElement>()?;
        for (k, v) in [("position", "absolute"), ("left", "0"), ("top", "0"), ("width", "0"), ("height", "0"), ("pointer-events", "none")] {
            root.style().set_property(k, v)?;
        }
        root.set_attribute("role", "application")?;
        root.set_attribute("aria-label", label)?;
        let requests = Rc::new(RefCell::new(Requests::default()));
        // The way in: first in the Tab order, invisible until focused.
        let enable = document.create_element("button")?.dyn_into::<HtmlElement>()?;
        enable.set_text_content(Some("Enable accessibility"));
        hide(&enable)?;
        for (kind, show) in [("focus", true), ("blur", false)] {
            let el = enable.clone();
            let cb = Closure::<dyn FnMut()>::new(move || {
                let _ = if show { el.style().set_property("clip", "auto") } else { hide(&el) };
            });
            enable.add_event_listener_with_callback(kind, cb.as_ref().unchecked_ref())?;
            cb.forget();
        }
        let r = requests.clone();
        let cb = Closure::<dyn FnMut()>::new(move || r.borrow_mut().enable = true);
        enable.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())?;
        cb.forget();
        let parent = canvas.parent_node().ok_or("the canvas has no parent")?;
        parent.insert_before(&enable, Some(canvas))?;
        parent.insert_before(&root, Some(canvas))?;
        Ok(Mirror { document: document.clone(), root, enable, nodes: HashMap::new(), requests, on: false })
    }

    /// Brings the DOM in line with this frame's description. `origin` is
    /// where the canvas starts in the page, in CSS pixels.
    pub fn sync(&mut self, ui: &Ui, origin: (f64, f64)) {
        if !self.on {
            return;
        }
        let _ = self.root.style().set_property("left", &format!("{}px", origin.0));
        let _ = self.root.style().set_property("top", &format!("{}px", origin.1));
        let _ = self.root.set_attribute("dir", if ui.locale().rtl { "rtl" } else { "ltr" });
        let nodes = ui.access_nodes();
        let present: HashMap<Id, &AccessNode> = nodes.iter().map(|n| (n.id, n)).collect();
        // Gone from the interface: gone from the page.
        self.nodes.retain(|id, (el, _, _)| {
            let keep = present.contains_key(id);
            if !keep {
                el.remove();
            }
            keep
        });
        for n in nodes {
            let parent = n.parent.filter(|p| present.contains_key(p));
            let state = signature(n);
            let fresh = !self.nodes.contains_key(&n.id);
            if fresh {
                let Ok(el) = self.element(n.id) else { continue };
                self.nodes.insert(n.id, (el, String::new(), None));
            }
            let (el, old, old_parent) = self.nodes.get_mut(&n.id).unwrap();
            if *old != state {
                apply(el, n);
                *old = state;
            }
            // Laid over what is drawn, relative to its container.
            let base = parent.and_then(|p| present.get(&p)).map_or((0.0, 0.0), |p| (p.rect.x, p.rect.y));
            let style = el.style();
            let _ = style.set_property("left", &format!("{}px", n.rect.x - base.0));
            let _ = style.set_property("top", &format!("{}px", n.rect.y - base.1));
            let _ = style.set_property("width", &format!("{}px", n.rect.w));
            let _ = style.set_property("height", &format!("{}px", n.rect.h));
            if fresh || *old_parent != parent {
                *old_parent = parent;
                let el = el.clone();
                let host = parent.and_then(|p| self.nodes.get(&p)).map_or(self.root.clone(), |(e, _, _)| e.clone());
                let _ = host.append_child(&el);
            }
        }
    }

    /// A new element for `id`, wired to send the reader's focus and clicks
    /// back.
    fn element(&self, id: Id) -> Result<HtmlElement, JsValue> {
        let el = self.document.create_element("div")?.dyn_into::<HtmlElement>()?;
        for (k, v) in [("position", "absolute"), ("color", "transparent"), ("overflow", "hidden"), ("pointer-events", "none"), ("outline", "none")] {
            el.style().set_property(k, v)?;
        }
        for kind in ["focus", "click"] {
            let r = self.requests.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
                // Only the element itself, not one inside it.
                if e.target() != e.current_target() {
                    return;
                }
                let mut r = r.borrow_mut();
                if kind == "focus" {
                    r.focus = Some(id);
                } else {
                    r.click = Some(id);
                }
            });
            el.add_event_listener_with_callback(kind, cb.as_ref().unchecked_ref())?;
            cb.forget();
        }
        // Enter and Space on a focused element act on it.
        let r = self.requests.clone();
        let cb = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
            if e.target() == e.current_target() && (e.key() == "Enter" || e.key() == " ") {
                e.prevent_default();
                r.borrow_mut().click = Some(id);
            }
        });
        el.add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref())?;
        cb.forget();
        Ok(el)
    }
}

/// Visually hidden but still read and focusable.
fn hide(el: &HtmlElement) -> Result<(), JsValue> {
    for (k, v) in
        [("position", "absolute"), ("clip", "rect(0 0 0 0)"), ("width", "1px"), ("height", "1px"), ("overflow", "hidden"), ("white-space", "nowrap")]
    {
        el.style().set_property(k, v)?;
    }
    Ok(())
}

/// Everything that decides the element's attributes.
fn signature(n: &AccessNode) -> String {
    format!("{:?}|{}|{:?}|{:?}|{}|{:?}|{}", n.role, n.label, n.value, n.checked, n.disabled, n.expanded, n.live)
}

/// Gives `el` the role, name and state of `n`.
fn apply(el: &HtmlElement, n: &AccessNode) {
    let set = |k: &str, v: Option<&str>| {
        let _ = match v {
            Some(v) => el.set_attribute(k, v),
            None => el.remove_attribute(k),
        };
    };
    set("role", aria(n.role));
    set("aria-label", (!n.label.is_empty()).then_some(n.label.as_str()));
    if n.role == Role::Label || n.role == Role::Heading {
        el.set_text_content(Some(&n.label));
    }
    let value = n.value.as_deref();
    match n.role {
        Role::Slider | Role::ProgressBar => set("aria-valuetext", value),
        Role::TextInput | Role::MultilineTextInput | Role::ComboBox => set("aria-description", value),
        _ => set("aria-description", None),
    }
    let checked = n.checked.map(|c| if c { "true" } else { "false" });
    match n.role {
        Role::ToggleButton => set("aria-pressed", checked),
        Role::Tab => set("aria-selected", checked),
        Role::Switch | Role::CheckBox | Role::RadioButton => set("aria-checked", checked),
        _ => {}
    }
    set("aria-expanded", n.expanded.map(|e| if e { "true" } else { "false" }));
    set("aria-disabled", n.disabled.then_some("true"));
    set("aria-live", n.live.then_some("polite"));
    let focusable = actionable(n.role) && !n.disabled;
    set("tabindex", focusable.then_some("0"));
}
