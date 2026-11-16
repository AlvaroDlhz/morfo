//! The documentation pages: each explains a group of components, shows them
//! working and gives the code behind them.

use std::f32::consts::{FRAC_PI_2, TAU};

use morfo::theme::{ACCENT, BORDER, TEXT};
use morfo::{Align, Color, Icon, Prominence, Size, Ui, Variant, Vec2};

use crate::*;

pub fn inicio(ui: &mut Ui, _s: &mut State) {
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
}

pub fn botones(ui: &mut Ui, s: &mut State) {
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

    ui.space(8.0);
    ui.title("Cinco tipos más");
    ui.muted("Todos aceptan las cuatro variantes y los tres tamaños, y hacen morph al cambiar de estado.");
    ui.row(|ui| {
        ui.muted("Tamaño de todos:");
        ui.segmented("btn-size", &["Sm", "Md", "Lg"], &mut s.btn_size);
    });
    let size = Size::ALL[s.btn_size];
    let now = ui.now();
    ui.card("card-tipos", |ui| {
        variant_row(ui, "1 · Solo icono", "Redondo; para barras de herramientas.", |ui, i, v| {
            let key = format!("t-icono-{i}");
            ui.icon_button(&key, Icon::Star, v, size);
            ui.tooltip(&key, VARIANT_NAMES[i]);
        });
        ui.separator();
        variant_row(ui, "2 · Icono y texto", "El icono acompaña a la etiqueta.", |ui, i, v| {
            ui.button_icon(&format!("t-ambos-{i}"), Icon::Download, VARIANT_NAMES[i], v, size);
        });
        ui.separator();
        variant_row(ui, "3 · Conmutable", "Se queda pulsado: gana una variante más fuerte y una marca.", |ui, i, v| {
            ui.toggle_button(&format!("t-conmutable-{i}"), VARIANT_NAMES[i], &mut s.follow[i], v, size);
        });
        ui.separator();
        variant_row(ui, "4 · Con carga", "Púlsalo: se encoge en un círculo con indicador y vuelve al terminar.", |ui, i, v| {
            let busy = now < s.busy_until[i];
            if ui.loading_button(&format!("t-carga-{i}"), VARIANT_NAMES[i], busy, v, size).clicked {
                s.busy_until[i] = now + 2.0;
            }
        });
        ui.separator();
        variant_row(ui, "5 · Mantener para confirmar", "Mantenlo pulsado un segundo; si lo sueltas antes, se vacía.", |ui, i, v| {
            if ui.hold_button(&format!("t-mantener-{i}"), VARIANT_NAMES[i], 1.0, v, size) {
                ui.toast(&format!("Confirmado ({})", VARIANT_NAMES[i]));
            }
        });
    });
    ui.card("card-iconos", |ui| {
        ui.label("Iconos incluidos");
        ui.muted("Dibujados con trazos: nítidos a cualquier tamaño. Deja el cursor encima para ver su nombre.");
        ui.row(|ui| {
            for (i, icon) in Icon::ALL.into_iter().enumerate() {
                let key = format!("icono-{i}");
                ui.icon_button(&key, icon, Variant::Ghost, Size::Sm);
                ui.tooltip(&key, ICON_NAMES[i]);
            }
        });
    });
    ui.code(&[
        "ui.icon_button(\"fav\", Icon::Star, Variant::Ghost, Size::Md);",
        "ui.button_icon(\"bajar\", Icon::Download, \"Descargar\", Variant::Soft, Size::Md);",
        "ui.toggle_button(\"seguir\", \"Seguir\", &mut siguiendo, Variant::Outline, Size::Md);",
        "",
        "// Mientras `enviando` sea true: círculo con indicador, sin clics.",
        "if ui.loading_button(\"enviar\", \"Enviar\", enviando, Variant::Solid, Size::Md).clicked { enviar(); }",
        "",
        "// Devuelve true una sola vez, al completar 1,5 s pulsado.",
        "if ui.hold_button(\"borrar\", \"Mantén para borrar\", 1.5, Variant::Outline, Size::Md) { borrar(); }",
    ]);
}

pub fn controles(ui: &mut Ui, s: &mut State) {
    ui.title("Controles");
    ui.muted("Reciben el valor por referencia y devuelven true cuando la persona lo cambia.");
    ui.card("card-controles", |ui| {
        ui.row(|ui| {
            if ui.toggle("light", &mut s.light) {
                ui.set_theme(theme_of(s));
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

    ui.space(8.0);
    ui.title("Cinco controles más");
    ui.muted("Todos responden con muelles: se pueden arrastrar, interrumpir y nunca saltan.");
    ui.card("card-mas-controles", |ui| {
        ui.label("Rango");
        ui.muted("Dos manijas: se mueve la más cercana y ninguna adelanta a la otra.");
        ui.row(|ui| {
            let (mut lo, mut hi) = s.price;
            ui.range_slider("rango-doble", &mut lo, &mut hi, 320.0);
            s.price = (lo, hi);
            ui.muted(&format!("{:.0} – {:.0} €", lo * 500.0, hi * 500.0));
        });
        ui.separator();
        ui.row(|ui| {
            ui.column(|ui| {
                ui.label("Perilla");
                ui.muted("Arrastra arriba o abajo, o usa la rueda.");
                ui.row(|ui| {
                    ui.knob("volumen", &format!("Volumen {:.0}%", s.volume * 100.0), &mut s.volume);
                    let mut graves = 1.0 - s.volume;
                    if ui.knob("graves", "Graves", &mut graves) {
                        s.volume = 1.0 - graves;
                    }
                });
            });
            ui.space(24.0);
            ui.column(|ui| {
                ui.label("Pad XY");
                ui.muted("Arrastra el punto: dos valores a la vez.");
                ui.xy_pad("pad", &mut s.point, Vec2::new(260.0, 140.0));
            });
        });
        ui.separator();
        ui.label("Valoración");
        ui.muted("Pasa el cursor para previsualizar; vuelve a pulsar la misma para quitarla.");
        ui.row(|ui| {
            ui.rating("estrellas", &mut s.stars, 5);
            ui.muted(&format!("{} de 5", s.stars));
        });
        ui.separator();
        ui.label("Muestras de color");
        ui.muted("Elige un acento: toda la interfaz cambia de color con un morph.");
        let colors = ACCENTS.map(Color::hex);
        if ui.swatches("acento", &colors, &mut s.accent) {
            ui.set_theme(theme_of(s));
        }
    });
    ui.code(&[
        "ui.range_slider(\"precio\", &mut minimo, &mut maximo, 320.0);   // ambos en 0..=1",
        "ui.knob(\"volumen\", \"Volumen\", &mut volumen);",
        "ui.xy_pad(\"pad\", &mut punto, Vec2::new(260.0, 140.0));        // punto: Vec2 en 0..=1",
        "ui.rating(\"estrellas\", &mut estrellas, 5);                    // estrellas: u8",
        "if ui.swatches(\"acento\", &colores, &mut elegido) {",
        "    ui.set_theme(Theme::dark().with_accent(colores[elegido]));",
        "}",
    ]);
}

pub fn seleccion(ui: &mut Ui, s: &mut State) {
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
}

pub fn cajas(ui: &mut Ui, s: &mut State) {
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
        "// Barra de la aplicación (ver abajo): primera llamada del frame",
        "let barra = ui.title_bar(\"barra\", \"Mi app\", maximizada, |ui| { ui.muted(\"Inicio\"); });",
        "",
        "// Contenido propio que debe animar también su salida",
        "let p = ui.presence(\"aviso\", visible);   // 0..1; sigue declarándolo mientras p > 0",
    ]);
    ui.card("card-barra", |ui| {
        ui.label("Barra de la aplicación");
        ui.muted("La de arriba sustituye a la del gestor de ventanas. Arrástrala para mover la ventana,");
        ui.muted("haz doble clic para maximizar y usa los bordes de la ventana para cambiar su tamaño.");
        ui.code(&[
            "// Crea la ventana sin decoraciones y llama a title_bar lo primero de cada frame.",
            "let barra = ui.title_bar(\"barra\", \"Mi app\", ventana.is_maximized(), |ui| {",
            "    ui.icon_button(\"buscar\", Icon::Search, Variant::Ghost, Size::Sm);",
            "});",
            "// La librería no toca la ventana: dice qué se pidió y el programa lo hace.",
            "if barra.drag { ventana.drag_window(); }",
            "if let Some(borde) = barra.resize { ventana.drag_resize_window(direccion(borde)); }",
            "if barra.minimize { ventana.set_minimized(true); }",
            "if barra.toggle_maximize { ventana.set_maximized(!ventana.is_maximized()); }",
            "if barra.close { salir(); }",
        ]);
    });
    ui.space(8.0);
    ui.title("Rejilla, columnas y alineación");
    ui.card("card-rejilla", |ui| {
        ui.label("grid(3): celdas del mismo ancho, fila a fila");
        ui.grid(3, |ui| {
            for (i, name) in ["Ventas", "Clientes", "Pedidos", "Devoluciones", "Stock"].iter().enumerate() {
                ui.card(&format!("celda-{i}"), |ui| {
                    ui.muted(name);
                    ui.text(&format!("{}", 120 + i * 37), 24.0);
                });
            }
        });
        ui.separator();
        ui.label("columns(&[1.0, 2.0]): anchos proporcionales");
        ui.columns(&[1.0, 2.0], |ui, i| {
            ui.card(&format!("col-{i}"), |ui| {
                ui.muted(if i == 0 { "Un tercio" } else { "Dos tercios del ancho" });
            });
        });
        ui.separator();
        ui.align(Align::End, |ui| {
            ui.row(|ui| {
                ui.button_with("cancelar", "Cancelar", Variant::Ghost, Size::Md);
                ui.button("aceptar", "Aceptar");
            });
        });
    });
    ui.code(&[
        "ui.grid(3, |ui| { for t in &tarjetas { ui.card(t.clave, |ui| { /* ... */ }); } });",
        "ui.columns(&[1.0, 2.0], |ui, i| { /* columna i */ });",
        "ui.align(Align::End, |ui| { ui.button(\"aceptar\", \"Aceptar\"); });",
    ]);
}

pub fn formas(ui: &mut Ui, s: &mut State) {
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
    ui.space(8.0);
    ui.title("Dibujo libre");
    ui.muted("canvas reserva un hueco y te da la lista de dibujo: polígonos rellenos, arcos, líneas y texto.");
    let level = s.level;
    let pal = *ui.palette();
    ui.card("card-lienzo", |ui| {
        ui.canvas("lienzo", Vec2::new(420.0, 150.0), |d, r| {
            // A gauge: a track arc and a value arc.
            let c = Vec2::new(r.x + 80.0, r.y + 80.0);
            let (a0, sweep) = (0.75 * std::f32::consts::PI, 1.5 * std::f32::consts::PI);
            d.arc(c, 56.0, a0, a0 + sweep, 12.0, pal[BORDER]);
            d.arc(c, 56.0, a0, a0 + sweep * level, 12.0, pal[ACCENT]);
            d.add_text(Vec2::new(c.x - 20.0, c.y - 12.0), 20.0, pal[TEXT], &format!("{:.0}%", level * 100.0));
            // A polygon: a six-pointed star.
            let s = Vec2::new(r.x + 300.0, r.y + 75.0);
            let pts: Vec<Vec2> = (0..12)
                .map(|i| {
                    let rad = if i % 2 == 0 { 62.0 } else { 30.0 };
                    let a = i as f32 / 12.0 * TAU - FRAC_PI_2;
                    s + Vec2::new(a.cos(), a.sin()) * rad
                })
                .collect();
            d.polygon(&pts, Color::hex(0xf5b83d));
        });
        ui.muted("El arco usa el valor del slider de «Controles».");
    });
    ui.code(&[
        "ui.canvas(\"lienzo\", Vec2::new(420.0, 150.0), |d, r| {",
        "    d.arc(centro, 56.0, inicio, fin, 12.0, color);      // arco grueso con extremos redondeados",
        "    d.polygon(&puntos, Color::hex(0xf5b83d));          // cualquier forma, rellena",
        "});",
    ]);
}

pub fn flotantes(ui: &mut Ui, s: &mut State) {
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
        "",
        "// El índice de esta documentación: un riel en el borde izquierdo que se",
        "// despliega al acercar el cursor y se pliega al salir o al elegir.",
        "ui.side_nav(\"indice\", \"morfo\", &[\"Inicio\", \"Botones\", \"Controles\"], &mut pagina);",
    ]);
}

pub fn entrada(ui: &mut Ui, s: &mut State) {
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

    ui.space(8.0);
    ui.title("Texto largo y listas para elegir");
    ui.card("card-texto", |ui| {
        ui.label("Párrafos que se ajustan");
        ui.paragraph_muted(
            "Este párrafo se parte en líneas según el ancho disponible. Si cambias el tamaño de la ventana, \
             se recoloca y lo que hay debajo se aparta con el mismo muelle que el resto de la interfaz. \
             Las palabras demasiado largas para una línea se cortan entre letras.",
        );
        ui.separator();
        ui.label("Área de texto");
        ui.muted("Enter crea una línea nueva; las flechas se mueven entre líneas; hace scroll al llenarse.");
        ui.text_area("notas", &mut s.notes, "Escribe tus notas…", Vec2::new(520.0, 120.0));
        ui.separator();
        ui.row(|ui| {
            ui.column(|ui| {
                ui.label("Desplegable");
                ui.select("plan-select", &["Gratis", "Pro", "Equipo", "Empresa"], &mut s.plan_pick);
            });
            ui.space(24.0);
            ui.column(|ui| {
                ui.label("Combobox");
                if ui.combobox("ciudad", &CITIES, &mut s.city_pick, &mut s.city_query, "Busca una ciudad…", 260.0) {
                    ui.toast(&format!("Ciudad: {}", CITIES[s.city_pick.unwrap_or(0)]));
                }
            });
        });
        ui.muted("El combobox no distingue tildes ni mayúsculas: «mala» encuentra «Málaga».");
    });
    ui.code(&[
        "ui.paragraph(\"Texto largo que se ajusta al ancho disponible…\");",
        "ui.text_area(\"notas\", &mut notas, \"Escribe…\", Vec2::new(520.0, 120.0));",
        "ui.select(\"plan\", &[\"Gratis\", \"Pro\", \"Equipo\"], &mut plan);",
        "if ui.combobox(\"ciudad\", &CIUDADES, &mut ciudad, &mut busqueda, \"Busca…\", 260.0) {",
        "    // ciudad: Option<usize> con la elegida",
        "}",
    ]);
    ui.space(8.0);
    ui.title("Teclado, selección y accesibilidad");
    ui.card("card-teclado", |ui| {
        ui.paragraph_muted(
            "Tab recorre todos los controles (Mayús+Tab hacia atrás) y un anillo de foco salta de uno a otro. \
             Enter o Espacio los activan; las flechas mueven sliders, perillas, estrellas y muestras. \
             En los campos: arrastra o usa Mayús+flechas para seleccionar, Ctrl+A/C/X/V para el portapapeles; \
             los métodos de entrada (tildes compuestas, chino, japonés…) muestran el texto en composición subrayado.",
        );
        ui.paragraph_muted(
            "Para lectores de pantalla, la interfaz describe cada elemento (papel, nombre, valor, estado). \
             Esta ventana lo publica con AccessKit: Orca en Linux, Narrador en Windows o VoiceOver en macOS pueden leerla y pulsar sus botones.",
        );
    });
    ui.code(&[
        "// El programa reenvía modificadores, atajos, portapapeles e IME:",
        "ui.set_modifiers(Modifiers { shift, ctrl });",
        "ui.key_pressed(Key::Copy);            // y después: if let Some(t) = ui.take_clipboard() { … }",
        "ui.paste(&texto_del_portapapeles);",
        "ui.ime_preedit(&texto_en_composicion); // y ui.ime_area() para colocar la ventana del IME",
        "",
        "// Accesibilidad: solo cuesta algo cuando un lector la pide",
        "ui.set_accessibility(true);",
        "for nodo in ui.access_nodes() { /* papel, nombre, valor, rectángulo */ }",
    ]);
}

pub fn adaptativo(ui: &mut Ui, s: &mut State) {
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
            if ui.button_icon("abrir-paleta", Icon::Search, "Paleta de comandos (Ctrl+K)", Variant::Soft, Size::Sm).clicked {
                s.palette = true;
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
        "",
        "// Paleta de comandos: con nada escrito, primero lo que más usas",
        "if let Some(i) = ui.command_palette(\"paleta\", &mut abierta, &comandos) { ejecutar(i); }",
    ]);
}

pub fn mutacion(ui: &mut Ui, s: &mut State) {
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
}

pub fn datos(ui: &mut Ui, s: &mut State) {
    ui.title("Datos");
    ui.muted("Una tabla que se ordena con morph y una lista que se reordena arrastrando.");
    ui.card("card-tabla", |ui| {
        ui.label("Tabla ordenable");
        ui.muted("Pulsa una cabecera: ascendente, descendente y vuelta al orden original. Las filas viajan.");
        if let Some(i) = ui.table("productos", &["Producto", "Categoría", "Precio", "Stock"], &PRODUCTS, &mut s.sort) {
            s.product = Some(i);
        }
        ui.row(|ui| {
            ui.muted("Fila elegida:");
            ui.label(s.product.map_or("ninguna", |i| PRODUCTS[i][0]));
        });
    });
    ui.card("card-lista", |ui| {
        ui.label("Lista reordenable");
        ui.muted("Arrastra una tarea arriba o abajo: las demás le hacen sitio.");
        ui.reorderable("tareas", &mut s.tasks, |t| t.as_str());
    });
    ui.code(&[
        "if let Some(fila) = ui.table(\"productos\", &[\"Producto\", \"Precio\"], &filas, &mut orden) {",
        "    // orden: Option<(columna, descendente)>; las columnas numéricas se ordenan como números",
        "}",
        "ui.reorderable(\"tareas\", &mut tareas, |t| t.as_str());   // el texto es la identidad",
    ]);
}
