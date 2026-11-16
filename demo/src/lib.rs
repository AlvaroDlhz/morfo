//! La documentación interactiva de morfo, hecha con morfo: cada página
//! explica un grupo de componentes, lo muestra funcionando y enseña el código
//! que lo produce. La comparten la ventana (`docs`) y el simulador sin
//! ventana (`snapshot`).

use morfo::{Color, Profile, Prominence, Size, Theme, Ui, Variant, Vec2};

/// Tamaño lógico recomendado de la ventana.
pub const WINDOW: Vec2 = Vec2::new(990.0, 780.0);

pub const PAGES: [&str; 10] =
    ["Inicio", "Botones", "Controles", "Selección", "Cajas", "Formas", "Flotantes", "Entrada", "Adaptativo", "Mutación"];

pub const ACTIONS: [(&str, &str); 5] =
    [("nuevo", "Nuevo"), ("abrir", "Abrir"), ("guardar", "Guardar"), ("exportar", "Exportar"), ("compartir", "Compartir")];

pub const OPTIONS: [(&str, &str); 3] = [("renombrar", "Renombrar"), ("duplicar", "Duplicar"), ("archivar", "Archivar")];

pub const FILE_MENU: [(&str, &str); 6] = [
    ("nuevo", "Nuevo"),
    ("abrir", "Abrir…"),
    ("guardar", "Guardar"),
    ("exportar", "Exportar PDF"),
    ("imprimir", "Imprimir"),
    ("propiedades", "Propiedades"),
];

const SHAPES: [(Vec2, f32, u32); 4] = [
    (Vec2::new(96.0, 96.0), 48.0, 0x7c6cff),
    (Vec2::new(300.0, 72.0), 20.0, 0x22c3a6),
    (Vec2::new(150.0, 150.0), 34.0, 0xff7a59),
    (Vec2::new(420.0, 40.0), 20.0, 0xf5b83d),
];

const STYLES: [(&str, Variant, Size); 4] = [
    ("Solid · Md", Variant::Solid, Size::Md),
    ("Soft · Lg", Variant::Soft, Size::Lg),
    ("Outline · Sm", Variant::Outline, Size::Sm),
    ("Ghost · Md", Variant::Ghost, Size::Md),
];

pub struct State {
    pub tab: usize,
    pub light: bool,
    pub level: f32,
    pub range: usize,
    pub details: bool,
    pub shape: usize,
    pub style: usize,
    pub clicks: u32,
    pub last: &'static str,
    pub news: bool,
    pub autosave: bool,
    pub plan: usize,
    pub tags: [bool; 3],
    pub seats: i32,
    pub faq: Option<usize>,
    pub dialog: bool,
    pub option: &'static str,
    pub toasts: u32,
    pub name: String,
    pub city: String,
    pub file: &'static str,
    pub search_open: bool,
    pub query: String,
    pub form: usize,
    pub mut_check: bool,
    pub mut_text: String,
    pub mut_toggle: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            tab: 0,
            light: false,
            level: 0.4,
            range: 1,
            details: false,
            shape: 0,
            style: 0,
            clicks: 0,
            last: "ninguna",
            news: true,
            autosave: false,
            plan: 1,
            tags: [true, false, false],
            seats: 3,
            faq: Some(0),
            dialog: false,
            option: "ninguna",
            toasts: 0,
            name: String::new(),
            city: String::new(),
            file: "ninguna",
            search_open: false,
            query: String::new(),
            form: 0,
            mut_check: true,
            mut_text: String::new(),
            mut_toggle: true,
        }
    }
}

/// Otra persona: perfil vacío, la interfaz vuelve a su forma inicial.
fn forget(ui: &mut Ui, s: &mut State) {
    let mut fresh = Profile::new();
    fresh.half_life = ui.profile().half_life;
    ui.set_profile(fresh);
    s.last = "ninguna";
    s.file = "ninguna";
}

/// Una página: aparece y desaparece con `reveal` y su contenido hace scroll.
fn page(ui: &mut Ui, key: &str, scroll: &str, open: bool, f: impl FnOnce(&mut Ui)) {
    let height = (ui.viewport().y - 200.0).max(120.0);
    ui.reveal(key, open, |ui| ui.scroll(scroll, height, f));
}

pub fn build(ui: &mut Ui, s: &mut State) {
    ui.heading("morfo");
    ui.muted("Documentación interactiva. Todo lo que ves aquí está hecho con la propia librería.");
    ui.space(4.0);
    ui.segmented("tab", &PAGES, &mut s.tab);
    ui.space(4.0);

    page(ui, "page-inicio", "scroll-inicio", s.tab == 0, |ui| {
        ui.title("Qué es");
        ui.muted("Componentes donde cada cambio es una transición continua y la interfaz se adapta a quien la usa.");
        ui.muted("Cada frame declaras lo que debe existir; morfo lleva cada elemento hasta ahí con muelles.");
        ui.code(&[
            "let mut ui = Ui::new(Theme::dark());",
            "",
            "// cada frame",
            "ui.begin(segundos, Vec2::new(ancho, alto));",
            "ui.heading(\"Hola\");",
            "if ui.button(\"guardar\", \"Guardar\").clicked { guardar(); }",
            "let frame = ui.end();",
            "",
            "renderer.render(ui.draw_list(), &mut pixmap, escala, ui.background());",
            "if frame.animating { pedir_otro_frame(); }",
            "else if let Some(s) = frame.wake_in { despertar_en(s); }",
        ]);
        ui.title("Cuatro reglas");
        ui.muted("1. La clave de un elemento (\"guardar\") es su identidad: si cambia de sitio o de forma, hace morph.");
        ui.muted("2. Si frame.animating es false no hay nada que dibujar: el programa duerme hasta el próximo evento.");
        ui.muted("3. frame.wake_in avisa de cuándo hará falta un frame sin que haya entrada (un aviso que caduca, el cursor).");
        ui.muted("4. La entrada se pasa con pointer_moved/pressed/released, scroll_wheel, key_pressed y text_input.");
    });

    page(ui, "page-botones", "scroll-botones", s.tab == 1, |ui| {
        ui.title("Botones");
        ui.muted("Cuatro variantes y tres tamaños. Devuelven una Response con clicked, hovered y held.");
        ui.card("card-botones", |ui| {
            ui.row(|ui| {
                ui.button_with("v-solid", "Solid", Variant::Solid, Size::Md);
                ui.button_with("v-soft", "Soft", Variant::Soft, Size::Md);
                ui.button_with("v-outline", "Outline", Variant::Outline, Size::Md);
                ui.button_with("v-ghost", "Ghost", Variant::Ghost, Size::Md);
            });
            ui.row(|ui| {
                ui.button_with("s-sm", "Sm", Variant::Soft, Size::Sm);
                ui.button_with("s-md", "Md", Variant::Soft, Size::Md);
                ui.button_with("s-lg", "Lg", Variant::Soft, Size::Lg);
            });
            ui.separator();
            ui.row(|ui| {
                let (name, variant, size) = STYLES[s.style];
                if ui.button_with("camaleon", name, variant, size).clicked {
                    s.style = (s.style + 1) % STYLES.len();
                    s.clicks += 1;
                }
                ui.muted(&format!("Púlsalo: el mismo botón cambia de variante y tamaño. Clics: {}", s.clicks));
            });
        });
        ui.code(&[
            "if ui.button(\"guardar\", \"Guardar\").clicked { guardar(); }",
            "ui.button_with(\"borrar\", \"Borrar\", Variant::Outline, Size::Sm);",
            "",
            "// La misma clave con otra variante o tamaño: el botón se transforma.",
            "let (nombre, variante, tamano) = ESTILOS[i];",
            "if ui.button_with(\"camaleon\", nombre, variante, tamano).clicked { i = (i + 1) % 4; }",
        ]);
    });

    page(ui, "page-controles", "scroll-controles", s.tab == 2, |ui| {
        ui.title("Controles");
        ui.muted("Reciben el valor por referencia y devuelven true cuando la persona lo cambia.");
        ui.card("card-controles", |ui| {
            ui.row(|ui| {
                if ui.toggle("light", &mut s.light) {
                    ui.set_theme(if s.light { Theme::light() } else { Theme::dark() });
                }
                ui.label("Tema claro");
                ui.muted("(set_theme: todos los colores hacen morph)");
            });
            ui.row(|ui| {
                ui.slider("level", &mut s.level, 260.0);
                ui.muted(&format!("{:.0} %", s.level * 100.0));
            });
            let w = ui.available_width();
            ui.progress(s.level, w);
            ui.space(2.0);
            ui.segmented("range", &["Día", "Semana", "Mes"], &mut s.range);
        });
        ui.code(&[
            "if ui.toggle(\"claro\", &mut claro) {",
            "    ui.set_theme(if claro { Theme::light() } else { Theme::dark() });",
            "}",
            "ui.slider(\"nivel\", &mut nivel, 260.0);   // nivel: f32 en 0..=1",
            "ui.progress(nivel, ui.available_width());",
            "ui.segmented(\"rango\", &[\"Día\", \"Semana\", \"Mes\"], &mut rango);",
        ]);
    });

    page(ui, "page-seleccion", "scroll-seleccion", s.tab == 3, |ui| {
        ui.title("Selección");
        ui.muted("Casillas que se dibujan trazo a trazo, opciones excluyentes, etiquetas, un contador que rueda y un acordeón.");
        ui.card("card-seleccion", |ui| {
            ui.row(|ui| {
                ui.checkbox("news", "Recibir novedades", &mut s.news);
                ui.checkbox("autosave", "Guardar automáticamente", &mut s.autosave);
            });
            ui.row(|ui| {
                ui.muted("Plan:");
                ui.radio("plan-basico", "Básico", &mut s.plan, 0);
                ui.radio("plan-pro", "Pro", &mut s.plan, 1);
                ui.radio("plan-equipo", "Equipo", &mut s.plan, 2);
            });
            ui.row(|ui| {
                ui.muted("Etiquetas:");
                ui.chip("tag-rust", "Rust", &mut s.tags[0]);
                ui.chip("tag-diseno", "Diseño", &mut s.tags[1]);
                ui.chip("tag-web", "Web", &mut s.tags[2]);
            });
            ui.row(|ui| {
                ui.stepper("seats", &mut s.seats, 1, 12);
                ui.muted("asientos (entre 1 y 12)");
            });
            ui.separator();
            ui.accordion("faq", &["¿Qué es un morph?", "¿Y si cambio el tema?", "¿Cuánto cuesta en reposo?"], &mut s.faq, |ui, i| {
                ui.muted(match i {
                    0 => "El paso continuo de una forma a otra: posición, tamaño, radio y color viajan juntos.",
                    1 => "Todos los colores viajan a la vez al nuevo tema; nada salta.",
                    _ => "Nada: si no hay animación ni entrada, no se dibuja ni se calcula.",
                });
            });
        });
        ui.code(&[
            "ui.checkbox(\"novedades\", \"Recibir novedades\", &mut novedades);",
            "ui.radio(\"plan-pro\", \"Pro\", &mut plan, 1);      // elegido si plan == 1",
            "ui.chip(\"rust\", \"Rust\", &mut etiquetas[0]);",
            "ui.stepper(\"asientos\", &mut asientos, 1, 12);",
            "ui.accordion(\"dudas\", &[\"Primera\", \"Segunda\"], &mut abierta, |ui, i| {",
            "    ui.muted(RESPUESTAS[i]);                  // solo se construye la sección abierta",
            "});",
        ]);
    });

    page(ui, "page-contenedores", "scroll-contenedores", s.tab == 4, |ui| {
        ui.title("Cajas");
        ui.muted("row y column colocan; card agrupa y su altura sigue al contenido; reveal abre y cierra; scroll desplaza.");
        ui.card("card-contenedores", |ui| {
            ui.row(|ui| {
                let label = if s.details { "Ocultar detalles" } else { "Ver detalles" };
                if ui.button_with("details", label, Variant::Outline, Size::Md).clicked {
                    s.details = !s.details;
                }
                ui.muted("La tarjeta y el código de abajo se recolocan solos.");
            });
            ui.reveal("details-body", s.details, |ui| {
                ui.separator();
                ui.label("Contenido revelado");
                ui.muted("Mientras está cerrado no cuesta nada: ni siquiera se construye.");
            });
        });
        ui.code(&[
            "ui.card(\"ajustes\", |ui| {",
            "    ui.row(|ui| {",
            "        if ui.button(\"mas\", \"Ver detalles\").clicked { abierto = !abierto; }",
            "        ui.muted(\"Texto a la derecha del botón\");",
            "    });",
            "    ui.reveal(\"detalles\", abierto, |ui| {",
            "        ui.label(\"Contenido revelado\");",
            "    });",
            "});",
            "",
            "// Contenido propio que debe animar también su salida",
            "let p = ui.presence(\"aviso\", visible);   // 0..1; sigue declarándolo mientras p > 0",
        ]);
    });

    page(ui, "page-formas", "scroll-formas", s.tab == 5, |ui| {
        ui.title("Formas");
        ui.muted("surface es la pieza básica: la misma clave con otro tamaño, radio o color fluye hasta la nueva forma.");
        ui.card("card-formas", |ui| {
            let (size, radius, color) = SHAPES[s.shape];
            ui.surface("blob", size, radius, Color::hex(color));
            ui.row(|ui| {
                if ui.button("morph", "Transformar").clicked {
                    s.shape = (s.shape + 1) % SHAPES.len();
                }
                ui.muted("Pulsa varias veces seguidas: se interrumpe sin saltos.");
            });
        });
        ui.code(&[
            "// Misma identidad, otra forma: tamaño, radio y color fluyen juntos.",
            "let (tamano, radio, color) = FORMAS[i];",
            "ui.surface(\"forma\", tamano, radio, color);",
            "if ui.button(\"morph\", \"Transformar\").clicked { i = (i + 1) % FORMAS.len(); }",
        ]);
    });

    page(ui, "page-flotantes", "scroll-flotantes", s.tab == 6, |ui| {
        ui.title("Flotantes");
        ui.muted("Se dibujan por encima, bloquean lo que tapan y animan también su salida. Escape o un clic fuera los cierra.");
        ui.card("card-flotantes", |ui| {
            ui.row(|ui| {
                if ui.button("dlg-open", "Abrir diálogo").clicked {
                    s.dialog = true;
                }
                ui.tooltip("dlg-open", "El diálogo nace de este botón y vuelve a él");
                if let Some(i) = ui.menu("opciones", "Opciones", &OPTIONS) {
                    s.option = OPTIONS[i].1;
                }
                if ui.button_with("toast-btn", "Lanzar aviso", Variant::Soft, Size::Md).clicked {
                    s.toasts += 1;
                    ui.toast(&format!("Aviso número {}", s.toasts));
                }
                ui.tooltip("toast-btn", "Se van solos, o al pulsarlos");
            });
            ui.row(|ui| {
                ui.muted("Opción elegida:");
                ui.label(s.option);
                ui.muted("· Deja el ratón quieto sobre un botón para ver su ayuda.");
            });
        });
        ui.dialog("dlg", "dlg-open", &mut s.dialog, 430.0, |ui| {
            ui.title("¿Guardar los cambios?");
            ui.muted("Este panel ha crecido desde el botón que lo abrió.");
            ui.muted("Lo de debajo no responde mientras esté abierto.");
            ui.space(4.0);
            ui.row(|ui| {
                if ui.button("dlg-ok", "Guardar").clicked {
                    ui.toast("Cambios guardados");
                    ui.close();
                }
                if ui.button_with("dlg-cancel", "Cancelar", Variant::Ghost, Size::Md).clicked {
                    ui.close();
                }
            });
        });
        ui.code(&[
            "if ui.button(\"abrir\", \"Abrir diálogo\").clicked { abierto = true; }",
            "ui.tooltip(\"abrir\", \"Ayuda al dejar el ratón encima\");",
            "",
            "// Crece desde el elemento \"abrir\" y vuelve a él al cerrarse.",
            "ui.dialog(\"dialogo\", \"abrir\", &mut abierto, 430.0, |ui| {",
            "    ui.title(\"¿Guardar los cambios?\");",
            "    if ui.button(\"ok\", \"Guardar\").clicked { ui.toast(\"Cambios guardados\"); ui.close(); }",
            "});",
            "",
            "let opciones = [(\"renombrar\", \"Renombrar\"), (\"duplicar\", \"Duplicar\")];",
            "if let Some(i) = ui.menu(\"opciones\", \"Opciones\", &opciones) { aplicar(i); }",
        ]);
    });

    page(ui, "page-entrada", "scroll-entrada", s.tab == 7, |ui| {
        ui.title("Entrada");
        ui.muted("Campos de texto con foco, cursor que se desliza y Tab para saltar; y áreas con scroll suave.");
        ui.card("card-entrada", |ui| {
            ui.row(|ui| {
                ui.text_field("name", &mut s.name, "Tu nombre", 240.0);
                ui.text_field("city", &mut s.city, "Tu ciudad", 200.0);
            });
            let greeting = match (s.name.is_empty(), s.city.is_empty()) {
                (true, _) => "Haz clic en un campo y escribe. Tab pasa al siguiente; Escape lo suelta.".to_string(),
                (false, true) => format!("Hola, {}.", s.name),
                (false, false) => format!("Hola, {}. ¿Qué tal por {}?", s.name, s.city),
            };
            ui.label(&greeting);
            ui.separator();
            ui.muted("Lista con scroll (usa la rueda aquí dentro): 60 filas, solo se dibujan las visibles.");
            ui.scroll("lista", 132.0, |ui| {
                for i in 1..=60 {
                    ui.label(&format!("Fila {i}"));
                }
            });
        });
        ui.code(&[
            "let campo = ui.text_field(\"nombre\", &mut nombre, \"Tu nombre\", 240.0);",
            "if campo.submitted { enviar(&nombre); }      // Enter; también .changed y .focused",
            "",
            "ui.scroll(\"lista\", 132.0, |ui| {",
            "    for fila in &filas { ui.label(fila); }",
            "});",
            "",
            "// El programa reenvía el teclado y la rueda:",
            "ui.text_input(\"a\");  ui.key_pressed(Key::Backspace);  ui.scroll_wheel(-40.0);",
        ]);
    });

    page(ui, "page-adaptativo", "scroll-adaptativo", s.tab == 8, |ui| {
        ui.title("Adaptativo");
        ui.muted("Usa una acción varias veces y aparta el ratón: pasa al frente y crece; las ignoradas se encogen.");
        ui.card("card-adaptativo", |ui| {
            if let Some(i) = ui.adaptive_actions("actions", &ACTIONS) {
                s.last = ACTIONS[i].1;
            }
            if let Some(i) = ui.suggestions("atajos", "Sugerido para ti:", "actions", &ACTIONS, 2) {
                s.last = ACTIONS[i].1;
            }
            ui.separator();
            ui.row(|ui| {
                if let Some(i) = ui.adaptive_menu("archivo", "Archivo", &FILE_MENU) {
                    s.file = FILE_MENU[i].1;
                }
                ui.muted("Menú adaptativo: lo que usas sube; lo que no, se pliega bajo «Más…». Elegido:");
                ui.label(s.file);
            });
            ui.separator();
            ui.row(|ui| {
                ui.muted("Última acción:");
                ui.label(s.last);
                if ui.button_with("olvidar", "Olvidar hábitos", Variant::Ghost, Size::Sm).clicked {
                    forget(ui, s);
                }
            });
        });
        ui.code(&[
            "let acciones = [(\"nuevo\", \"Nuevo\"), (\"abrir\", \"Abrir\"), (\"guardar\", \"Guardar\")];",
            "if let Some(i) = ui.adaptive_actions(\"acciones\", &acciones) { ejecutar(i); }",
            "",
            "// Atajos que aparecen solos con las 2 acciones más usadas de ese grupo",
            "ui.suggestions(\"atajos\", \"Sugerido para ti:\", \"acciones\", &acciones, 2);",
            "",
            "// Menú que ordena por uso y pliega lo que nunca se toca",
            "if let Some(i) = ui.adaptive_menu(\"archivo\", \"Archivo\", &entradas) { abrir(i); }",
            "",
            "// Guardar y restaurar la forma de cada persona (texto, unos 200 bytes)",
            "let texto = ui.profile().encode();",
            "ui.set_profile(Profile::decode(&texto)?);",
        ]);
    });

    page(ui, "page-mutacion", "scroll-mutacion", s.tab == 9, |ui| {
        ui.title("Mutación");
        ui.muted("Un componente puede convertirse en otro: la forma viaja y cambia de tamaño mientras el contenido se funde.");
        ui.muted("Envuelve lo que cambia en morph, con una clave y una «forma»; cuando la forma cambia, se transforma.");

        ui.card("card-buscador", |ui| {
            ui.label("Un botón que se convierte en buscador");
            ui.muted("Púlsalo y escribe. Si lo dejas vacío y haces clic fuera (o pulsas Escape), vuelve a ser botón.");
            let open = s.search_open;
            ui.morph("buscador", open, |ui| {
                if !open {
                    if ui.button_with("abrir", "Buscar", Variant::Soft, Size::Md).clicked {
                        s.search_open = true;
                        ui.request_focus("campo");
                    }
                } else {
                    let e = ui.text_field("campo", &mut s.query, "Escribe y pulsa Enter…", 340.0);
                    if e.submitted && !s.query.is_empty() {
                        ui.toast(&format!("Buscando «{}»", s.query));
                    }
                    if !e.focused && s.query.is_empty() {
                        s.search_open = false;
                    }
                }
            });
        });

        ui.card("card-cualquiera", |ui| {
            ui.label("Cualquier componente en cualquier otro");
            ui.muted("Elige una forma: el mismo elemento pasa por botón, casilla, campo, interruptor y tarjeta.");
            ui.segmented("forma", &["Botón", "Casilla", "Campo", "Interruptor", "Tarjeta"], &mut s.form);
            let form = s.form;
            ui.morph("camaleon", form, |ui| match form {
                0 => {
                    ui.button("boton", "Soy un botón");
                }
                1 => {
                    ui.checkbox("casilla", "Ahora soy una casilla", &mut s.mut_check);
                }
                2 => {
                    ui.text_field("campo", &mut s.mut_text, "Ahora soy un campo de texto", 320.0);
                }
                3 => ui.row(|ui| {
                    ui.toggle("interruptor", &mut s.mut_toggle);
                    ui.label("Y ahora un interruptor");
                }),
                _ => ui.card_w("tarjeta", 440.0, |ui| {
                    ui.title("Ahora soy una tarjeta");
                    ui.muted("Con título, texto y su propio botón.");
                    ui.button_with("accion", "Acción", Variant::Soft, Size::Sm);
                }),
            });
        });

        ui.card("card-uso", |ui| {
            ui.label("Según el uso: el mismo elemento cambia de componente");
            let level = ui.prominence("exportar");
            let mut used = false;
            ui.morph("exportar-forma", level, |ui| match level {
                Prominence::Compact => used = ui.button_with("compacto", "Exportar", Variant::Ghost, Size::Sm).clicked,
                Prominence::Regular => used = ui.button_with("normal", "Exportar", Variant::Soft, Size::Md).clicked,
                Prominence::Featured => ui.card_w("destacado", 460.0, |ui| {
                    ui.title("Exportar PDF");
                    ui.muted("Lo usas a menudo, así que ha pasado de botón a tarjeta.");
                    used = ui.button("ahora", "Exportar ahora").clicked;
                }),
            });
            if used {
                ui.record("exportar");
                ui.toast("Exportado");
            }
            ui.row(|ui| {
                let n = ui.profile().count(ui.id("exportar"));
                ui.muted(&format!("Usado {n} veces. Hacia la cuarta, el botón se convierte en tarjeta."));
                if ui.button_with("reiniciar-uso", "Olvidar hábitos", Variant::Ghost, Size::Sm).clicked {
                    forget(ui, s);
                }
            });
        });

        ui.code(&[
            "// Envuelve lo que cambia; cuando la «forma» cambia, uno se transforma en otro.",
            "ui.morph(\"buscador\", abierto, |ui| {",
            "    if !abierto {",
            "        if ui.button(\"abrir\", \"Buscar\").clicked { abierto = true; ui.request_focus(\"campo\"); }",
            "    } else {",
            "        ui.text_field(\"campo\", &mut consulta, \"Escribe…\", 340.0);",
            "    }",
            "});",
            "",
            "// La forma es cualquier valor con Hash: un bool, un número, un enum...",
            "let peso = ui.prominence(\"exportar\");",
            "ui.morph(\"exportar-forma\", peso, |ui| match peso {",
            "    Prominence::Featured => ui.card_w(\"tarjeta\", 460.0, |ui| { /* título, texto, botón */ }),",
            "    _ => { ui.button(\"boton\", \"Exportar\"); }",
            "});",
        ]);
    });
}

fn first_font(env: &str, candidates: &[&str]) -> Option<Vec<u8>> {
    std::env::var(env).ok().into_iter().chain(candidates.iter().map(|s| s.to_string())).find_map(|p| std::fs::read(p).ok())
}

/// Fuente de interfaz del sistema (o la de `MORFO_FONT=/ruta/fuente.ttf`).
pub fn load_font() -> Vec<u8> {
    first_font(
        "MORFO_FONT",
        &[
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "C:\\Windows\\Fonts\\segoeui.ttf",
        ],
    )
    .expect("no se encontró ninguna fuente; define MORFO_FONT con la ruta a un .ttf")
}

/// Fuente monoespaciada para el código (o `MORFO_MONO_FONT`). Si no hay
/// ninguna, el código se dibuja con la fuente normal.
pub fn load_mono_font() -> Option<Vec<u8>> {
    first_font(
        "MORFO_MONO_FONT",
        &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoMono-Regular.ttf",
            "/System/Library/Fonts/Menlo.ttc",
            "C:\\Windows\\Fonts\\consola.ttf",
        ],
    )
}

/// Renderizador con las fuentes del sistema ya cargadas.
pub fn renderer() -> morfo_skia::Renderer {
    let mut r = morfo_skia::Renderer::new(load_font()).expect("fuente no válida");
    if let Some(mono) = load_mono_font() {
        // Una fuente de código ilegible no impide abrir la documentación.
        let _ = r.set_mono_font(mono);
    }
    r
}
