//! A task list: a text field, check boxes, a list that reorders by
//! dragging, and the Spanish locale. The usage profile is kept between
//! runs (`app_id`), so the interface remembers what you use.
//!
//! `cargo run -p morfo-winit --example tasks`

use morfo::{Locale, Size, Variant};
use morfo_winit::Options;

struct Task {
    text: String,
    done: bool,
}

fn main() -> Result<(), morfo_winit::Error> {
    let mut tasks: Vec<Task> =
        ["Escribir el informe", "Revisar la propuesta", "Llamar a Ana"].into_iter().map(|t| Task { text: t.to_string(), done: false }).collect();
    let mut draft = String::new();
    let options = Options { locale: Locale::SPANISH, app_id: Some("morfo-ejemplo-tareas".into()), ..Options::new("Tareas") };
    morfo_winit::run(options, move |ui, _| {
        ui.heading("Tareas");
        ui.row(|ui| {
            let entry = ui.text_field("nueva", &mut draft, "Nueva tarea…", 320.0);
            let add = ui.button_with("anadir", "Añadir", Variant::Solid, Size::Md).clicked;
            if (entry.submitted || add) && !draft.trim().is_empty() && !tasks.iter().any(|t| t.text == draft.trim()) {
                tasks.push(Task { text: draft.trim().to_string(), done: false });
                draft.clear();
                ui.request_focus("nueva");
            }
        });
        let pending = tasks.iter().filter(|t| !t.done).count();
        ui.muted(&format!("{pending} pendientes de {}", tasks.len()));
        // Each task's key is its text: a task keeps its identity when it moves.
        for task in &mut tasks {
            ui.checkbox(&task.text, &task.text, &mut task.done);
        }
        ui.separator();
        ui.muted("Arrastra para cambiar el orden:");
        ui.reorderable("orden", &mut tasks, |t| t.text.as_str());
        if tasks.iter().any(|t| t.done) && ui.button_with("limpiar", "Quitar las hechas", Variant::Ghost, Size::Sm).clicked {
            tasks.retain(|t| !t.done);
        }
    })
}
