//! Screen readers: morfo's description of each frame as an AccessKit tree,
//! and the readers' requests back to morfo.

use std::collections::{HashMap, HashSet};

use accesskit::{Action, Node, NodeId, Toggled, TreeId, TreeInfo, TreeUpdate};
use accesskit_winit::WindowEvent as AccessEvent;
use morfo::{AccessNode, Id, Role, Ui};

/// The id of the window itself in the tree.
pub const ROOT: NodeId = NodeId(u64::MAX);

/// The AccessKit role for a morfo role.
pub fn role(role: Role) -> accesskit::Role {
    use accesskit::Role as R;
    match role {
        Role::Button => R::Button,
        Role::ToggleButton => R::Button,
        Role::Switch => R::Switch,
        Role::CheckBox => R::CheckBox,
        Role::RadioButton => R::RadioButton,
        Role::Slider => R::Slider,
        Role::TextInput => R::TextInput,
        Role::MultilineTextInput => R::MultilineTextInput,
        Role::ComboBox => R::ComboBox,
        Role::ListItem => R::ListItem,
        Role::MenuItem => R::MenuItem,
        Role::Tab => R::Tab,
        Role::Link => R::Link,
        Role::Label => R::Label,
        Role::Heading => R::Heading,
        Role::Table => R::Table,
        Role::Row => R::Row,
        Role::Image => R::Image,
        Role::Group => R::Group,
        Role::Dialog => R::Dialog,
        Role::ScrollView => R::ScrollView,
        Role::TabList => R::TabList,
        Role::Menu => R::Menu,
        Role::Tree => R::Tree,
        Role::TreeItem => R::TreeItem,
        Role::ProgressBar => R::ProgressIndicator,
        Role::Status => R::Status,
        _ => R::GenericContainer,
    }
}

/// Whether people act on an element of this role (rather than read it).
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
            | Role::ProgressBar
            | Role::Status
    )
}

/// The frame `nodes` describe, as an AccessKit tree under a window titled
/// `title`, with the keyboard on `focus`. `scale` turns logical pixels into
/// the window's physical ones.
pub fn tree(nodes: &[AccessNode], focus: Option<Id>, scale: f64, title: &str) -> TreeUpdate {
    let mut seen = HashSet::with_capacity(nodes.len());
    let unique: Vec<&AccessNode> = nodes.iter().filter(|n| seen.insert(n.id)).collect();
    // Each element under its container, in reading order; what has no
    // known container hangs from the window.
    let mut children: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for n in &unique {
        let parent = n.parent.filter(|p| seen.contains(p)).map_or(ROOT, |p| NodeId(p.0));
        children.entry(parent).or_default().push(NodeId(n.id.0));
    }
    let mut out = Vec::with_capacity(unique.len() + 1);
    for n in unique {
        let mut node = Node::new(role(n.role));
        node.set_label(n.label.as_str());
        if let Some(v) = &n.value {
            node.set_value(v.as_str());
        }
        if let Some(c) = n.checked {
            node.set_toggled(if c { Toggled::True } else { Toggled::False });
        }
        if n.disabled {
            node.set_disabled();
        }
        let r = n.rect;
        node.set_bounds(accesskit::Rect {
            x0: r.x as f64 * scale,
            y0: r.y as f64 * scale,
            x1: r.right() as f64 * scale,
            y1: r.bottom() as f64 * scale,
        });
        if let Some(open) = n.expanded {
            node.set_expanded(open);
        }
        if n.live {
            node.set_live(accesskit::Live::Polite);
        }
        if actionable(n.role) && !n.disabled {
            node.add_action(Action::Focus);
            node.add_action(Action::Click);
        }
        if let Some(kids) = children.remove(&NodeId(n.id.0)) {
            node.set_children(kids);
        }
        out.push((NodeId(n.id.0), node));
    }
    let mut root = Node::new(accesskit::Role::Window);
    root.set_label(title);
    root.set_children(children.remove(&ROOT).unwrap_or_default());
    out.push((ROOT, root));
    let focus = focus.map(|f| NodeId(f.0)).filter(|f| seen.contains(&Id(f.0))).unwrap_or(ROOT);
    let mut info = TreeInfo::new(ROOT);
    info.toolkit_name = Some("morfo".into());
    TreeUpdate { nodes: out, tree: Some(info), tree_id: TreeId::ROOT, focus }
}

/// Carries a screen reader's request out on `ui`.
pub fn handle(ui: &mut Ui, event: AccessEvent) {
    match event {
        AccessEvent::InitialTreeRequested => ui.set_accessibility(true),
        AccessEvent::AccessibilityDeactivated => ui.set_accessibility(false),
        AccessEvent::ActionRequested(req) => match req.action {
            Action::Click => ui.access_click(Id(req.target_node.0)),
            Action::Focus => ui.access_focus(Id(req.target_node.0)),
            _ => {}
        },
    }
}
