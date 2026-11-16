//! # morfo
//!
//! Reusable UI components whose every visual property is driven by an
//! analytic spring, so anything can fluidly morph into anything else, plus a
//! usage-learning engine that lets an interface reshape itself per user.
//!
//! The crate is renderer-agnostic and has **zero dependencies**: each frame
//! it produces a flat [`DrawList`] that any backend (CPU, GPU, canvas) can
//! paint. When nothing is moving, [`Frame::animating`] is `false` and the host
//! can stop drawing entirely.
//!
//! ```
//! use morfo::{Ui, Theme, Vec2};
//!
//! let mut ui = Ui::new(Theme::dark());
//! let mut dark = true;
//! ui.begin(0.0, Vec2::new(800.0, 600.0));
//! ui.heading("Hello");
//! if ui.button("save", "Save").clicked { /* ... */ }
//! ui.toggle("dark", &mut dark);
//! let frame = ui.end();
//! // paint ui.draw_list() with any backend; redraw again only if frame.animating
//! # let _ = frame;
//! ```
#![forbid(unsafe_code)]

pub mod adapt;
pub mod anim;
pub mod draw;
pub mod geom;
pub mod id;
mod input;
mod morph;
mod overlay;
mod select;
pub mod theme;
pub mod ui;
mod widgets;

pub use adapt::{Profile, Prominence};
pub use anim::{Animatable, Motion, Spring, SpringKind, SpringStep};
pub use draw::{DrawCmd, DrawList};
pub use geom::{Color, Rect, Vec2};
pub use id::Id;
pub use theme::Theme;
pub use input::TextEdit;
pub use ui::{ApproxText, Frame, Key, Response, TextMeasure, Ui};
pub use widgets::{Size, Variant};
