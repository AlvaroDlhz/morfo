//! The documentation pages: each explains a group of components, shows them
//! working and gives the code behind them.

use std::f32::consts::{FRAC_PI_2, TAU};

use morfo::theme::{ACCENT, BORDER, TEXT};
use morfo::{Align, Color, DrawList, Icon, Prominence, Rect, Reveal, Side, Size, Transition, Ui, Variant, Vec2};

use crate::*;
use morfo::Prominence as P;

/// Una línea por página, en el orden de [`PAGES`]: qué aprende quien llega a ella.
const DESCRIPCIONES: [&str; 18] = [
    "Las ideas de morfo, el ciclo de un frame y las reglas que lo sostienen.",
    "Variantes, tamaños y los cinco tipos de botón.",
    "Interruptores, deslizadores, perillas y otros controles de valor.",
    "Casillas, radios, etiquetas, contadores y acordeones.",
    "row, column, grid, card, reveal y scroll: cómo se colocan las cosas.",
    "Superficies que fluyen de una forma a otra, y dibujo con arcos y polígonos.",
    "Diálogos, menús, avisos y tooltips que flotan sobre lo demás.",
    "Campos de texto, áreas multilínea, desplegables y buscadores.",
    "Interfaces que se reordenan y se adaptan al uso de cada persona.",
    "Un componente que se transforma en otro sin cortes.",
    "Tablas ordenables, listas que se reordenan arrastrando y listas de miles de filas.",
    "Barras, líneas, tendencias, mapas de calor y medidores que viajan a sus datos.",
    "Pestañas, migas de pan, árboles, paginadores y un panel lateral.",
    "Fechas, colores, etiquetas, varias opciones a la vez y atajos de teclado.",
    "Esqueletos de carga, insignias, procesos por pasos, divisores y tableros.",
    "Contenido que entra al verse, contadores, paralaje, cabeceras fijas y saltos a un ancla.",
    "Siete coreografías para cambiar de vista y elementos que vuelan de una vista a otra.",
    "Componentes dentro de componentes: claves propias, bloques pulsables o desactivados, contenido en pestañas y tablas, estilo heredado.",
];

pub fn inicio(ui: &mut Ui, _s: &mut State) {
    ui.title("Qué es morfo");
    ui.paragraph_muted(
        "morfo es una librería de componentes para interfaces en Rust donde cada cambio es una transición continua y la interfaz se adapta a quien la usa. Dos personas que ejecutan el mismo programa pueden acabar viendo interfaces distintas.",
    );
    ui.space(4.0);
    ui.grid(2, |ui| {
        let ideas = [
            (
                "Movimiento sin código",
                "Reordenar, redimensionar, cambiar de variante o de tema es siempre una transición. No hay código de animación que escribir.",
            ),
            ("Adaptación por uso", "La interfaz aprende qué usa cada persona y reordena y agranda lo que más usa."),
            ("Coste cero en reposo", "Si nada se mueve, frame.animating es false y el programa duerme hasta el próximo evento."),
            ("Tres renderizadores", "El núcleo no depende de nada. Pinta por CPU, GPU o en el navegador, según el backend que elijas."),
        ];
        for (i, (titulo, texto)) in ideas.into_iter().enumerate() {
            ui.card(&format!("idea-{i}"), |ui| {
                ui.label(titulo);
                ui.paragraph_muted(texto);
            });
        }
    });

    ui.space(8.0);
    ui.title("Tres ideas que lo sostienen");
    ui.paragraph_muted("Todo lo demás de la librería se apoya en estas tres.");
    ui.card("ideas", |ui| {
        ui.label("1 · Identidad: una clave, un elemento");
        ui.paragraph_muted(
            "Cada elemento se reconoce por su clave (\"guardar\"). Si en el siguiente frame la misma clave aparece en otro sitio, con otra forma o con otro texto, el elemento no se recrea: viaja hasta allí. Por eso reordenar o transformar no necesita código extra.",
        );
        ui.separator();
        ui.label("2 · Muelles: el layout solo fija destinos");
        ui.paragraph_muted(
            "El layout calcula dónde debe estar cada cosa. Un muelle analítico (la solución cerrada del muelle amortiguado) lleva posición, tamaño, color y tipografía hasta ahí. Es exacto a cualquier frecuencia de refresco y se puede interrumpir a mitad de camino conservando la velocidad.",
        );
        ui.separator();
        ui.label("3 · Perfil: el uso decide el orden");
        ui.paragraph_muted(
            "El perfil guarda una puntuación por elemento que decae con el tiempo. De ella salen el orden, el peso visual y, si se activa, la densidad. Tiene histéresis para que nada baile, y no se mueve nada mientras el puntero está encima.",
        );
    });

    ui.space(8.0);
    ui.title("El ciclo de un frame");
    ui.paragraph_muted("La aplicación no guarda una copia de la interfaz. Cada frame declara lo que debe existir, y morfo se encarga del resto.");
    ui.card("ciclo", |ui| {
        let pasos = [
            ("ui.begin(tiempo, tamaño)", "Empieza el frame: avanza los muelles según el tiempo transcurrido y fija el área disponible."),
            (
                "Declarar",
                "Cada llamada (button, card, text_field…) describe un elemento y devuelve una Response: clicked, hovered, held y su rectángulo.",
            ),
            ("ui.end()", "Resuelve el foco y la entrada, y devuelve un Frame con animating y wake_in."),
            ("Pintar", "El backend dibuja ui.draw_list(). El de GPU omite los frames idénticos al anterior."),
            ("Dormir o repetir", "Si animating es true se pide otro frame; si no, el programa espera a la siguiente entrada o a wake_in."),
        ];
        for (i, (paso, texto)) in pasos.into_iter().enumerate() {
            ui.label(&format!("{}. {paso}", i + 1));
            ui.paragraph_muted(texto);
        }
    });

    ui.space(8.0);
    ui.title("Un programa mínimo");
    ui.paragraph_muted(
        "Así es el esqueleto de cualquier aplicación hecha con morfo: un Ui, un frame por vuelta y la entrada que llega entre frames.",
    );
    ui.code(&[
        "use morfo::{Theme, Ui, Vec2};",
        "",
        "let mut ui = Ui::new(Theme::dark());",
        "let mut guardado = 0;",
        "",
        "// cada frame, con el tiempo en segundos y el tamaño en píxeles lógicos",
        "ui.begin(ahora(), Vec2::new(ancho, alto));",
        "ui.heading(\"Hola\");",
        "if ui.button(\"guardar\", \"Guardar\").clicked { guardado += 1; }",
        "ui.muted(&format!(\"Guardado {guardado} veces\"));",
        "let frame = ui.end();",
        "",
        "pintar(ui.draw_list(), ui.background());",
        "if frame.animating { pedir_frame(); }",
        "else if let Some(s) = frame.wake_in { despertar_en(s); }",
        "",
        "// la entrada llega entre frames:",
        "ui.pointer_moved(posicion); ui.pointer_pressed(); ui.pointer_released();",
    ]);

    ui.space(8.0);
    ui.title("Cuatro reglas");
    ui.card("reglas", |ui| {
        ui.label("1 · La clave es la identidad");
        ui.paragraph_muted(
            "\"guardar\" identifica al elemento a lo largo de los frames, y también en el perfil guardado entre sesiones. Cámbiala y el elemento pasa a ser otro; cámbiala de sitio y viaja hasta allí. Lo que hay dentro de una tarjeta, un diálogo o cualquier contenedor con clave vive bajo la suya, así que se puede repetir sin choques; para repetir un grupo sin contenedor, ui.scope(clave, …).",
        );
        ui.separator();
        ui.label("2 · Si animating es false, no hay nada que hacer");
        ui.paragraph_muted("El programa puede dormir hasta el próximo evento. Es lo que hace que una interfaz en reposo cueste cero.");
        ui.separator();
        ui.label("3 · wake_in es el despertador");
        ui.paragraph_muted(
            "Un aviso que caduca, un cursor de texto que parpadea o una tooltip que aparece necesitan un frame aunque nadie toque nada. wake_in dice en cuántos segundos.",
        );
        ui.separator();
        ui.label("4 · La entrada se pasa tal cual llega");
        ui.paragraph_muted(
            "pointer_moved, pointer_pressed, pointer_released, scroll_wheel, key_pressed, text_input, paste e ime_preedit. morfo decide qué significa cada una según lo que haya bajo el puntero o el foco.",
        );
    });

    ui.space(8.0);
    ui.title("Mapa del curso");
    ui.paragraph_muted(
        "Cada página explica un grupo de componentes, lo muestra funcionando y enseña el código que lo produce. El temario está en el borde izquierdo: lleva el cursor allí para desplegarlo.",
    );
    for (m, (modulo, paginas)) in COURSE.iter().enumerate() {
        ui.card(&format!("modulo-{m}"), |ui| {
            ui.label(&format!("Módulo {} · {modulo}", m + 1));
            for &p in paginas.iter() {
                ui.paragraph(&format!("Lección {} · {}", lesson_of(p) + 1, PAGES[p]));
                ui.paragraph_muted(DESCRIPCIONES[p]);
            }
        });
    }

    ui.space(8.0);
    ui.title("Empezar en tu proyecto");
    ui.card("empezar", |ui| {
        ui.label("1 · Añade la librería");
        ui.code(&["[dependencies]", "morfo = { package = \"morfo-ui\", version = \"0.1\" }"]);
        ui.label("2 · Elige un backend para pintar");
        ui.paragraph_muted("morfo-skia pinta por CPU sin necesitar GPU; morfo-wgpu lo hace con la GPU; morfo-web lo lleva al navegador.");
        ui.label("3 · Mira la demo, que se recarga sola");
        ui.paragraph_muted(
            "Esta documentación es una aplicación hecha con morfo. Con cargo run -p morfo-demo --bin docs, guardar un fichero la recarga en unos segundos, conservando la página y el estado. Su código, demo/src/pages.rs, es el ejemplo más completo.",
        );
    });

    ui.space(8.0);
    ui.title("Lo que todavía no hace");
    ui.card("limites", |ui| {
        ui.paragraph_muted("Una fuente normal y una monoespaciada: sin negritas, cursivas ni texto bidireccional (árabe, hebreo).");
        ui.paragraph_muted("La tabla dibuja todas sus filas: para miles de filas hace falta paginarla.");
        ui.paragraph_muted("El scroll es vertical y con rueda. Dibuja solo lo visible, pero recorre el layout de todo el contenido.");
        ui.paragraph_muted("La recarga en vivo relanza el programa entero y solo funciona en Unix.");
    });
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
    ui.space(8.0);
    ui.title("Salidas animadas");
    ui.muted("transient anima la salida de cualquier contenido, aunque ya no se construya: se encoge, se funde y los demás cierran el hueco.");
    ui.card("card-avisos", |ui| {
        let notices = [
            ("Copia de seguridad completada", "Hace 2 minutos"),
            ("Nuevo comentario en «Portada»", "Hace 1 hora"),
            ("Actualización disponible", "Ayer"),
        ];
        for (i, (title, when)) in notices.iter().enumerate() {
            ui.transient(&format!("aviso-{i}"), !s.dismissed[i], |ui| {
                ui.row(|ui| {
                    ui.column(|ui| {
                        ui.label(title);
                        ui.muted(when);
                    });
                    if ui.icon_button(&format!("cerrar-{i}"), Icon::Close, Variant::Ghost, Size::Sm).clicked {
                        s.dismissed[i] = true;
                    }
                });
            });
        }
        if s.dismissed.iter().all(|&d| d) && ui.button_with("restaurar", "Volver a mostrar", Variant::Soft, Size::Sm).clicked {
            s.dismissed = [false; 3];
        }
    });
    ui.code(&[
        "ui.transient(\"aviso\", visible, |ui| { ui.label(\"Copia completada\"); });",
        "// visible = false: lo último que se vio sale animado; f ya no se llama",
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
            ui.toggle_button("notice-btn", "Aviso fijo", &mut s.pinned, Variant::Outline, Size::Md);
            ui.tooltip("notice-btn", "Se queda en la esquina mientras se declare");
        });
        if s.pinned {
            ui.notice("fijo", "Sincronizando… Este aviso no caduca: sigue aquí hasta que lo apagues.");
        }
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
        "ui.toast(\"Guardado\"); // se va solo",
        "if sincronizando { ui.notice(\"sync\", \"Sincronizando…\"); } // se queda mientras se declare",
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
        // After «Nuevo» you always «Abrir»? It is offered before you look for it.
        if let Some(i) = ui.predicted_action("siguiente", "Lo siguiente que sueles hacer:", "actions", &ACTIONS) {
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
    ui.space(8.0);
    ui.title("Inicio adaptativo");
    ui.muted("Las aplicaciones que más abres pasan delante y se hacen el doble de anchas.");
    ui.adaptive_dashboard("inicio", &TILES, 3, |ui, i, level| {
        let (_, name) = TILES[i];
        ui.title(name);
        ui.muted(match level {
            P::Featured => "La usas a menudo",
            P::Compact => "Casi no la usas",
            P::Regular => "Ábrela para usarla",
        });
        let label = format!("Abrir {}", name.to_lowercase());
        ui.button_with(&format!("abrir-{i}"), &label, if level == P::Featured { Variant::Solid } else { Variant::Soft }, Size::Sm).clicked
    });

    ui.space(8.0);
    ui.title("Tus hábitos");
    ui.muted("Todo lo que la interfaz ha aprendido de ti, por qué y cómo controlarlo.");
    let habits = ui.habits();
    ui.card("card-habitos", |ui| {
        ui.row(|ui| {
            let mut learning = !ui.profile().frozen;
            if ui.toggle("aprender", &mut learning) {
                ui.profile_mut().frozen = !learning;
            }
            ui.label(if learning { "Aprender de mi uso" } else { "Aprendizaje en pausa: nada nuevo se registra" });
        });
        ui.row(|ui| {
            if ui.toggle("densidad", &mut s.density) {
                ui.set_adaptive_density(s.density);
            }
            ui.label("Densidad adaptativa");
            ui.muted(&format!("(ahora al {:.0} %: cuanto más usas la app, más compacta)", ui.density() * 100.0));
        });
        ui.separator();
        if habits.is_empty() {
            ui.muted("Aún no hay hábitos: usa un poco la documentación.");
        }
        let top = habits.first().map_or(1.0, |h| h.score.max(1e-3));
        for h in habits.iter().take(6) {
            let key = format!("habito-{:x}", h.id.0);
            ui.scope(&key, |ui| {
                ui.row(|ui| {
                    ui.column(|ui| {
                        ui.label(&h.label);
                        ui.muted(&reason_text(ui, h.reason));
                    });
                });
                ui.row(|ui| {
                    ui.progress(h.score / top, 360.0);
                    ui.muted(&format!("{} usos", h.count));
                    if ui.button_with("olvidar", "Olvidar", Variant::Ghost, Size::Sm).clicked {
                        ui.profile_mut().forget(h.id);
                    }
                });
            });
        }
        ui.separator();
        ui.row(|ui| {
            if ui.button_icon("copiar-perfil", Icon::Download, "Copiar mi perfil", Variant::Soft, Size::Sm).clicked {
                let text = ui.profile().encode();
                ui.copy_to_clipboard(&text);
                ui.toast("Perfil copiado: llévalo a otro equipo o guárdalo");
            }
            if ui.hold_button("borrar-perfil", "Mantén para borrarlo todo", 1.2, Variant::Outline, Size::Sm) {
                forget(ui, s);
                ui.toast("Perfil borrado");
            }
        });
    });

    ui.code(&[
        "// La siguiente acción habitual, ofrecida antes de que la busques",
        "ui.predicted_action(\"siguiente\", \"Lo siguiente:\", \"acciones\", &acciones);",
        "",
        "// Panel de inicio: lo más usado delante y a doble ancho",
        "ui.adaptive_dashboard(\"inicio\", &apps, 3, |ui, i, peso| ui.button(\"abrir\", \"Abrir\").clicked);",
        "",
        "// Control de la persona: ver, explicar, olvidar, pausar, exportar",
        "for h in ui.habits() { /* h.label, h.count, h.reason */ }",
        "ui.profile_mut().frozen = true;          ui.profile_mut().forget(id);",
        "ui.set_adaptive_density(true);          // más compacta cuanto más se usa",
        "ui.profile_mut().time_of_day = Some(desfase_utc);   // hábitos por hora del día",
        "",
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
            ui.record_named("exportar", "Exportar PDF");
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

    ui.space(8.0);
    ui.title("Listas muy largas");
    ui.card("card-registro", |ui| {
        ui.paragraph_muted(
            "Veinte mil filas, y solo se construyen las que se ven: cuesta lo mismo que una lista de diez. Las filas que entran al hacer scroll aparecen tal cual, sin animación de entrada.",
        );
        ui.virtual_list("registro", 20_000, 34.0, 240.0, |ui, i| {
            ui.muted(&format!("#{:05}", i + 1));
            ui.label(&format!("{} en {}", EVENTOS[i % EVENTOS.len()], CITIES[(i * 7) % CITIES.len()]));
            ui.fill();
            ui.muted(&format!("{:02}:{:02}", (i / 60) % 24, i % 60));
        });
    });
    ui.code(&[
        "// 20 000 filas de 34 px en una ventana de 240 px",
        "ui.virtual_list(\"registro\", 20_000, 34.0, 240.0, |ui, i| { ui.label(&fila(i)); });",
    ]);
}

const EVENTOS: [&str; 5] = ["Pedido enviado", "Pago recibido", "Alta de cliente", "Devolución", "Consulta"];

// ----- Gráficos ---------------------------------------------------------------

const MESES: [&str; 6] = ["Jun", "Jul", "Ago", "Sep", "Oct", "Nov"];
const VENTAS: [[f32; 6]; 2] = [[120.0, 95.0, 140.0, 180.0, 160.0, 210.0], [80.0, 110.0, 90.0, 130.0, 170.0, 150.0]];
const DIAS: [&str; 7] = ["Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom"];
const VISITAS: [[f32; 7]; 2] = [[320.0, 410.0, 380.0, 450.0, 520.0, 610.0, 580.0], [280.0, 300.0, 350.0, 330.0, 400.0, 420.0, 390.0]];
const REGISTROS: [[f32; 7]; 2] = [[40.0, 52.0, 48.0, 61.0, 70.0, 95.0, 88.0], [30.0, 34.0, 41.0, 38.0, 45.0, 50.0, 47.0]];
const TENDENCIAS: [(&str, [f32; 8], &str); 3] = [
    ("Ingresos", [12.0, 14.0, 13.0, 17.0, 16.0, 19.0, 21.0, 24.0], "+18 %"),
    ("Gastos", [9.0, 8.5, 9.4, 8.8, 8.1, 8.3, 7.9, 7.6], "-4 %"),
    ("Devoluciones", [3.0, 3.2, 2.8, 3.9, 4.4, 4.1, 4.8, 5.2], "+9 %"),
];
const HORAS: [&str; 8] = ["0h", "3h", "6h", "9h", "12h", "15h", "18h", "21h"];

/// Uso por día y hora, inventado pero con forma creíble: más entre semana y
/// en horario de trabajo.
fn uso_semanal() -> Vec<f32> {
    let hora = [0.1, 0.05, 0.3, 1.0, 0.85, 0.9, 0.6, 0.35];
    (0..7).flat_map(|d| (0..8).map(move |h| hora[h] * if d >= 5 { 0.45 } else { 1.0 } * (0.8 + ((d * 8 + h) % 5) as f32 * 0.1) * 100.0)).collect()
}

pub fn graficos(ui: &mut Ui, s: &mut State) {
    ui.title("Gráficos");
    ui.muted("Cada valor y la propia escala son muelles: con datos nuevos, el gráfico viaja hasta ellos en lugar de redibujarse.");
    let set = s.chart_set.min(1);
    ui.card("card-barras", |ui| {
        ui.row(|ui| {
            ui.label("Ventas por mes");
            ui.fill();
            ui.segmented("serie", &["Este año", "El anterior"], &mut s.chart_set);
        });
        let ancho = ui.available_width();
        if let Some(i) = ui.bar_chart("barras", &MESES, &VENTAS[set], Vec2::new(ancho, 200.0)) {
            ui.toast(&format!("{}: {} ventas", MESES[i], VENTAS[set][i]));
        }
        ui.separator();
        ui.label("Visitas y registros por día");
        ui.line_chart("lineas", &DIAS, &[("Visitas", &VISITAS[set]), ("Registros", &REGISTROS[set])], Vec2::new(ancho, 220.0));
        ui.muted("Cambia de año arriba: barras y líneas viajan a los valores nuevos. Pasa el ratón para ver cada valor.");
    });
    ui.code(&[
        "// Las barras se identifican por su etiqueta: reordenar los datos las mueve.",
        "if let Some(i) = ui.bar_chart(\"ventas\", &meses, &valores, Vec2::new(600.0, 200.0)) {",
        "    abrir_detalle(i);",
        "}",
        "ui.line_chart(\"visitas\", &dias, &[(\"Visitas\", &v), (\"Registros\", &r)], tam);",
    ]);
    ui.space(8.0);
    ui.title("Tendencias, calor y medidores");
    ui.card("card-mini", |ui| {
        ui.label("sparkline: la tendencia dentro de una fila");
        for (nombre, valores, cambio) in TENDENCIAS {
            ui.row(|ui| {
                ui.space(4.0);
                ui.label(nombre);
                ui.fill();
                ui.sparkline(&valores, 140.0);
                ui.muted(cambio);
            });
        }
        ui.separator();
        ui.label("heatmap: uso por día de la semana y hora");
        if let Some((d, h)) = ui.heatmap("calor", &DIAS, &HORAS, &uso_semanal()) {
            ui.toast(&format!("{} a las {}", DIAS[d], HORAS[h]));
        }
        ui.separator();
        ui.row(|ui| {
            ui.column(|ui| {
                ui.label("gauge: un valor entre 0 y 1");
                ui.muted("Ámbar al acercarse al límite, rojo al llegar.");
                ui.slider("nivel-medidor", &mut s.gauge_level, 220.0);
            });
            ui.fill();
            ui.gauge("cpu", "Uso de CPU", s.gauge_level, 180.0);
        });
    });
    ui.code(&[
        "ui.sparkline(&ultimos_dias, 140.0);                 // verde si sube, rojo si baja",
        "ui.heatmap(\"uso\", &dias, &horas, &valores);         // valores fila a fila",
        "ui.gauge(\"cpu\", \"Uso de CPU\", 0.72, 180.0);",
    ]);
}

// ----- Navegación -------------------------------------------------------------

const ARCHIVOS: [(u8, &str); 10] = [
    (0, "proyecto"),
    (1, "src"),
    (2, "main.rs"),
    (2, "ui"),
    (3, "botones.rs"),
    (3, "tablas.rs"),
    (1, "docs"),
    (2, "guia.md"),
    (1, "Cargo.toml"),
    (0, "notas.txt"),
];

/// Los índices de `ARCHIVOS` desde la raíz hasta `i`, ambos incluidos.
fn ruta(i: usize) -> Vec<usize> {
    let mut out = vec![i];
    let mut depth = ARCHIVOS[i].0;
    for j in (0..i).rev() {
        if ARCHIVOS[j].0 < depth {
            out.push(j);
            depth = ARCHIVOS[j].0;
        }
    }
    out.reverse();
    out
}

pub fn navegacion(ui: &mut Ui, s: &mut State) {
    ui.title("Navegación");
    ui.muted("Pestañas, migas de pan, un árbol, un paginador y un panel lateral. Todo lo que cambia de sitio viaja.");
    ui.card("card-pestanas", |ui| {
        ui.tabs("pestanas", &["Resumen", "Actividad", "Ajustes"], &mut s.nav_tab);
        let tab = s.nav_tab;
        ui.morph("contenido-pestana", tab, |ui| match tab {
            0 => {
                ui.label("Resumen del proyecto");
                ui.paragraph_muted(
                    "La línea de debajo de las pestañas salta con un pequeño rebote, y el contenido se transforma en el de la pestaña nueva.",
                );
            }
            1 => {
                for (i, linea) in ["Ana ha comentado el informe", "Se ha publicado la versión 0.2", "Luis se ha unido al equipo"].iter().enumerate()
                {
                    ui.scope(&format!("act-{i}"), |ui| {
                        ui.row(|ui| {
                            ui.badge("punto", 1);
                            ui.label(linea);
                        });
                    });
                }
            }
            _ => {
                ui.row(|ui| {
                    ui.toggle("notif", &mut s.news);
                    ui.label("Notificaciones");
                });
                ui.row(|ui| {
                    ui.toggle("auto", &mut s.autosave);
                    ui.label("Guardar automáticamente");
                });
            }
        });
    });
    ui.code(&[
        "ui.tabs(\"pestanas\", &[\"Resumen\", \"Actividad\", \"Ajustes\"], &mut pestana);",
        "ui.morph(\"contenido\", pestana, |ui| match pestana { /* cada pestaña */ });",
    ]);

    ui.space(8.0);
    ui.title("Árbol y migas de pan");
    ui.card("card-arbol", |ui| {
        ui.columns(&[1.0, 1.3], |ui, col| {
            if col == 0 {
                ui.tree("archivos", &ARCHIVOS, &mut s.tree_sel);
            } else {
                let elegido = s.tree_sel.unwrap_or(0).min(ARCHIVOS.len() - 1);
                let indices = ruta(elegido);
                let nombres: Vec<&str> = indices.iter().map(|&i| ARCHIVOS[i].1).collect();
                if let Some(k) = ui.breadcrumbs("migas", &nombres) {
                    s.tree_sel = Some(indices[k]);
                }
                ui.paragraph_muted("Las migas siguen al árbol. Pulsa una para subir; si la ruta no cabe, su centro se pliega en «…».");
                ui.muted("Con el foco en una rama, la flecha derecha la abre y la izquierda la cierra.");
            }
        });
    });
    ui.code(&[
        "// (profundidad, nombre) en orden de lectura; el árbol recuerda qué ramas están abiertas.",
        "ui.tree(\"archivos\", &[(0, \"src\"), (1, \"main.rs\"), (0, \"Cargo.toml\")], &mut elegido);",
        "if let Some(i) = ui.breadcrumbs(\"ruta\", &[\"proyecto\", \"src\", \"main.rs\"]) { subir_a(i); }",
    ]);

    ui.space(8.0);
    ui.title("Paginador y panel lateral");
    ui.card("card-paginas", |ui| {
        let paginas = 24;
        ui.row(|ui| {
            ui.pagination("paginador", paginas, &mut s.page_no);
            ui.fill();
            ui.muted(&format!("Página {} de {paginas}", s.page_no + 1));
        });
        let base = s.page_no * 3;
        ui.morph("resultados", s.page_no, |ui| {
            for i in 0..3 {
                ui.label(&format!("Resultado {}", base + i + 1));
            }
        });
        ui.separator();
        ui.row(|ui| {
            if ui.button_icon("abrir-cajon", Icon::Menu, "Abrir panel lateral", Variant::Soft, Size::Md).clicked {
                s.drawer = true;
            }
            ui.muted("Entra por la derecha; se cierra con la ×, Escape o un clic fuera.");
        });
    });
    ui.drawer("cajon", "Ajustes rápidos", Side::Right, &mut s.drawer, 340.0, |ui| {
        ui.row(|ui| {
            ui.toggle("noticias", &mut s.news);
            ui.label("Recibir novedades");
        });
        ui.row(|ui| {
            ui.toggle("guardar", &mut s.autosave);
            ui.label("Guardar automáticamente");
        });
        ui.label("Volumen");
        ui.slider("volumen", &mut s.volume, 280.0);
        ui.paragraph_muted("Lo que se declara dentro del cajón es una interfaz como cualquier otra.");
    });
    ui.code(&[
        "ui.pagination(\"paginas\", 24, &mut pagina);           // pagina empieza en 0",
        "ui.drawer(\"ajustes\", \"Ajustes\", Side::Right, &mut abierto, 340.0, |ui| {",
        "    ui.toggle(\"noticias\", &mut noticias);",
        "});",
    ]);
}

// ----- Formularios ------------------------------------------------------------

const DIAS_LARGOS: [&str; 7] = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"];
const LENGUAJES: [&str; 8] = ["Rust", "Go", "Python", "TypeScript", "Kotlin", "Swift", "C++", "Zig"];

pub fn formularios(ui: &mut Ui, s: &mut State) {
    ui.title("Formularios");
    ui.muted("Fechas, colores, etiquetas, varias opciones a la vez y atajos de teclado.");
    ui.card("card-fecha", |ui| {
        ui.row(|ui| {
            ui.label("Fecha de entrega");
            ui.date_picker("fecha", &mut s.date);
            ui.muted(&format!("Cae en {}.", DIAS_LARGOS[s.date.weekday() as usize]));
        });
        ui.muted("Ábrelo: las flechas cambian de mes y los días se deslizan. Con el campo enfocado, las flechas mueven un día o una semana.");
        ui.separator();
        ui.row_top(|ui| {
            ui.color_picker("color", &mut s.color, 260.0);
            ui.column(|ui| {
                ui.label("Selector de color");
                ui.paragraph_muted("Saturación y brillo en el cuadrado, tono en la barra. Recuerda el tono aunque llegues al negro.");
                if ui.button_with("acento", "Probar como acento", Variant::Soft, Size::Md).clicked {
                    ui.set_theme(crate::theme_of(s).with_accent(s.color));
                }
            });
        });
    });
    ui.code(&[
        "ui.date_picker(\"entrega\", &mut fecha);             // morfo::Date",
        "ui.color_picker(\"color\", &mut color, 260.0);        // morfo::Color",
    ]);
    ui.space(8.0);
    ui.title("Etiquetas, opciones y atajos");
    ui.card("card-etiquetas", |ui| {
        let ancho = ui.available_width().min(520.0);
        ui.label("Etiquetas");
        ui.tag_input("etiquetas", &mut s.labels, "Escribe y pulsa Intro o coma", ancho);
        ui.muted("Cada etiqueta sale volando del campo. Retroceso con el campo vacío quita la última.");
        ui.separator();
        ui.label("Lenguajes que usas");
        ui.multi_select("lenguajes", &LENGUAJES, &mut s.langs, "Elige uno o varios", ancho);
        ui.muted("La lista no se cierra al elegir; escribe para filtrar (sin importar tildes).");
        ui.separator();
        ui.row(|ui| {
            ui.label("Atajo para buscar");
            ui.shortcut_input("atajo", &mut s.shortcut, 260.0);
        });
        ui.muted("Púlsalo y haz la combinación. Retroceso lo borra, Escape lo deja como estaba.");
    });
    ui.code(&[
        "ui.tag_input(\"etiquetas\", &mut etiquetas, \"Añade etiquetas\", 520.0);",
        "ui.multi_select(\"lenguajes\", &opciones, &mut elegidas, \"Elige\", 520.0);",
        "ui.shortcut_input(\"atajo\", &mut atajo, 260.0);         // Option<Shortcut>",
        "// El programa pasa Ctrl+tecla con ui.chord(tecla) mientras ui.wants_keys().",
    ]);
}

// ----- Estado y paneles -------------------------------------------------------

const PASOS: [&str; 4] = ["Cuenta", "Datos", "Pago", "Listo"];

pub fn estado(ui: &mut Ui, s: &mut State) {
    ui.title("Estado y paneles");
    ui.muted("Contenido que carga, avisos con número, procesos por pasos y espacio que se reparte arrastrando.");
    let cargando = ui.now() < s.load_until;
    ui.card("card-carga", |ui| {
        ui.row(|ui| {
            ui.label("Esqueleto mientras carga");
            ui.fill();
            if ui.button_with("recargar", "Recargar", Variant::Soft, Size::Sm).clicked {
                s.load_until = ui.now() + 2.0;
            }
        });
        ui.skeleton("perfil", cargando, 4, |ui| {
            ui.title("Ada Lovelace");
            ui.paragraph_muted("Escribió el primer algoritmo pensado para una máquina. Este texto ocupa el sitio que tenían las líneas grises, y llega transformándose desde ellas.");
        });
        ui.separator();
        ui.row(|ui| {
            ui.icon_button("buzon", Icon::Download, Variant::Soft, Size::Md);
            ui.badge_on("buzon", s.inbox);
            ui.label("Sin leer");
            ui.badge("sin-leer", s.inbox);
            ui.fill();
            if ui.button_with("llega", "Llega uno", Variant::Ghost, Size::Sm).clicked {
                s.inbox += 1;
            }
            if ui.button_with("leer", "Leer todo", Variant::Ghost, Size::Sm).clicked {
                s.inbox = 0;
            }
        });
    });
    ui.code(&[
        "ui.skeleton(\"perfil\", cargando, 4, |ui| { /* el contenido real */ });",
        "ui.icon_button(\"buzon\", Icon::Download, Variant::Soft, Size::Md);",
        "ui.badge_on(\"buzon\", sin_leer);                       // salta al cambiar, se va en 0",
    ]);
    ui.space(8.0);
    ui.title("Pasos");
    ui.card("card-pasos", |ui| {
        ui.steps("pasos", &PASOS, &mut s.step);
        let paso = s.step;
        ui.morph("paso", paso, |ui| {
            ui.label(&format!("Paso {}: {}", paso + 1, PASOS[paso]));
            ui.paragraph_muted(match paso {
                0 => "Crea tu cuenta. Los pasos ya hechos se marcan y se pueden pulsar para volver.",
                1 => "Tus datos. La línea se llena hasta el paso actual.",
                2 => "El pago. El contenido de cada paso se transforma en el del siguiente.",
                _ => "Todo listo.",
            });
        });
        ui.row(|ui| {
            if s.step > 0 && ui.button_icon("atras", Icon::ArrowLeft, "Atrás", Variant::Ghost, Size::Md).clicked {
                s.step -= 1;
            }
            ui.fill();
            if s.step + 1 < PASOS.len() && ui.button_icon("adelante", Icon::ArrowRight, "Siguiente", Variant::Solid, Size::Md).clicked {
                s.step += 1;
            }
        });
    });
    ui.code(&["ui.steps(\"alta\", &[\"Cuenta\", \"Datos\", \"Pago\", \"Listo\"], &mut paso);"]);
    ui.space(8.0);
    ui.title("Divisor y tablero");
    ui.card("card-paneles", |ui| {
        ui.splitter("divisor", &mut s.split, 170.0, |ui, i| {
            if i == 0 {
                ui.label("Notas");
                ui.paragraph_muted("Arrastra la barra del medio: el texto se reajusta al ancho mientras se mueve. Con el foco en ella, las flechas también la mueven.");
            } else {
                ui.label("Vista previa");
                for linea in ["Introducción", "Instalación", "Primeros pasos"] {
                    ui.muted(linea);
                }
            }
        });
        ui.separator();
        ui.muted("Arrastra las tarjetas de una columna a otra: las demás hacen sitio.");
        ui.kanban("tablero", &["Por hacer", "En curso", "Hecho"], &mut s.board);
    });
    ui.code(&[
        "ui.splitter(\"divisor\", &mut proporcion, 170.0, |ui, panel| { /* panel 0 o 1 */ });",
        "ui.kanban(\"tablero\", &[\"Por hacer\", \"En curso\", \"Hecho\"], &mut columnas);",
    ]);
}

// ----- Scroll ----------------------------------------------------------------

const REVEALS: [&str; 6] = ["Fade", "Rise", "Left", "Right", "Grow", "Wipe"];

/// Las tarjetas que entran al hacer scroll: un nombre y su color.
const TARJETAS: [(&str, u32); 15] = [
    ("Coral", 0xff7a59),
    ("Lavanda", 0x9b8cff),
    ("Menta", 0x3ccf9e),
    ("Ámbar", 0xf5b83d),
    ("Cielo", 0x4aa8ff),
    ("Ciruela", 0x8e4585),
    ("Lima", 0x9ccc3d),
    ("Arena", 0xd9b382),
    ("Pizarra", 0x64748b),
    ("Rubí", 0xe5484d),
    ("Turquesa", 0x22c3c3),
    ("Miel", 0xe8a33d),
    ("Musgo", 0x6b8e4e),
    ("Índigo", 0x5b5bd6),
    ("Salmón", 0xf59a8b),
];

const AGENDA: [(&str, &[&str]); 5] = [
    ("A", &["Ana", "Alberto", "Alicia", "Andrés", "Aurora", "Antonio"]),
    ("B", &["Beatriz", "Bruno", "Blanca", "Borja", "Belén"]),
    ("C", &["Carmen", "Carlos", "Clara", "Cristina", "César", "Celia"]),
    ("D", &["Daniel", "Diana", "David", "Dolores", "Darío"]),
    ("E", &["Elena", "Emilio", "Eva", "Esteban", "Eugenia"]),
];

const CIUDADES_AGENDA: [&str; 6] = ["Madrid", "Lugo", "Cádiz", "Soria", "Teruel", "Ávila"];

/// Un degradado vertical de `top` a `bottom`, en franjas de unos 4 px que
/// se tocan sin solaparse (solapadas, al desvanecerse se verían rayas).
fn gradient(list: &mut DrawList, r: Rect, top: Color, bottom: Color) {
    let strips = ((r.h / 4.0) as usize).clamp(8, 160);
    let h = r.h / strips as f32;
    for i in 0..strips {
        let (y, next) = ((r.y + h * i as f32).round(), (r.y + h * (i + 1) as f32).round());
        list.rect(Rect::new(r.x, y, r.w, next - y), 0.0, top.mix(bottom, i as f32 / (strips - 1) as f32));
    }
}

/// Una línea de colinas que ondula a lo ancho de `r`, de `base` hacia abajo.
fn hills(list: &mut DrawList, r: Rect, base: f32, height: f32, waves: f32, phase: f32, color: Color) {
    let mut pts = vec![Vec2::new(r.x, r.bottom())];
    for i in 0..=48 {
        let t = i as f32 / 48.0;
        let y = base - height * (0.55 + 0.45 * (t * waves * TAU + phase).sin() * (t * 2.3 + phase).cos());
        pts.push(Vec2::new(r.x + r.w * t, y));
    }
    pts.push(Vec2::new(r.right(), r.bottom()));
    list.polygon(&pts, color);
}

pub fn scroll(ui: &mut Ui, s: &mut State) {
    ui.title("Animaciones de scroll");
    ui.paragraph_muted(
        "Movimiento que depende de dónde está el scroll: lo que entra al verse, números que cuentan, paralaje, cabeceras fijas y viajes a un ancla.",
    );

    ui.scroll_reveal("sec-aparecer", Reveal::Rise, |ui| {
        ui.card("card-aparecer", |ui| {
            ui.row(|ui| {
                ui.label("Aparecer al entrar en pantalla");
                ui.fill();
                if ui.button_with("repetir", "Repetir", Variant::Ghost, Size::Sm).clicked {
                    s.reveal_round += 1;
                }
            });
            ui.segmented("estilo-aparecer", &REVEALS, &mut s.reveal_style);
            ui.paragraph_muted(
                "Haz scroll dentro del recuadro: cada tarjeta entra la primera vez que se ve, y las que llegan a la vez lo hacen una detrás de otra.",
            );
            let (style, round) = (Reveal::ALL[s.reveal_style], s.reveal_round);
            ui.scroll("lista-aparecer", 280.0, |ui| {
                ui.grid(3, |ui| {
                    for (i, (nombre, color)) in TARJETAS.into_iter().enumerate() {
                        ui.scroll_reveal(&format!("tarjeta-{round}-{}-{i}", s.reveal_style), style, |ui| {
                            ui.card(&format!("tarjeta-{i}"), |ui| {
                                let w = ui.available_width();
                                ui.canvas(&format!("muestra-{i}"), Vec2::new(w, 46.0), |list, r| {
                                    list.rect(r, 10.0, Color::hex(color));
                                });
                                ui.label(nombre);
                                ui.muted(&format!("#{color:06x}"));
                            });
                        });
                    }
                });
            });
        });
    });
    ui.code(&[
        "ui.scroll_reveal(\"tarjeta-3\", Reveal::Rise, |ui| { /* el contenido */ });",
        "// las que entran a la vez lo hacen una detrás de otra; el layout no se mueve",
    ]);

    ui.space(8.0);
    ui.scroll_reveal("sec-contadores", Reveal::Rise, |ui| {
        ui.card("card-contadores", |ui| {
            ui.label("Números que cuentan al verse");
            ui.paragraph_muted("Empiezan en cero la primera vez que aparecen y luego se deslizan a cualquier valor nuevo.");
            let cifras: [(f32, usize, &str); 3] = [(12480.0, 0, "descargas este mes"), (98.6, 1, "% del tiempo en reposo"), (4.2, 1, "ms por frame")];
            ui.columns(&[1.0, 1.0, 1.0], |ui, i| {
                let (valor, decimales, texto) = cifras[i];
                ui.count_up(&format!("cifra-{i}"), valor, decimales, 34.0);
                ui.muted(texto);
            });
        });
    });
    ui.code(&["ui.count_up(\"descargas\", 12480.0, 0, 34.0);   // decimales: 0, tamaño: 34"]);

    ui.space(8.0);
    ui.scroll_reveal("sec-paralaje", Reveal::Rise, |ui| {
        ui.label("Paralaje");
        ui.paragraph_muted("Haz scroll en la página: cada capa del paisaje se mueve a una velocidad, y las del fondo se quedan atrás.");
        let alto = 230.0;
        ui.parallax("paisaje", alto, &[0.4, 0.26, 0.14, 0.0], |ui, capa| {
            let w = ui.available_width();
            match capa {
                0 => ui.canvas("cielo", Vec2::new(w, alto + 420.0), |list, r| {
                    gradient(list, r, Color::hex(0x1b2350), Color::hex(0xf39a63));
                    let sol = Vec2::new(r.x + r.w * 0.68, r.center().y + 40.0);
                    list.rect(Rect::new(sol.x - 70.0, sol.y - 70.0, 140.0, 140.0), 70.0, Color::hex(0xffcf7d).with_alpha(0.18));
                    list.rect(Rect::new(sol.x - 38.0, sol.y - 38.0, 76.0, 76.0), 38.0, Color::hex(0xffd98a));
                }),
                1 => ui.canvas("montanas", Vec2::new(w, alto + 280.0), |list, r| {
                    hills(list, r, r.center().y + 70.0, 120.0, 1.6, 0.4, Color::hex(0x6a4f8f));
                }),
                2 => ui.canvas("colinas", Vec2::new(w, alto + 150.0), |list, r| {
                    hills(list, r, r.center().y + 95.0, 70.0, 2.4, 2.0, Color::hex(0x3d2f63));
                }),
                _ => ui.canvas("frente", Vec2::new(w, alto), |list, r| {
                    hills(list, r, r.bottom() + 6.0, 46.0, 3.1, 4.0, Color::hex(0x1f1838));
                }),
            }
        });
    });
    ui.code(&[
        "// capas de atrás hacia delante: 0,4 va a menos de la mitad de velocidad, 0 va con la página",
        "ui.parallax(\"paisaje\", 230.0, &[0.4, 0.26, 0.14, 0.0], |ui, capa| { /* dibuja la capa */ });",
    ]);

    ui.space(8.0);
    ui.scroll_reveal("sec-agenda", Reveal::Rise, |ui| {
        ui.card("card-agenda", |ui| {
            ui.label("Cabeceras fijas, progreso y saltos a un ancla");
            ui.paragraph_muted("La letra se queda arriba mientras pasan sus nombres, hasta que la siguiente la empuja. Los botones viajan a una sección, y la barra de arriba dice cuánto llevas.");
            ui.row(|ui| {
                for (letra, _) in AGENDA {
                    if ui.button_with(&format!("ir-{letra}"), &format!("Ir a {letra}"), Variant::Soft, Size::Sm).clicked {
                        ui.scroll_to(&format!("letra-{letra}"));
                    }
                }
            });
            ui.scroll("agenda", 300.0, |ui| {
                ui.sticky_section(
                    "agenda-entera",
                    |ui| {
                        let p = ui.scroll_progress();
                        ui.row(|ui| {
                            ui.label("Agenda");
                            ui.fill();
                            ui.muted(&format!("{:.0} %", p * 100.0));
                            ui.progress(p, 140.0);
                        });
                    },
                    |ui| {
                        for (letra, nombres) in AGENDA {
                            ui.sticky_section(
                                &format!("letra-{letra}"),
                                |ui| ui.title(letra),
                                |ui| {
                                    for (i, nombre) in nombres.iter().enumerate() {
                                        ui.row(|ui| {
                                            ui.label(nombre);
                                            ui.fill();
                                            ui.muted(CIUDADES_AGENDA[(i + nombre.len()) % CIUDADES_AGENDA.len()]);
                                        });
                                    }
                                },
                            );
                        }
                        if ui.scroll_linked(40.0, 120.0) > 0.5 && ui.button_icon("arriba", Icon::ArrowUp, "Volver arriba", Variant::Ghost, Size::Sm).clicked {
                            ui.scroll_to("agenda-entera");
                        }
                    },
                );
            });
            ui.paragraph_muted("Todas las áreas de scroll rebotan: sigue girando la rueda al llegar arriba o abajo y el contenido se estira un poco antes de volver.");
        });
    });
    ui.code(&[
        "ui.sticky_section(\"letra-B\", |ui| ui.title(\"B\"), |ui| { /* sus filas */ });",
        "ui.scroll_to(\"letra-B\");                      // viaja hasta ella con un muelle",
        "ui.progress(ui.scroll_progress(), 140.0);      // dentro del scroll: 0 arriba, 1 al final",
        "ui.scroll_linked(40.0, 120.0)                  // 0 → 1 mientras se recorren esos píxeles",
    ]);
}

// ----- Transiciones ----------------------------------------------------------

const ESTILOS_TR: [&str; 7] = ["Fade", "Slide", "Shift", "Push", "Zoom", "Flip", "Rise"];

const EXPLICA_TR: [&str; 7] = [
    "Fade: la vista que se va se desvanece mientras llega la otra. Para cambios sin dirección.",
    "Slide: las dos vistas recorren todo el ancho, como las páginas de un carrusel.",
    "Shift: un desplazamiento corto con fundido, en la dirección del avance. Es la que usa esta documentación entre lecciones.",
    "Push: la nueva vista entra por encima y la anterior retrocede oscureciéndose. Al volver, la de arriba se retira.",
    "Zoom: hacia delante, la vieja crece y se desvanece mientras la nueva crece hasta su sitio. Hacia atrás, al revés.",
    "Flip: una tarjeta que se da la vuelta. La vieja se pliega hasta su borde y la nueva se despliega desde él.",
    "Rise: la vieja sube y se desvanece, la nueva sube desde abajo.",
];

/// Los pasos del asistente de ejemplo: título, texto y color.
pub(crate) const PASOS_TR: [(&str, &str, u32); 4] = [
    ("Bienvenida", "Un asistente de cuatro pasos. Avanza y retrocede para ver la coreografía en las dos direcciones.", 0x7c6cff),
    ("Tu perfil", "Cada paso es una vista distinta: la región solo sabe qué número de vista toca ahora.", 0x22c3a6),
    ("Preferencias", "La vista nueva funciona desde el primer frame; la que se va es solo una imagen de cómo estaba.", 0xff7a59),
    ("Listo", "Volver al principio es ir hacia atrás: la transición se invierte sola.", 0xf5b83d),
];

/// Las fotos de la galería: nombre, texto, cielo arriba, horizonte, sol y colinas.
pub(crate) const FOTOS: [(&str, &str, [u32; 5]); 6] = [
    ("Amanecer", "Luz fría que se calienta por abajo.", [0x2b3a67, 0xf6b48f, 0xfff1c9, 0x7a6a9a, 0x463d6b]),
    ("Mediodía", "Cielo limpio y colinas verdes.", [0x3b8fe0, 0xbfe3ff, 0xfffbe6, 0x5fa86b, 0x2f7a4d]),
    ("Atardecer", "El sol bajo tiñe todo de naranja.", [0x1b2350, 0xf39a63, 0xffd98a, 0x6a4f8f, 0x3d2f63]),
    ("Noche", "Una luna sobre montañas oscuras.", [0x070b1f, 0x1f2a55, 0xe8ecff, 0x262c52, 0x121531]),
    ("Niebla", "Capas grises que se pierden a lo lejos.", [0x9aa3ad, 0xdfe3e6, 0xf7f7f2, 0x8b949e, 0x5f6872]),
    ("Aurora", "Verdes y violetas sobre la nieve.", [0x0d1b2a, 0x3ddc97, 0xd6fff0, 0x6c5ce7, 0x2d3561]),
];

/// La foto `i`, pintada en `r`.
fn foto(list: &mut DrawList, r: Rect, i: usize) {
    let [cielo, horizonte, sol, lejos, cerca] = FOTOS[i].2.map(Color::hex);
    list.push(morfo::DrawCmd::PushClip { rect: r, radius: 12.0 });
    gradient(list, r, cielo, horizonte);
    let d = r.h * 0.28;
    let c = Vec2::new(r.x + r.w * (0.3 + 0.08 * i as f32), r.y + r.h * 0.38);
    list.rect(Rect::new(c.x - d * 0.5, c.y - d * 0.5, d, d), d * 0.5, sol);
    hills(list, r, r.y + r.h * 0.78, r.h * 0.32, 1.4, i as f32, lejos);
    hills(list, r, r.y + r.h * 0.95, r.h * 0.22, 2.2, i as f32 * 1.7 + 1.0, cerca);
    list.push(morfo::DrawCmd::PopClip);
}

pub fn transiciones(ui: &mut Ui, s: &mut State) {
    ui.title("Transiciones");
    ui.paragraph_muted(
        "Cuando una zona cambia de vista, la que se va y la que llega se mueven juntas. Lo que las dos comparten vuela de un sitio a otro.",
    );

    ui.card("card-coreografias", |ui| {
        ui.label("Siete coreografías");
        ui.segmented("estilo-tr", &ESTILOS_TR, &mut s.tr_style);
        ui.morph("explica-tr", s.tr_style, |ui| ui.paragraph_muted(EXPLICA_TR[s.tr_style]));
        let paso = s.tr_step;
        ui.transition("asistente", paso, Transition::ALL[s.tr_style], |ui| {
            let (titulo, texto, color) = PASOS_TR[paso];
            ui.card(&format!("paso-tr-{paso}"), |ui| {
                ui.row(|ui| {
                    ui.canvas(&format!("insignia-tr-{paso}"), Vec2::new(44.0, 44.0), |list, r| {
                        list.rect(r, 22.0, Color::hex(color));
                    });
                    ui.column(|ui| {
                        ui.muted(&format!("Paso {} de {}", paso + 1, PASOS_TR.len()));
                        ui.title(titulo);
                    });
                });
                ui.paragraph_muted(texto);
            });
        });
        ui.row(|ui| {
            if paso > 0 && ui.button_icon("tr-atras", Icon::ArrowLeft, "Atrás", Variant::Ghost, Size::Md).clicked {
                s.tr_step -= 1;
            }
            ui.fill();
            if paso + 1 < PASOS_TR.len() {
                if ui.button_icon("tr-siguiente", Icon::ArrowRight, "Siguiente", Variant::Solid, Size::Md).clicked {
                    s.tr_step += 1;
                }
            } else if ui.button_with("tr-otra-vez", "Empezar de nuevo", Variant::Soft, Size::Md).clicked {
                s.tr_step = 0;
            }
        });
    });
    ui.code(&[
        "ui.transition(\"asistente\", paso, Transition::Push, |ui| match paso {",
        "    0 => bienvenida(ui),",
        "    _ => perfil(ui),",
        "});  // un número mayor es avanzar: entra por la derecha",
    ]);

    ui.space(8.0);
    ui.card("card-galeria", |ui| {
        ui.label("Un elemento que comparten dos vistas");
        ui.paragraph_muted("Abre una foto: vuela desde la galería hasta la vista de detalle, y vuelve a su sitio al cerrarla.");
        let vista = s.photo.map_or(0, |i| i + 1);
        let photo = s.photo;
        ui.transition("galeria", vista, Transition::Zoom, |ui| match photo {
            None => ui.grid(3, |ui| {
                for (i, (nombre, _, _)) in FOTOS.iter().enumerate() {
                    ui.column(|ui| {
                        let w = ui.available_width();
                        ui.hero(&format!("foto-{i}"), |ui| {
                            ui.canvas(&format!("mini-{i}"), Vec2::new(w, 96.0), |list, r| foto(list, r, i));
                        });
                        if ui.button_with(&format!("abrir-{i}"), nombre, Variant::Ghost, Size::Sm).clicked {
                            s.photo = Some(i);
                        }
                    });
                }
            }),
            Some(i) => {
                let (nombre, texto, _) = FOTOS[i];
                ui.row(|ui| {
                    if ui.button_icon("cerrar-foto", Icon::ArrowLeft, "Galería", Variant::Ghost, Size::Sm).clicked {
                        s.photo = None;
                    }
                    ui.fill();
                    ui.muted(&format!("Foto {} de {}", i + 1, FOTOS.len()));
                });
                let w = ui.available_width();
                ui.hero(&format!("foto-{i}"), |ui| {
                    ui.canvas(&format!("grande-{i}"), Vec2::new(w, 250.0), |list, r| foto(list, r, i));
                });
                ui.title(nombre);
                ui.paragraph_muted(texto);
            }
        });
    });
    ui.code(&[
        "// en la galería y en el detalle, con la misma clave",
        "ui.hero(\"foto-3\", |ui| ui.canvas(\"mini-3\", tamaño, pintar));",
        "ui.hero(\"foto-3\", |ui| ui.canvas(\"grande-3\", tamaño_grande, pintar));",
    ]);

    ui.space(8.0);
    ui.card("card-lecciones", |ui| {
        ui.label("Entre lecciones");
        ui.paragraph_muted(
            "Esta documentación pone todas sus páginas dentro de una transición Shift, con el número de lección como vista: la siguiente lección llega desde la derecha y la anterior desde la izquierda. El scroll de cada página se conserva al volver a ella.",
        );
    });
    ui.code(&["ui.transition(\"paginas\", leccion, Transition::Shift, |ui| pagina(ui, leccion));"]);
}

// ----- Composición -----------------------------------------------------------

const PROYECTOS: [(&str, &str); 3] = [
    ("Tienda en línea", "12 tareas abiertas · actualizado hoy"),
    ("Aplicación de notas", "3 tareas abiertas · hace dos días"),
    ("Web del estudio", "Terminado · el mes pasado"),
];

const EQUIPO: [&[&str]; 4] = [&["Ana", "Diseño", "80"], &["Bruno", "Código", "45"], &["Carmen", "Textos", "100"], &["David", "Pruebas", "20"]];

const TEMAS: [(&str, &str, u32); 3] = [
    ("Primeros pasos", "Instalar, el primer frame y la primera ventana.", 4),
    ("Componentes", "Botones, campos, tablas y todo lo demás.", 18),
    ("Adaptación", "Cómo aprende la interfaz de quien la usa.", 6),
];

pub fn composicion(ui: &mut Ui, s: &mut State) {
    ui.title("Componentes dentro de componentes");
    ui.paragraph_muted(
        "Un contenedor es dueño de lo que contiene: sus claves, si responde, cómo se ve. Así un componente hecho de otros se puede repetir, pulsar, apagar o vestir de una vez.",
    );

    ui.card("card-claves", |ui| {
        ui.label("Cada contenedor tiene su espacio de claves");
        ui.paragraph_muted(
            "Las dos tarjetas usan la clave «borrar» para su botón, y son dos botones distintos: lo que declara una tarjeta, un diálogo, un scroll o cualquier contenedor con clave queda bajo la clave del contenedor. Para encontrar algo anidado sin escribir la ruta, ui.find(\"borrar\").",
        );
        ui.grid(2, |ui| {
            for i in 0..2 {
                ui.card(&format!("nota-{i}"), |ui| {
                    ui.label(&format!("Nota {}", i + 1));
                    if ui.button_with("borrar", "Borrar", Variant::Soft, Size::Sm).clicked {
                        ui.toast(&format!("Borrada la nota {}", i + 1));
                    }
                });
            }
        });
    });
    ui.code(&[
        "for i in 0..2 {",
        "    ui.card(&format!(\"nota-{i}\"), |ui| {",
        "        if ui.button(\"borrar\", \"Borrar\").clicked { /* solo esta nota */ }",
        "    });",
        "}",
        "let segundo = ui.id(\"nota-1\").child(\"borrar\");   // o ui.find_all(\"borrar\")[1]",
    ]);

    ui.space(8.0);
    ui.title("Un bloque entero que se pulsa");
    ui.paragraph_muted("Cualquier contenido se comporta como un botón: se eleva bajo el ratón, se hunde al pulsarlo y llega con Tab. La estrella de dentro sigue siendo suya: pulsarla no abre el proyecto.");
    for (i, (nombre, detalle)) in PROYECTOS.iter().enumerate() {
        let r = ui.pressable(&format!("proyecto-{i}"), &format!("Abrir {nombre}"), |ui| {
            ui.card("tarjeta", |ui| {
                ui.row(|ui| {
                    ui.column(|ui| {
                        ui.label(nombre);
                        ui.muted(detalle);
                    });
                    ui.fill();
                    let estrella = if s.favs[i] { Variant::Soft } else { Variant::Ghost };
                    if ui.icon_button("favorito", Icon::Star, estrella, Size::Sm).clicked {
                        s.favs[i] = !s.favs[i];
                    }
                });
            });
        });
        if r.clicked {
            ui.toast(&format!("Abriste «{nombre}»"));
        }
    }
    ui.code(&[
        "if ui.pressable(\"proyecto\", \"Abrir el proyecto\", |ui| {",
        "    ui.label(nombre);",
        "    if ui.icon_button(\"favorito\", Icon::Star, Variant::Ghost, Size::Sm).clicked {",
        "        /* solo la estrella */",
        "    }",
        "}).clicked { abrir(); }",
    ]);

    ui.space(8.0);
    ui.title("Apagar una parte entera");
    ui.card("card-pagos", |ui| {
        ui.row(|ui| {
            ui.label("Cuenta verificada");
            ui.fill();
            ui.toggle("verificada", &mut s.verified);
        });
        ui.paragraph_muted("Sin verificar, todo el bloque de pagos se atenúa, deja de responder y Tab lo salta, con lo que tenga dentro.");
        ui.enabled(s.verified, |ui| {
            ui.card("pagos", |ui| {
                ui.label("Pagos");
                ui.text_field("iban", &mut s.iban, "IBAN", 320.0);
                ui.row(|ui| {
                    ui.chip("mensual", "Cobro mensual", &mut s.autosave);
                    ui.fill();
                    if ui.button("guardar-pagos", "Guardar").clicked {
                        ui.toast("Pagos guardados");
                    }
                });
            });
        });
    });
    ui.code(&["ui.enabled(verificada, |ui| ui.card(\"pagos\", |ui| { /* campos, botones… */ }));"]);

    ui.space(8.0);
    ui.title("Contenido en vez de texto");
    ui.paragraph_muted(
        "Pestañas, menús, tablas y acordeones aceptan, además de texto, lo que construyas para cada pestaña, opción, celda o cabecera.",
    );
    ui.card("card-contenido", |ui| {
        let sin_leer = [4u32, 0, 2];
        ui.tabs_with("bandejas", &["Entrada", "Enviados", "Borradores"], &mut s.inbox_tab, |ui, i, _| {
            ui.label(["Entrada", "Enviados", "Borradores"][i]);
            if sin_leer[i] > 0 {
                ui.badge("cuenta", sin_leer[i]);
            }
        });
        ui.space(4.0);
        ui.row(|ui| {
            ui.muted("Un menú con atajos:");
            if let Some(i) = ui.menu_with("editar", "Editar", &[("copiar", "Copiar"), ("pegar", "Pegar"), ("buscar", "Buscar")], |ui, i| {
                ui.label(["Copiar", "Pegar", "Buscar"][i]);
                ui.fill();
                ui.muted(["Ctrl+C", "Ctrl+V", "Ctrl+F"][i]);
            }) {
                ui.toast(&format!("Elegiste {}", ["Copiar", "Pegar", "Buscar"][i]));
            }
        });
        ui.space(4.0);
        let done = &mut s.done;
        ui.table_with("equipo", &["Persona", "Área", "Avance", ""], &EQUIPO, &mut s.team_sort, |ui, r, c| match c {
            0 => ui.label(EQUIPO[r][0]),
            1 => ui.muted(EQUIPO[r][1]),
            2 => {
                let v: f32 = EQUIPO[r][2].parse().unwrap_or(0.0);
                ui.progress(v / 100.0, 90.0);
                ui.muted(&format!("{v:.0} %"));
            }
            _ => {
                if ui.toggle_button("hecho", if done[r] { "Revisado" } else { "Revisar" }, &mut done[r], Variant::Soft, Size::Sm) {
                    ui.toast(if done[r] { "Marcado como revisado" } else { "Pendiente de revisar" });
                }
            }
        });
        ui.space(4.0);
        ui.accordion_with(
            "temas",
            &["Primeros pasos", "Componentes", "Adaptación"],
            &mut s.topic,
            |ui, i, abierto| {
                ui.column(|ui| {
                    ui.label(TEMAS[i].0);
                    if !abierto {
                        ui.muted(TEMAS[i].1);
                    }
                });
                ui.fill();
                ui.badge("lecciones", TEMAS[i].2);
            },
            |ui, i| ui.paragraph_muted(&format!("{} Son {} lecciones.", TEMAS[i].1, TEMAS[i].2)),
        );
    });
    ui.code(&[
        "ui.tabs_with(\"bandejas\", &nombres, &mut actual, |ui, i, elegida| {",
        "    ui.label(nombres[i]);",
        "    ui.badge(\"n\", sin_leer[i]);",
        "});",
        "ui.menu_with(\"editar\", \"Editar\", &opciones, |ui, i| { /* la opción i */ });",
        "ui.table_with(\"equipo\", &cabeceras, &filas, &mut orden, |ui, fila, col| { /* celda */ });",
        "ui.accordion_with(\"temas\", &nombres, &mut abierta, |ui, i, abierta| { /* cabecera */ },",
        "    |ui, i| { /* cuerpo */ });",
    ]);

    ui.space(8.0);
    ui.title("Estilo que se hereda");
    ui.paragraph_muted("Tamaño, color de acento y variante de los botones se dan una vez al bloque y lo toman todos los componentes de dentro.");
    ui.card("card-estilo", |ui| {
        ui.row(|ui| {
            ui.muted("Tamaño");
            ui.segmented("tam", &["Sm", "Md", "Lg"], &mut s.preview_size);
            ui.fill();
            ui.muted("Acento");
            let colores: Vec<Color> = ACCENTS.iter().map(|&c| Color::hex(c)).collect();
            ui.swatches("acento-bloque", &colores, &mut s.preview_accent);
        });
        let size = Size::ALL[s.preview_size];
        ui.with_size(size, |ui| {
            ui.with_accent(Color::hex(ACCENTS[s.preview_accent]), |ui| {
                ui.card("muestra", |ui| {
                    ui.label("Un bloque con su propio estilo");
                    ui.row(|ui| {
                        ui.toggle("muestra-on", &mut s.preview_on);
                        ui.slider("muestra-nivel", &mut s.level, 180.0);
                        ui.button("muestra-ok", "Aceptar");
                    });
                    ui.with_variant(Variant::Ghost, |ui| {
                        ui.row(|ui| {
                            ui.button("muestra-a", "Copiar");
                            ui.button("muestra-b", "Mover");
                            ui.button("muestra-c", "Compartir");
                        });
                    });
                });
            });
        });
    });
    ui.code(&[
        "ui.with_size(Size::Lg, |ui| {",
        "    ui.with_accent(verde, |ui| { /* todo lo de dentro, más grande y verde */ });",
        "});",
        "ui.with_variant(Variant::Ghost, |ui| { ui.button(\"copiar\", \"Copiar\"); /* … */ });",
    ]);
}
