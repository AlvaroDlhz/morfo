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

mod a11y;
pub mod adapt;
pub mod anim;
pub mod bidi;
mod buttons;
mod charts;
mod chrome;
mod compose;
mod controls;
mod course;
mod data;
pub mod draw;
mod edit;
mod forms;
pub mod geom;
mod habits;
pub mod id;
mod image;
mod input;
mod layout;
mod lazy;
mod locale;
mod morph;
mod navigate;
mod overlay;
mod panes;
mod path;
mod pick;
mod scrolling;
mod select;
mod session;
mod status;
pub mod testing;
mod text;
pub mod theme;
mod transition;
pub mod ui;
mod widgets;

pub use a11y::{AccessNode, Role};
pub use adapt::{Adaptation, Profile, Prominence, Reason};
pub use anim::{Animatable, Motion, MotionPreference, Spring, SpringKind, SpringStep};
pub use buttons::Icon;
pub use chrome::{ResizeEdge, TitleBar};
pub use draw::{DrawCmd, DrawList, ImageData, ImageId, TextStyle};
pub use forms::{Date, Shortcut};
pub use geom::{Color, Hsv, Rect, Vec2};
pub use habits::Habit;
pub use id::Id;
pub use image::Fit;
pub use input::TextEdit;
pub use layout::Align;
pub use locale::{DateOrder, KeyNames, Locale};
pub use navigate::Side;
pub use path::Path;
pub use scrolling::Reveal;
pub use text::Span;
pub use theme::{contrast, on_color, ContrastIssue, Theme};
pub use transition::Transition;
pub use ui::{ApproxText, Direction, Frame, Key, Modifiers, Response, TextMeasure, Ui};
pub use widgets::{Size, Variant};
