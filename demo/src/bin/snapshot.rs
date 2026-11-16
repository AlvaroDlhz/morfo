//! Headless run of the documentation app: simulates a user at 60 fps, saves PNG frames
//! to `out/` and reports how much work the library actually did.

use std::time::{Duration, Instant};

use morfo::{DrawCmd, Id, Key, Theme, Ui, Vec2};
use morfo_demo::{build, State, WINDOW};
use morfo_skia::{tiny_skia::Pixmap, Renderer};

const SIZE: Vec2 = WINDOW;
const SCALE: f32 = 2.0;

struct Sim {
    ui: Ui,
    state: State,
    renderer: Renderer,
    pixmap: Pixmap,
    time: f64,
    frames: u32,
    idle: u32,
    build: Duration,
    paint: Duration,
}

impl Sim {
    /// One simulated frame; returns whether the UI wants another.
    fn frame(&mut self) -> bool {
        self.time += 1.0 / 60.0;
        let t = Instant::now();
        self.ui.begin(self.time, SIZE);
        build(&mut self.ui, &mut self.state);
        let out = self.ui.end();
        self.build += t.elapsed();
        self.frames += 1;
        self.idle += !out.animating as u32;
        out.animating
    }

    fn frames(&mut self, n: u32) {
        for _ in 0..n {
            self.frame();
        }
    }

    /// Runs until the interface is fully at rest; returns the frames it took.
    fn settle(&mut self) -> u32 {
        let mut n = 0;
        while self.frame() {
            n += 1;
            assert!(n < 2000, "the interface never came to rest");
        }
        n
    }

    /// Brings the pointer to the left edge and waits for the index to unfold.
    fn open_index(&mut self) {
        self.ui.pointer_moved(Vec2::new(3.0, SIZE.y * 0.5));
        self.settle();
    }

    /// Opens documentation page `i` from the course index and waits for it.
    fn go(&mut self, i: u64) {
        self.open_index();
        let lesson = morfo_demo::lesson_of(i as usize) as u64;
        let row = self.find("indice").with(lesson);
        // The syllabus is taller than the window: scroll it to the lesson.
        for _ in 0..20 {
            let r = self.ui.rect_of(row).expect("lesson in the index");
            let dy = if r.bottom() > SIZE.y - 24.0 {
                -(r.bottom() - SIZE.y + 120.0)
            } else if r.y < 80.0 {
                120.0 - r.y
            } else {
                break;
            };
            self.ui.pointer_moved(Vec2::new(60.0, SIZE.y * 0.5));
            self.ui.scroll_wheel(dy);
            self.settle();
        }
        self.click(row);
        self.ui.pointer_left();
        self.settle();
    }

    /// The element declared with `key` in the last frame, wherever it is
    /// nested (inside cards, scroll areas, the page's transition).
    fn find(&self, key: &str) -> Id {
        self.ui.find(key).unwrap_or_else(|| panic!("nothing was declared with the key «{key}»"))
    }

    /// Where the text `text` is drawn, if it is.
    fn text_y(&self, text: &str) -> Option<f32> {
        let list = self.ui.draw_list();
        list.commands().iter().find_map(|c| match *c {
            DrawCmd::Text { pos, start, len, .. } if list.text(start, len) == text => Some(pos.y),
            _ => None,
        })
    }

    fn click(&mut self, id: Id) {
        let r = self.ui.rect_of(id).expect("element not on screen");
        self.ui.pointer_moved(r.center());
        self.frames(4);
        self.ui.pointer_pressed();
        self.frames(3);
        self.ui.pointer_released();
        self.frames(2);
    }

    /// Runs `n` frames painting every one, as a window would, and reports
    /// what the painting cost.
    fn measure(&mut self, what: &str, n: u32) {
        let (mut total, mut worst, mut rows, mut painted) = (Duration::ZERO, Duration::ZERO, 0u64, 0u32);
        for _ in 0..n {
            self.frame();
            let t = Instant::now();
            let damage = self.renderer.render(self.ui.draw_list(), &mut self.pixmap, SCALE, self.ui.background());
            let dt = t.elapsed();
            total += dt;
            worst = worst.max(dt);
            if let Some((a, b)) = damage {
                rows += (b - a) as u64;
                painted += 1;
            }
        }
        let height = self.pixmap.height() as u64;
        println!(
            "  {what:<34} avg {:>8.2?}  worst {:>8.2?}  {painted}/{n} frames painted, {:.0}% of the rows each",
            total / n,
            worst,
            if painted == 0 { 0.0 } else { rows as f64 * 100.0 / (painted as u64 * height) as f64 },
        );
    }

    fn save(&mut self, name: &str) {
        let t = Instant::now();
        self.renderer.render(self.ui.draw_list(), &mut self.pixmap, SCALE, self.ui.background());
        self.paint = t.elapsed();
        std::fs::create_dir_all("out").unwrap();
        self.pixmap.save_png(format!("out/{name}.png")).unwrap();
        println!("  out/{name}.png  ({} draw commands, painted in {:.2?})", self.ui.draw_list().len(), self.paint);
    }
}

fn main() {
    let renderer = morfo_demo::renderer();
    let mut ui = Ui::new(Theme::dark());
    ui.set_text_measure(Box::new(renderer.measure()));
    // Short memory so the demo shows habits fading too.
    ui.profile_mut().half_life = 120.0;
    let mut s = Sim {
        ui,
        state: State::default(),
        renderer,
        pixmap: Pixmap::new((SIZE.x * SCALE) as u32, (SIZE.y * SCALE) as u32).unwrap(),
        time: 0.0,
        frames: 0,
        idle: 0,
        build: Duration::ZERO,
        paint: Duration::ZERO,
    };

    println!("entrance settled in {} frames", s.settle());
    s.save("01-inicio");

    // The side index: the rail at the edge unfolds into a panel.
    s.ui.pointer_moved(Vec2::new(3.0, SIZE.y * 0.5));
    s.frames(6);
    s.save("01b-indice-desplegandose");
    s.settle();
    s.ui.pointer_moved(s.ui.rect_of(s.find("indice").with(6)).unwrap().center());
    s.settle();
    s.save("01c-indice-abierto");
    s.ui.pointer_left();
    s.time += 1.0;
    s.settle();

    // The search at the top right: the magnifier unfolds into a field.
    let lupa = s.find("buscar").child("boton");
    s.click(lupa);
    s.ui.pointer_left();
    s.frames(6);
    s.save("01d-buscador-desplegandose");
    s.settle();
    s.ui.text_input("ent");
    s.frame();
    s.settle();
    s.save("01e-buscador");
    s.ui.key_pressed(Key::Enter);
    s.frame();
    s.settle();
    assert_eq!(s.state.tab, 7, "the search took us to «Entrada»");
    // The lesson footer: «Siguiente» goes on through the course.
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(-20000.0);
    s.settle();
    s.click(s.find("siguiente"));
    s.ui.pointer_left();
    s.settle();
    assert_eq!(s.state.tab, 13, "after «Entrada» comes «Formularios»");
    s.go(0);

    // Adaptive page: build a habit, then watch the toolbar reshape itself.
    s.go(8);
    let action = |s: &Sim, key: &str| s.find("actions").child(key);
    for _ in 0..5 {
        s.click(action(&s, "exportar"));
    }
    // Nothing moved while the pointer was there; now it adapts.
    s.ui.pointer_left();
    s.frames(9);
    s.save("02-morph-en-curso");
    for _ in 0..3 {
        s.click(action(&s, "compartir"));
    }
    for _ in 0..3 {
        s.click(action(&s, "nuevo"));
        s.click(action(&s, "abrir"));
    }
    s.ui.pointer_left();
    println!("adaptation settled in {} frames", s.settle());
    s.save("03-adaptado");
    println!("profile is {} bytes of text", s.ui.profile().encode().len());

    // Adaptive menu: use two entries a lot; the rest fold under "Más…".
    let file = s.find("archivo");
    for key in ["guardar", "guardar", "guardar", "guardar", "exportar", "exportar", "exportar"] {
        s.click(file);
        s.settle();
        s.click(file.child(key));
        s.settle();
    }
    s.click(file);
    s.settle();
    s.save("04-menu-adaptativo");
    s.ui.key_pressed(Key::Escape);
    s.settle();

    s.go(1);
    s.click(s.find("camaleon"));
    s.ui.pointer_left();
    s.settle();
    s.save("05-botones");

    // The five extra kinds, in every variant: scroll down to them.
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(-520.0);
    s.settle();
    s.save("05b-tipos-de-boton");
    s.click(s.find("t-conmutable-0"));
    s.click(s.find("t-carga-1"));
    s.frames(14);
    s.save("05c-cargando-y-conmutado");
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(-420.0);
    s.settle();
    let r = s.ui.rect_of(s.find("t-mantener-2")).unwrap();
    s.ui.pointer_moved(r.center());
    s.frame();
    s.ui.pointer_pressed();
    s.frames(36);
    s.save("05d-manteniendo");
    s.frames(40);
    s.ui.pointer_released();
    s.ui.pointer_left();
    s.time += 3.0;
    s.settle();
    s.click(s.find("btn-size").with(2));
    s.ui.pointer_left();
    s.settle();
    s.save("05e-tipos-grandes");
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(5000.0);
    s.ui.pointer_left();
    s.settle();

    s.go(3);
    s.click(s.find("autosave"));
    s.click(s.find("plan-equipo"));
    s.click(s.find("tag-web"));
    s.click(s.find("seats").with(1));
    s.frames(3);
    s.click(s.find("faq").with(2));
    s.frames(4);
    s.save("06-seleccion-en-movimiento");
    s.ui.pointer_left();
    s.settle();
    s.save("07-seleccion");

    s.go(6);
    s.click(s.find("toast-btn"));
    s.click(s.find("opciones"));
    s.settle();
    s.save("08-menu-y-aviso");
    s.click(s.find("opciones").child("duplicar"));
    s.settle();
    // Rest the pointer on the button until its tooltip is due.
    let r = s.ui.rect_of(s.find("dlg-open")).unwrap();
    s.ui.pointer_moved(r.center());
    s.frames(40);
    s.settle();
    s.save("09-tooltip");
    s.click(s.find("dlg-open"));
    s.frames(7);
    s.save("10-dialogo-abriendose");
    s.settle();
    s.save("11-dialogo");
    s.click(s.find("dlg-ok"));
    s.ui.pointer_left();
    s.settle();

    s.go(7);
    s.click(s.find("name"));
    s.ui.text_input("Álvaro");
    s.frame();
    s.ui.key_pressed(Key::Tab);
    s.frame();
    s.ui.text_input("Madrid");
    s.frame();
    let r = s.ui.rect_of(s.find("lista")).unwrap();
    s.ui.pointer_moved(r.center());
    s.ui.scroll_wheel(-700.0);
    s.frames(8);
    s.save("12-entrada-y-scroll");
    s.ui.key_pressed(Key::Escape);
    s.ui.pointer_left();
    // Let the toasts expire, as the host would on `wake_in`.
    s.time += 4.0;
    s.settle();

    // Wrapped text, text area, select and combobox, further down.
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(-760.0);
    s.settle();
    s.click(s.find("notas"));
    s.ui.text_input("Primera línea de notas, bastante larga para que el área tenga que partirla en dos.");
    s.frame();
    s.ui.key_pressed(Key::Enter);
    s.frame();
    s.ui.text_input("Segunda línea.");
    s.frames(3);
    s.save("12b-area-de-texto");
    s.ui.key_pressed(Key::Escape);
    s.frame();
    s.click(s.find("plan-select"));
    s.settle();
    s.save("12c-desplegable");
    s.click(s.find("plan-select").child("option").with(2));
    s.settle();
    s.click(s.find("ciudad"));
    s.ui.text_input("mala");
    s.frame();
    s.settle();
    s.save("12d-combobox");
    s.ui.key_pressed(Key::Enter);
    s.frame();
    s.ui.pointer_left();
    s.time += 4.0;
    s.settle();
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(5000.0);
    s.ui.pointer_left();
    s.settle();

    // Data: sort the table by price, then drag a task.
    s.go(10);
    s.click(s.find("productos").child("header").with(2));
    s.frames(8);
    s.save("12e-tabla-ordenandose");
    s.ui.pointer_left();
    s.settle();
    s.save("12f-tabla-por-precio");
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(-600.0);
    s.settle();
    let task = s.ui.rect_of(s.find("tareas").child("Revisar los textos")).unwrap();
    s.ui.pointer_moved(task.center());
    s.frame();
    s.ui.pointer_pressed();
    for i in 1..=10 {
        s.ui.pointer_moved(task.center() + Vec2::new(0.0, 14.0 * i as f32));
        s.frame();
    }
    s.save("12g-arrastrando-tarea");
    s.ui.pointer_released();
    s.frame();
    s.ui.pointer_left();
    s.settle();
    s.save("12h-lista-reordenada");

    // The command palette: type, then run with Enter.
    s.state.palette = true;
    s.frames(4);
    s.ui.text_input("selec");
    s.frames(20);
    s.save("12i-paleta");
    s.ui.key_pressed(Key::Enter);
    s.frame();
    s.settle();
    assert_eq!(s.state.tab, 3, "the palette took us to «Selección»");

    s.go(2);
    s.click(s.find("light"));
    let r = s.ui.rect_of(s.find("level")).unwrap();
    s.ui.pointer_moved(Vec2::new(r.x + r.w * 0.4, r.center().y));
    s.frame();
    s.ui.pointer_pressed();
    s.frame();
    s.ui.pointer_moved(Vec2::new(r.x + r.w * 0.85, r.center().y));
    s.frames(10);
    s.save("13-controles-en-movimiento");
    s.ui.pointer_released();
    s.settle();
    s.save("14-controles-claro");

    // The five extra controls, further down the page.
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(-560.0);
    s.settle();
    let r = s.ui.rect_of(s.find("rango-doble")).unwrap();
    let thumb = |v: f32| Vec2::new(r.x + 12.0 + (r.w - 24.0) * v, r.center().y);
    s.ui.pointer_moved(thumb(0.7));
    s.frame();
    s.ui.pointer_pressed();
    for i in 0..8 {
        s.ui.pointer_moved(thumb(0.7 + 0.025 * i as f32));
        s.frame();
    }
    s.ui.pointer_released();
    let k = s.ui.rect_of(s.find("volumen")).unwrap();
    let c = Vec2::new(k.center().x, k.y + 30.0);
    s.ui.pointer_moved(c);
    s.frame();
    s.ui.pointer_pressed();
    for i in 0..8 {
        s.ui.pointer_moved(c - Vec2::new(0.0, 6.0 * i as f32));
        s.frame();
    }
    s.ui.pointer_released();
    let pad = s.ui.rect_of(s.find("pad")).unwrap();
    s.ui.pointer_moved(pad.center());
    s.frame();
    s.ui.pointer_pressed();
    for i in 0..8 {
        s.ui.pointer_moved(pad.center() + Vec2::new(9.0 * i as f32, -5.0 * i as f32));
        s.frame();
    }
    s.ui.pointer_released();
    let stars = s.ui.rect_of(s.find("estrellas")).unwrap();
    s.ui.pointer_moved(Vec2::new(stars.x + stars.w * 0.9, stars.center().y));
    s.frames(6);
    s.save("14b-mas-controles");
    s.ui.pointer_pressed();
    s.frame();
    s.ui.pointer_released();
    s.frame();
    s.click(s.find("acento").with(1));
    s.frames(12);
    s.save("14c-acento-cambiando");
    s.ui.pointer_left();
    s.settle();
    s.save("14d-acento-verde");
    s.click(s.find("acento").with(0));
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(5000.0);
    s.ui.pointer_left();
    s.settle();

    s.go(5);
    s.click(s.find("morph"));
    s.frames(9);
    s.save("15-forma-en-morph");
    s.ui.pointer_left();
    s.settle();

    // Mutation: one component turning into another.
    s.go(9);
    s.click(s.find("buscador").child("abrir"));
    s.frames(7);
    s.save("17-boton-mutando-a-campo");
    s.settle();
    s.ui.text_input("morfo");
    s.settle();
    s.save("18-buscador");
    s.click(s.find("forma").with(4));
    s.ui.pointer_left();
    s.frames(9);
    s.save("19-mutando-a-tarjeta");
    s.settle();
    s.save("20-tarjeta");
    // The last example is below the fold: scroll the page down to it.
    s.ui.pointer_moved(Vec2::new(400.0, 600.0));
    s.ui.scroll_wheel(-2000.0);
    s.settle();
    let export = s.find("exportar-forma").child("normal");
    for _ in 0..4 {
        s.click(export);
    }
    s.ui.pointer_left();
    s.frames(10);
    s.save("21-boton-creciendo-a-tarjeta");
    s.settle();
    s.time += 4.0;
    s.settle();
    s.save("22-destacado");
    s.go(5);

    // Richer layout, free drawing and keyboard focus.
    s.go(4);
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(-5000.0);
    s.settle();
    s.ui.pointer_left();
    s.settle();
    s.save("23-rejilla-y-columnas");
    s.go(5);
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(-600.0);
    s.settle();
    s.ui.pointer_left();
    s.settle();
    s.save("24-dibujo-libre");
    s.go(1);
    for _ in 0..4 {
        s.ui.key_pressed(Key::Tab);
        s.frame();
    }
    s.frames(6);
    s.save("25-foco-viajando");
    s.settle();
    s.save("26-foco-con-teclado");
    s.ui.key_pressed(Key::Escape);
    s.ui.pointer_moved(Vec2::new(450.0, 300.0));
    s.ui.pointer_pressed();
    s.frame();
    s.ui.pointer_released();
    s.ui.pointer_left();
    s.settle();

    // The adaptive layer: the next step, the home screen, the habits panel.
    s.go(8);
    let bar = s.find("actions");
    for _ in 0..3 {
        s.click(bar.child("nuevo"));
        s.click(bar.child("abrir"));
        s.ui.pointer_left();
        s.time += 20.0;
        s.settle();
    }
    s.click(bar.child("nuevo"));
    s.ui.pointer_left();
    s.settle();
    s.save("27-siguiente-paso");
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(-520.0);
    s.settle();
    for _ in 0..5 {
        s.click(s.find("abrir-4"));
    }
    s.ui.pointer_left();
    s.settle();
    s.save("28-inicio-adaptativo");
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(-820.0);
    s.settle();
    s.click(s.find("densidad"));
    s.ui.pointer_left();
    s.settle();
    s.save("29-tus-habitos");
    s.click(s.find("densidad"));
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(5000.0);
    s.settle();
    s.ui.pointer_left();
    s.settle();

    // Exit animations on the «Cajas» page.
    s.go(4);
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(-5000.0);
    s.settle();
    s.click(s.find("cerrar-1"));
    s.frames(6);
    s.save("30-aviso-saliendo");
    s.ui.pointer_left();
    s.settle();
    s.save("31-aviso-cerrado");

    // The newer pages, in the states a still capture cannot reach.
    s.go(13);
    s.click(s.find("fecha"));
    s.ui.pointer_left();
    s.settle();
    s.save("33-calendario");
    s.click(s.find("fecha").child("siguiente"));
    s.frames(6);
    s.save("34-calendario-cambiando-de-mes");
    s.settle();
    s.click(s.find("fecha").child("day").with(3));
    s.settle();
    assert_eq!((s.state.date.day, s.state.date.month == 12 || s.state.date.month == 1), (3, true), "a day of the next month");
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(-450.0);
    s.settle();
    s.click(s.find("lenguajes"));
    s.ui.text_input("py");
    s.frame();
    s.settle();
    s.save("35-varias-opciones");
    s.ui.key_pressed(Key::Enter);
    s.frame();
    s.ui.key_pressed(Key::Escape);
    s.frame();
    s.settle();
    assert!(s.state.langs.contains(&2), "«Python» was chosen with the keyboard");

    s.go(12);
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(-20000.0);
    s.settle();
    s.click(s.find("abrir-cajon"));
    s.ui.pointer_left();
    s.frames(8);
    s.save("36-panel-lateral-entrando");
    s.settle();
    s.save("37-panel-lateral");
    s.ui.key_pressed(Key::Escape);
    s.frame();
    s.settle();
    assert!(!s.state.drawer);

    s.go(11);
    s.ui.pointer_moved(s.ui.rect_of(s.find("barras").child("Sep")).unwrap().center());
    s.settle();
    s.save("38-valor-de-barra");
    s.click(s.find("serie").with(1));
    s.ui.pointer_left();
    s.frames(10);
    s.save("39-graficos-cambiando");
    s.settle();
    s.save("40-graficos-otro-ano");

    s.go(14);
    s.ui.pointer_moved(Vec2::new(450.0, 500.0));
    s.ui.scroll_wheel(-20000.0);
    s.settle();
    let card = s.ui.rect_of(s.find("tablero").child("Escribir la guía")).unwrap();
    let lane = s.ui.rect_of(s.find("tablero").child("lane").with(2)).unwrap();
    s.ui.pointer_moved(card.center());
    s.frames(2);
    s.ui.pointer_pressed();
    s.frames(2);
    let to = Vec2::new(lane.center().x, lane.y + 110.0);
    for k in 1..=12 {
        s.ui.pointer_moved(card.center() + (to - card.center()) * (k as f32 / 12.0));
        s.frame();
    }
    s.frames(10);
    s.save("41-arrastrando-tarjeta");
    s.ui.pointer_released();
    s.frames(6);
    s.save("42-tarjeta-soltandose");
    s.settle();
    assert_eq!(s.state.board[2].last().map(String::as_str), Some("Escribir la guía"), "the card moved to «Hecho»");
    s.ui.pointer_left();
    s.settle();

    // Scroll: cards that enter as they come into view, the parallax
    // landscape, pinned headers and the travel to a section.
    s.go(15);
    s.save("43-scroll");
    let list = s.ui.rect_of(s.find("lista-aparecer")).unwrap();
    s.ui.pointer_moved(list.center());
    s.ui.scroll_wheel(-260.0);
    s.frames(14);
    s.save("44-tarjetas-entrando");
    s.settle();
    s.ui.pointer_moved(Vec2::new(450.0, 130.0));
    s.ui.scroll_wheel(-640.0);
    s.settle();
    s.save("45-paralaje");
    s.ui.scroll_wheel(-20000.0);
    s.settle();
    s.click(s.find("ir-C"));
    s.ui.pointer_left();
    s.frames(6);
    s.save("46-viajando-a-una-seccion");
    s.settle();
    s.save("47-cabeceras-fijas");
    let agenda = s.ui.rect_of(s.find("agenda")).unwrap();
    let c = s.text_y("C").expect("the header «C» is drawn");
    assert!(c > agenda.y && c < agenda.y + 60.0, "«C» pinned under the progress bar: {c} in {agenda:?}");

    // Transitions: a step of the wizard on its way, and a photo flying
    // from the gallery to its detail view and back.
    s.go(16);
    s.save("48-transiciones");
    s.click(s.find("tr-siguiente"));
    s.ui.pointer_left();
    s.frames(4);
    s.save("49-transicion-push");
    s.settle();
    assert_eq!(s.state.tr_step, 1);
    s.state.tr_style = 5;
    s.settle();
    s.click(s.find("tr-siguiente"));
    s.frames(5);
    s.save("50-transicion-flip");
    s.settle();
    s.ui.pointer_moved(Vec2::new(450.0, 130.0));
    s.ui.scroll_wheel(-480.0);
    s.settle();
    s.click(s.find("abrir-2"));
    s.ui.pointer_left();
    s.frames(5);
    s.save("51-foto-volando");
    s.settle();
    s.save("52-foto-abierta");
    assert_eq!(s.state.photo, Some(2));
    s.click(s.find("cerrar-foto"));
    s.ui.pointer_left();
    s.frames(5);
    s.save("53-foto-volviendo");
    s.settle();
    assert_eq!(s.state.photo, None);

    // Components inside components: a pressable block whose star is its
    // own, a disabled part, content in a menu and a table, inherited style.
    s.go(17);
    s.save("54-composicion");
    s.ui.pointer_moved(Vec2::new(450.0, 130.0));
    s.ui.scroll_wheel(-380.0);
    s.settle();
    let star = s.ui.find_all("favorito")[1];
    s.click(star);
    s.settle();
    assert!(s.state.favs[1], "the star inside the block toggled");
    s.ui.pointer_moved(s.ui.rect_of(s.find("proyecto-2")).unwrap().center());
    s.settle();
    s.save("55-bloque-pulsable");
    s.ui.pointer_moved(Vec2::new(450.0, 130.0));
    s.ui.scroll_wheel(-360.0);
    s.settle();
    s.click(s.find("guardar-pagos"));
    s.settle();
    s.save("56-parte-desactivada");
    s.click(s.find("verificada"));
    s.settle();
    assert!(s.state.verified);
    s.ui.pointer_moved(Vec2::new(450.0, 130.0));
    s.ui.scroll_wheel(-420.0);
    s.settle();
    s.click(s.find("editar"));
    s.settle();
    s.save("57-menu-con-atajos");
    s.ui.key_pressed(Key::Escape);
    s.frame();
    s.settle();
    let review = s.ui.find_all("hecho")[1];
    s.click(review);
    s.settle();
    assert!(s.state.done[1], "the button in the cell took the click");
    s.state.preview_size = 2;
    s.ui.pointer_moved(Vec2::new(450.0, 130.0));
    s.ui.scroll_wheel(-20000.0);
    s.settle();
    s.save("58-estilo-heredado");
    s.ui.pointer_left();
    s.settle();

    // The syllabus, with the progress of everything visited.
    s.open_index();
    s.ui.pointer_moved(Vec2::new(150.0, 400.0));
    s.settle();
    s.save("32-temario-con-progreso");
    s.ui.pointer_left();
    s.time += 1.0;
    s.settle();

    // The incremental painter must leave exactly the pixels a full repaint would.
    s.save("16-forma");
    let mut fresh = morfo_demo::renderer();
    let mut full = Pixmap::new(s.pixmap.width(), s.pixmap.height()).unwrap();
    fresh.render(s.ui.draw_list(), &mut full, SCALE, s.ui.background());
    let differing = full.data().chunks(4).zip(s.pixmap.data().chunks(4)).filter(|(a, b)| a != b).count();
    let worst = full.data().iter().zip(s.pixmap.data()).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
    if differing > 0 {
        let w = full.width() as usize;
        for (i, (a, b)) in full.data().chunks(4).zip(s.pixmap.data().chunks(4)).enumerate() {
            if a != b {
                eprintln!("pixel ({}, {}): full {:?} vs incremental {:?}", i % w, i / w, a, b);
            }
        }
        full.save_png("out/zz-full.png").unwrap();
    }
    // Edges cut by a repainted band may round one level differently.
    assert!(worst <= 2, "incremental and full repaint differ by {worst}/255 in {differing} pixels");
    println!("incremental repaint vs full repaint: {differing} pixels differ, by at most {worst}/255");

    // What painting costs in a live window, frame by frame.
    println!("\npainting {}x{} px, every frame:", s.pixmap.width(), s.pixmap.height());
    s.go(1);
    s.measure("at rest", 60);
    let r = s.ui.rect_of(s.find("v-solid")).unwrap();
    for i in 0..4 {
        s.ui.pointer_moved(Vec2::new(r.x + 20.0 + i as f32 * 110.0, r.center().y));
        s.measure("moving the pointer over buttons", 15);
    }
    s.ui.pointer_left();
    s.settle();
    s.open_index();
    s.ui.pointer_moved(s.ui.rect_of(s.find("indice").with(3)).unwrap().center());
    s.frame();
    s.ui.pointer_pressed();
    s.frame();
    s.ui.pointer_released();
    s.measure("switching page (everything moves)", 40);
    s.settle();
    s.go(6);
    s.ui.pointer_moved(s.ui.rect_of(s.find("dlg-open")).unwrap().center());
    s.ui.pointer_pressed();
    s.frame();
    s.ui.pointer_released();
    s.measure("opening a dialog (dims the screen)", 40);
    s.ui.key_pressed(Key::Escape);
    s.settle();
    s.go(7);
    s.click(s.find("name"));
    for ch in ["m", "o", "r", "f", "o"] {
        s.ui.text_input(ch);
        s.measure("typing in a text field", 6);
    }
    s.ui.key_pressed(Key::Escape);
    s.go(5);

    // At rest: the UI must report that nothing needs drawing.
    let before = s.idle;
    s.frames(120);
    println!("\n{} frames simulated, {} elements alive", s.frames, s.ui.node_count());
    println!("average UI build: {:.1?} per frame", s.build / s.frames);
    println!("at rest: {}/120 frames reported idle (host can skip them all)", s.idle - before);
}
