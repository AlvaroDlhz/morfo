//! Doing what an application's own title bar ([`Ui::title_bar`]) asks of
//! the window: move it, resize it, minimize, maximize or close it.
//!
//! [`Ui::title_bar`]: morfo::Ui::title_bar

use morfo::{ResizeEdge, TitleBar, Ui};
use winit::window::{CursorIcon, ResizeDirection, Window};

/// The resize direction for a window border.
pub fn direction(e: ResizeEdge) -> ResizeDirection {
    match e {
        ResizeEdge::North => ResizeDirection::North,
        ResizeEdge::South => ResizeDirection::South,
        ResizeEdge::East => ResizeDirection::East,
        ResizeEdge::West => ResizeDirection::West,
        ResizeEdge::NorthEast => ResizeDirection::NorthEast,
        ResizeEdge::NorthWest => ResizeDirection::NorthWest,
        ResizeEdge::SouthEast => ResizeDirection::SouthEast,
        ResizeEdge::SouthWest => ResizeDirection::SouthWest,
    }
}

/// The pointer's shape over a border (or none).
pub fn cursor(e: Option<ResizeEdge>) -> CursorIcon {
    match e {
        None => CursorIcon::Default,
        Some(ResizeEdge::North | ResizeEdge::South) => CursorIcon::NsResize,
        Some(ResizeEdge::East | ResizeEdge::West) => CursorIcon::EwResize,
        Some(ResizeEdge::NorthEast | ResizeEdge::SouthWest) => CursorIcon::NeswResize,
        Some(ResizeEdge::NorthWest | ResizeEdge::SouthEast) => CursorIcon::NwseResize,
    }
}

/// Remembers the pointer's shape so it is only changed when needed.
#[derive(Default)]
pub struct Chrome {
    edge: Option<ResizeEdge>,
}

impl Chrome {
    /// Carries out `bar` on `window`. Returns `true` when it asked to close.
    pub fn apply(&mut self, ui: &mut Ui, window: &Window, bar: TitleBar) -> bool {
        if bar.hover_edge != self.edge {
            window.set_cursor(cursor(bar.hover_edge));
            self.edge = bar.hover_edge;
        }
        if bar.drag || bar.resize.is_some() {
            let _ = match bar.resize {
                Some(edge) => window.drag_resize_window(direction(edge)),
                None => window.drag_window(),
            };
            // The window manager takes the pointer from here: the release may
            // never arrive.
            ui.pointer_released();
            ui.pointer_left();
        }
        if bar.minimize {
            window.set_minimized(true);
        }
        if bar.toggle_maximize {
            window.set_maximized(!window.is_maximized());
        }
        bar.close
    }
}
