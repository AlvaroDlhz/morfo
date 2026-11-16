//! The interactive documentation, running in a browser: the same interface
//! as the desktop app (morfo-demo's `build`), painted on a canvas.
//!
//! Build with `web/build.sh`, then serve the `web/` folder.

use morfo::Theme;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let mut state = morfo_demo::State { native_window: false, ..Default::default() };
    morfo_web::run("morfo", Theme::dark(), move |ui| morfo_demo::build(ui, &mut state))
}
