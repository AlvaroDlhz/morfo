//! Window chrome: an application title bar that replaces the one drawn by
//! the window manager.
//!
//! morfo does not own the window, so the bar does not move or close it by
//! itself: it reports what the user asked for ([`TitleBar`]) and the host
//! does it (with winit: `drag_window`, `set_minimized`, `set_maximized`,
//! `drag_resize_window`, closing). Create the window without decorations.

use crate::anim::SpringKind::Snappy;
use crate::buttons::{draw_icon, Icon};
use crate::geom::{Color, Rect, Vec2};
use crate::theme::{ACCENT, BORDER, SURFACE, TEXT};
use crate::ui::{Dir, Node, Ui, LINE_HEIGHT};

/// A window border or corner, for resizing an undecorated window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResizeEdge {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

/// What the user asked of the window this frame. The host carries it out.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TitleBar {
    /// Pressed on an empty part of the bar: start moving the window now,
    /// while the button is still down.
    pub drag: bool,
    pub minimize: bool,
    /// The maximize button, or a double click on the bar.
    pub toggle_maximize: bool,
    pub close: bool,
    /// Pressed on a border: start resizing the window from it.
    pub resize: Option<ResizeEdge>,
    /// Border under the pointer, to show a resize cursor.
    pub hover_edge: Option<ResizeEdge>,
}

/// Width of the invisible strip along the window border that resizes it.
const EDGE: f32 = 5.0;
/// Corners are easier to catch.
const CORNER: f32 = 12.0;
/// Two presses on the bar within this many seconds are a double click.
const DOUBLE_CLICK: f64 = 0.4;

fn edge_at(p: Vec2, vp: Vec2) -> Option<ResizeEdge> {
    let (left, right) = (p.x < EDGE, p.x > vp.x - EDGE);
    let (top, bottom) = (p.y < EDGE, p.y > vp.y - EDGE);
    let (near_left, near_right) = (p.x < CORNER, p.x > vp.x - CORNER);
    let (near_top, near_bottom) = (p.y < CORNER, p.y > vp.y - CORNER);
    use ResizeEdge::*;
    Some(match () {
        _ if (top && near_left) || (left && near_top) => NorthWest,
        _ if (top && near_right) || (right && near_top) => NorthEast,
        _ if (bottom && near_left) || (left && near_bottom) => SouthWest,
        _ if (bottom && near_right) || (right && near_bottom) => SouthEast,
        _ if top => North,
        _ if bottom => South,
        _ if left => West,
        _ if right => East,
        _ => return None,
    })
}

impl Ui {
    /// The application's own title bar, across the top of the window: the
    /// title on the left, your content after it (`f`, laid out as a row),
    /// and minimize, maximize/restore and close on the right. Pressing an
    /// empty part of it moves the window, a double click maximizes, and the
    /// window borders resize it (unless `maximized`).
    ///
    /// Call it first in the frame: it moves the rest of the layout down.
    pub fn title_bar(&mut self, key: &str, title: &str, maximized: bool, f: impl FnOnce(&mut Ui)) -> TitleBar {
        let id = self.declare(key);
        let vp = self.viewport();
        let theme = self.theme;
        let h = (theme.control * 1.05).round();
        let bar = Rect::new(0.0, 0.0, vp.x, h);
        let mut out = TitleBar::default();
        let now = self.now;
        let pal = self.pal;

        // Borders first, so a press on them wins over anything underneath.
        if !maximized {
            if let Some(p) = self.input.pointer {
                out.hover_edge = edge_at(p, vp);
                if out.hover_edge.is_some() && self.input.pressed && self.active.is_none() {
                    out.resize = out.hover_edge;
                    // Claim the press so nothing else reacts to it.
                    self.active = Some(id.child("edge"));
                }
            }
        }

        self.draw.rect(bar, 0.0, pal[SURFACE]);
        self.draw.rect(Rect::new(0.0, h - 1.0, vp.x, 1.0), 0.0, pal[BORDER]);
        // A small mark in the accent color, then the title.
        let font = theme.font * 0.95;
        let mark = Rect::new(theme.pad, (h - 10.0) * 0.5, 10.0, 10.0);
        self.draw.rect(mark, 5.0, pal[ACCENT]);
        let title_x = mark.right() + 10.0;
        self.draw.add_text(Vec2::new(title_x, (h - font * LINE_HEIGHT) * 0.5), font, pal[TEXT], title);

        // Window buttons.
        let bw = (h * 1.15).round();
        let resize_icon = if maximized { Icon::Restore } else { Icon::Maximize };
        let snappy = self.steps[Snappy as usize];
        for (i, (icon, name)) in [(Icon::Minus, "minimize"), (resize_icon, "maximize"), (Icon::Close, "close")].into_iter().enumerate() {
            let r = Rect::new(vp.x - bw * (3 - i) as f32, 0.0, bw, h - 1.0);
            let cid = id.child(name);
            let hit = self.hit(cid, r);
            let spoken = match name {
                "minimize" => self.locale.minimize,
                "maximize" => self.locale.maximize,
                _ => self.locale.close,
            };
            self.describe(cid, crate::a11y::Role::Button, spoken, r, None, None);
            let n = self.nodes.entry(cid).or_insert_with(Node::new);
            let fresh = n.touch(self.frame);
            n.rect.snap(r);
            self.animating |= n.hover.drive(hit.hovered as u8 as f32, fresh, &snappy) | n.press.drive(hit.held as u8 as f32, fresh, &snappy);
            let (hover, press) = (n.hover.get().clamp(0.0, 1.0), n.press.get().clamp(0.0, 1.0));
            let is_close = name == "close";
            let fill = if is_close {
                Color::hex(0xe5484d).with_alpha(hover * (1.0 - 0.15 * press))
            } else {
                pal[TEXT].with_alpha((0.08 + 0.06 * press) * hover)
            };
            self.draw.rect(r, 0.0, fill);
            let ink = if is_close { pal[TEXT].mix(Color::WHITE, hover) } else { pal[TEXT] };
            draw_icon(&mut self.draw, icon, r.center(), (h * 0.3).round(), ink.with_alpha(0.85));
            if hit.clicked {
                match name {
                    "minimize" => out.minimize = true,
                    "maximize" => out.toggle_maximize = true,
                    _ => out.close = true,
                }
            }
        }

        // Your content, as a row after the title, vertically centered.
        let content_id = id.child("content");
        let prev = self.sizes.get(&content_id).copied().unwrap_or(Vec2::ZERO);
        let x0 = title_x + self.text.width(title, font) + theme.gap * 2.0;
        let origin = Vec2::new(x0, ((h - prev.y) * 0.5).max(0.0));
        let saved = self.scope;
        self.scope = id;
        let gap = theme.gap;
        // Up to the window buttons, so `ui.fill()` puts content at the right.
        self.next_width = Some((vp.x - bw * 3.0 - gap - x0).max(0.0));
        self.group(content_id, Dir::Row, Vec2::ZERO, gap, Some(origin), |ui| ui.within(id, f));
        self.scope = saved;

        // A press on an empty part of the bar moves the window; two in a
        // row maximize it.
        if self.input.pressed && self.active.is_none() && self.pointer_in(bar) {
            let n = self.nodes.entry(id).or_insert_with(Node::new);
            n.touch(self.frame);
            if now - n.stamp < DOUBLE_CLICK {
                out.toggle_maximize = true;
                n.stamp = f64::NEG_INFINITY;
            } else {
                out.drag = true;
                n.stamp = now;
            }
        }

        // Everything else is laid out below the bar.
        self.top_inset = h;
        if let Some(root) = self.layouts.first_mut() {
            root.origin.y += h;
        }
        out
    }

    /// Height taken at the top of the window by [`title_bar`](Self::title_bar)
    /// (0 without one).
    pub fn top_inset(&self) -> f32 {
        self.top_inset
    }
}
