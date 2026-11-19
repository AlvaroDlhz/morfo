//! The smallest morfo application: a counter in a window.
//!
//! `cargo run -p morfo-winit --example hello`

fn main() -> Result<(), morfo_winit::Error> {
    let mut clicks = 0;
    morfo_winit::run(morfo_winit::Options::new("Hello, morfo"), move |ui, _| {
        ui.heading("Hello, morfo");
        ui.muted("Every change is a motion: press the button and watch the text.");
        if ui.button("more", "Click me").clicked {
            clicks += 1;
        }
        ui.label(&format!("Clicked {clicks} times"));
    })
}
