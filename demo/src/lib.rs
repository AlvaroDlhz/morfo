//! La documentación interactiva de morfo, hecha con morfo: cada página
//! explica un grupo de componentes, lo muestra funcionando y enseña el código
//! que lo produce. La comparten la ventana (`docs`) y el simulador sin
//! ventana (`snapshot`).

mod pages;

use morfo::{Color, Date, Icon, Profile, Shortcut, Size, Theme, TitleBar, Transition, Ui, Variant, Vec2};

/// Tamaño lógico recomendado de la ventana.
pub const WINDOW: Vec2 = Vec2::new(900.0, 780.0);

pub const PAGES: [&str; 18] = [
    "Inicio",
    "Botones",
    "Controles",
    "Selección",
    "Cajas",
    "Formas",
    "Flotantes",
    "Entrada",
    "Adaptativo",
    "Mutación",
    "Datos",
    "Gráficos",
    "Navegación",
    "Formularios",
    "Estado y paneles",
    "Scroll",
    "Transiciones",
    "Composición",
];

/// El temario: las páginas agrupadas en módulos, en el orden en que se
/// aprenden. Cada lección es el índice de su página en [`PAGES`].
pub const COURSE: [(&str, &[usize]); 5] = [
    ("Fundamentos", &[0, 1, 2]),
    ("Construir interfaces", &[3, 4, 17, 5, 7, 13]),
    ("Capas y estado", &[6, 14]),
    ("Navegar y mostrar datos", &[12, 10, 11]),
    ("Lo propio de morfo", &[8, 9, 15, 16]),
];

/// Las páginas en el orden del curso.
pub fn course_order() -> Vec<usize> {
    COURSE.iter().flat_map(|(_, pages)| pages.iter().copied()).collect()
}

/// Posición en el curso (0 = primera lección) de la página `tab`.
pub fn lesson_of(tab: usize) -> usize {
    course_order().iter().position(|&p| p == tab).unwrap_or(0)
}

const CITIES: [&str; 8] = ["Madrid", "Barcelona", "Valencia", "Sevilla", "Málaga", "Bilbao", "Zaragoza", "A Coruña"];

const PRODUCTS: [&[&str]; 7] = [
    &["Teclado mecánico", "Periféricos", "89 €", "34"],
    &["Monitor 27\"", "Pantallas", "329 €", "12"],
    &["Ratón inalámbrico", "Periféricos", "39 €", "120"],
    &["Portátil 14\"", "Ordenadores", "1149 €", "7"],
    &["Auriculares", "Audio", "129 €", "56"],
    &["Webcam HD", "Periféricos", "59 €", "0"],
    &["Altavoces", "Audio", "79 €", "18"],
];

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

const VARIANT_NAMES: [&str; 4] = ["Solid", "Soft", "Outline", "Ghost"];

const ICON_NAMES: [&str; 18] = [
    "Plus",
    "Minus",
    "Close",
    "Check",
    "ArrowLeft",
    "ArrowRight",
    "ArrowUp",
    "ArrowDown",
    "ChevronDown",
    "Menu",
    "Search",
    "Home",
    "Download",
    "Play",
    "Pause",
    "Star",
    "Maximize",
    "Restore",
];

/// One kind of button shown in its four variants, labelled.
fn variant_row(ui: &mut Ui, title: &str, hint: &str, mut f: impl FnMut(&mut Ui, usize, Variant)) {
    ui.label(title);
    ui.muted(hint);
    ui.row(|ui| {
        for (i, v) in Variant::ALL.into_iter().enumerate() {
            f(ui, i, v);
        }
    });
}

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
    pub btn_size: usize,
    pub follow: [bool; 4],
    pub busy_until: [f64; 4],
    /// Whether the window is maximized (the host fills it in each frame).
    pub maximized: bool,
    /// What the title bar asked of the window this frame (the host does it).
    pub chrome: TitleBar,
    /// Running in a desktop window (with its own title bar), not a web page.
    pub native_window: bool,
    pub price: (f32, f32),
    pub volume: f32,
    pub stars: u8,
    pub accent: usize,
    pub point: Vec2,
    pub notes: String,
    pub plan_pick: usize,
    pub city_pick: Option<usize>,
    pub city_query: String,
    pub sort: Option<(usize, bool)>,
    pub product: Option<usize>,
    pub tasks: Vec<String>,
    /// The command palette is open.
    pub palette: bool,
    /// The search at the top right is unfolded.
    pub finder: bool,
    /// Notices dismissed on the «Cajas» page (they animate out).
    pub dismissed: [bool; 3],
    pub density: bool,
    /// The pinned notice of the «Flotantes» page is on.
    pub pinned: bool,
    pub chart_set: usize,
    pub gauge_level: f32,
    pub nav_tab: usize,
    pub tree_sel: Option<usize>,
    pub page_no: usize,
    pub drawer: bool,
    pub date: Date,
    pub color: Color,
    pub labels: Vec<String>,
    pub langs: Vec<usize>,
    pub shortcut: Option<Shortcut>,
    /// The skeleton on «Estado y paneles» shows until this time.
    pub load_until: f64,
    pub inbox: u32,
    pub step: usize,
    pub split: f32,
    pub board: Vec<Vec<String>>,
    /// How the cards of the «Scroll» page enter, and how many times they
    /// have been asked to enter again.
    pub reveal_style: usize,
    pub reveal_round: u32,
    /// The choreography and the step of the «Transiciones» wizard.
    pub tr_style: usize,
    pub tr_step: usize,
    /// The photo open in the gallery, if any.
    pub photo: Option<usize>,
    /// «Composición»: favourite projects, whether the account is verified,
    /// the open tab, table and section, the size and accent of the preview.
    pub favs: [bool; 3],
    pub verified: bool,
    pub inbox_tab: usize,
    pub team_sort: Option<(usize, bool)>,
    pub done: [bool; 4],
    pub topic: Option<usize>,
    pub preview_size: usize,
    pub preview_accent: usize,
    pub preview_on: bool,
    pub iban: String,
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
            btn_size: 1,
            follow: [false, true, false, true],
            busy_until: [0.0; 4],
            maximized: false,
            chrome: TitleBar::default(),
            native_window: true,
            price: (0.25, 0.7),
            volume: 0.62,
            stars: 3,
            accent: 0,
            point: Vec2::new(0.3, 0.6),
            notes: String::new(),
            plan_pick: 1,
            city_pick: None,
            city_query: String::new(),
            sort: None,
            product: None,
            tasks: ["Diseñar la portada", "Revisar los textos", "Preparar la demo", "Publicar la versión", "Escribir el anuncio"]
                .map(String::from)
                .to_vec(),
            palette: false,
            finder: false,
            dismissed: [false; 3],
            density: false,
            pinned: false,
            chart_set: 0,
            gauge_level: 0.62,
            nav_tab: 0,
            tree_sel: Some(2),
            page_no: 0,
            drawer: false,
            date: Date::new(2026, 11, 16),
            color: Color::hex(0x22c3a6),
            labels: ["diseño", "rust"].map(String::from).to_vec(),
            langs: vec![0, 3],
            shortcut: Some(Shortcut { ctrl: true, shift: false, key: "K".into() }),
            load_until: 0.0,
            inbox: 3,
            step: 1,
            split: 0.45,
            board: vec![
                ["Escribir la guía", "Revisar colores", "Probar en Windows"].map(String::from).to_vec(),
                ["Gráficos animados"].map(String::from).to_vec(),
                ["Publicar la 0.1"].map(String::from).to_vec(),
            ],
            reveal_style: 1,
            reveal_round: 0,
            tr_style: 3,
            tr_step: 0,
            photo: None,
            favs: [true, false, false],
            verified: false,
            inbox_tab: 0,
            team_sort: None,
            done: [true, false, false, true],
            topic: Some(0),
            preview_size: 1,
            preview_accent: 1,
            preview_on: true,
            iban: String::new(),
        }
    }
}

/// Colores de acento que se pueden elegir en la página «Controles».
pub const ACCENTS: [u32; 6] = [0x7c6cff, 0x22c3a6, 0xff7a59, 0xf5b83d, 0xe5484d, 0x3b82f6];

// Lo que una recarga en vivo (`morfo-live`) lleva a la nueva compilación:
// lo elegido y escrito en las páginas. Lo demás empieza de nuevo.
#[cfg(not(target_arch = "wasm32"))]
morfo_live::persist!(State {
    tab,
    light,
    level,
    range,
    details,
    shape,
    style,
    clicks,
    news,
    autosave,
    plan,
    tags,
    seats,
    faq,
    dialog,
    toasts,
    name,
    city,
    search_open,
    query,
    form,
    mut_check,
    mut_text,
    mut_toggle,
    btn_size,
    follow,
    price,
    volume,
    stars,
    accent,
    notes,
    plan_pick,
    city_pick,
    city_query,
    sort,
    product,
    tasks,
    palette,
    finder,
    dismissed,
    density,
    pinned,
    chart_set,
    gauge_level,
    nav_tab,
    tree_sel,
    page_no,
    labels,
    langs,
    inbox,
    step,
    split,
    board,
    reveal_style,
    tr_style,
    tr_step,
    photo,
    favs,
    verified,
    inbox_tab,
    team_sort,
    done,
    topic,
    preview_size,
    preview_accent,
    preview_on,
    iban,
});

#[cfg(not(target_arch = "wasm32"))]
impl State {
    /// El estado que dejó la compilación anterior, en lo que aún encaje en
    /// esta: el código puede haber cambiado por debajo (una página menos,
    /// otro número de opciones).
    pub fn restored(snap: &morfo_live::Snapshot) -> State {
        let mut s = State::default();
        s.restore(snap);
        s.tab = s.tab.min(PAGES.len() - 1);
        s.accent = s.accent.min(ACCENTS.len() - 1);
        s.style = s.style.min(STYLES.len() - 1);
        s.shape = s.shape.min(SHAPES.len() - 1);
        s.btn_size = s.btn_size.min(Size::ALL.len() - 1);
        s.chart_set = s.chart_set.min(1);
        s.nav_tab = s.nav_tab.min(2);
        s.step = s.step.min(3);
        s.reveal_style = s.reveal_style.min(morfo::Reveal::ALL.len() - 1);
        s.tr_style = s.tr_style.min(Transition::ALL.len() - 1);
        s.tr_step = s.tr_step.min(pages::PASOS_TR.len() - 1);
        s.photo = s.photo.filter(|&p| p < pages::FOTOS.len());
        s.inbox_tab = s.inbox_tab.min(2);
        s.preview_size = s.preview_size.min(2);
        s.preview_accent = s.preview_accent.min(ACCENTS.len() - 1);
        s.topic = s.topic.filter(|&t| t < 3);
        s.langs.retain(|&l| l < 8);
        if s.board.len() != 3 {
            s.board = State::default().board;
        }
        if let Some(d) = snap.get::<i64>("date") {
            s.date = Date::from_days(d);
        }
        if let Some(c) = snap.get::<u32>("color") {
            s.color = Color::hex(c);
        }
        s
    }

    /// Everything [`restored`](Self::restored) reads back.
    pub fn save_live(&self, snap: &mut morfo_live::Snapshot) {
        self.save(snap);
        snap.set("date", &self.date.to_days());
        snap.set("color", &self.color.to_hex());
    }
}

/// El tema según las preferencias elegidas: claro u oscuro, y su acento.
pub fn theme_of(s: &State) -> Theme {
    let base = if s.light { Theme::light() } else { Theme::dark() };
    base.with_accent(Color::hex(ACCENTS[s.accent]))
}

/// Las aplicaciones del panel de inicio adaptativo.
pub const TILES: [(&str, &str); 6] =
    [("correo", "Correo"), ("calendario", "Calendario"), ("tareas", "Tareas"), ("notas", "Notas"), ("fotos", "Fotos"), ("musica", "Música")];

/// Por qué la interfaz coloca algo donde lo coloca, en palabras.
pub fn reason_text(ui: &Ui, reason: Option<morfo::Reason>) -> String {
    use morfo::Reason;
    match reason {
        Some(Reason::AfterPrevious(prev)) => format!("Sueles usarlo justo después de «{}»", ui.label_of(prev).unwrap_or("otra acción")),
        Some(Reason::TimeOfDay(b)) => {
            format!("Lo usas sobre todo {}", ["de madrugada", "por la mañana", "por la tarde", "por la noche"][b as usize % 4])
        }
        Some(Reason::Frequent(n)) => format!("Lo has usado {n} veces"),
        Some(Reason::Rare) => "Apenas lo usas: se hace pequeño".into(),
        Some(Reason::Recent) | None => "Lo has usado hace poco".into(),
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

/// Cada página: su clave (para el pie), su área de scroll y quién la dibuja.
type Page = (&'static str, &'static str, fn(&mut Ui, &mut State));
const PAGE_FNS: [Page; 18] = [
    ("page-inicio", "scroll-inicio", pages::inicio),
    ("page-botones", "scroll-botones", pages::botones),
    ("page-controles", "scroll-controles", pages::controles),
    ("page-seleccion", "scroll-seleccion", pages::seleccion),
    ("page-contenedores", "scroll-contenedores", pages::cajas),
    ("page-formas", "scroll-formas", pages::formas),
    ("page-flotantes", "scroll-flotantes", pages::flotantes),
    ("page-entrada", "scroll-entrada", pages::entrada),
    ("page-adaptativo", "scroll-adaptativo", pages::adaptativo),
    ("page-mutacion", "scroll-mutacion", pages::mutacion),
    ("page-datos", "scroll-datos", pages::datos),
    ("page-graficos", "scroll-graficos", pages::graficos),
    ("page-navegacion", "scroll-navegacion", pages::navegacion),
    ("page-formularios", "scroll-formularios", pages::formularios),
    ("page-estado", "scroll-estado", pages::estado),
    ("page-scroll", "scroll-scroll", pages::scroll),
    ("page-transiciones", "scroll-transiciones", pages::transiciones),
    ("page-composicion", "scroll-composicion", pages::composicion),
];

/// La página abierta: su contenido hace scroll y, al cambiar de lección,
/// la nueva llega desde el lado hacia el que se avanza en el curso.
fn page(ui: &mut Ui, s: &mut State) {
    let height = (ui.viewport().y - ui.top_inset() - 100.0).max(120.0);
    let index = s.tab.min(PAGE_FNS.len() - 1);
    let (key, scroll, f) = PAGE_FNS[index];
    ui.transition("paginas", lesson_of(index), Transition::Shift, |ui| {
        ui.scroll(scroll, height, |ui| {
            f(ui, s);
            lesson_footer(ui, s, key, index);
        })
    });
}

/// Al final de cada lección: anterior, posición en el curso y siguiente.
fn lesson_footer(ui: &mut Ui, s: &mut State, key: &str, page: usize) {
    let order = course_order();
    let at = lesson_of(page);
    ui.space(8.0);
    ui.separator();
    ui.scope(key, |ui| {
        ui.row(|ui| {
            if at > 0 && ui.button_icon("anterior", Icon::ArrowLeft, PAGES[order[at - 1]], Variant::Ghost, Size::Md).clicked {
                s.tab = order[at - 1];
            }
            ui.fill();
            ui.muted(&format!("Lección {} de {}", at + 1, order.len()));
            if let Some(&next) = order.get(at + 1) {
                if ui.button_icon("siguiente", Icon::ArrowRight, &format!("Siguiente: {}", PAGES[next]), Variant::Soft, Size::Md).clicked {
                    s.tab = next;
                }
            }
        });
    });
}

pub fn build(ui: &mut Ui, s: &mut State) {
    let order = course_order();
    let lesson = lesson_of(s.tab);

    // Lo que el buscador encuentra: todas las lecciones y algunas acciones.
    let mut found: Vec<(String, String)> = Vec::new();
    for (m, (module, pages)) in COURSE.iter().enumerate() {
        for &p in pages.iter() {
            found.push((format!("ir-{p}"), format!("{} · Módulo {}: {module}", PAGES[p], m + 1)));
        }
    }
    found.push(("tema".into(), "Cambiar entre tema claro y oscuro".into()));
    found.push(("aviso".into(), "Lanzar un aviso".into()));
    found.push(("paleta".into(), "Abrir la paleta de comandos".into()));
    found.push(("olvidar".into(), "Olvidar hábitos y progreso".into()));
    let refs: Vec<(&str, &str)> = found.iter().map(|(k, l)| (k.as_str(), l.as_str())).collect();
    let hint = if s.native_window { "Buscar (Ctrl+K)" } else { "Buscar" };

    // La barra de la aplicación sustituye a la del gestor de ventanas; a la
    // derecha, el buscador plegado en una lupa.
    let mut picked = None;
    let place = format!("Lección {} de {} · {}", lesson + 1, order.len(), PAGES[s.tab]);
    let header = |ui: &mut Ui, finder: &mut bool, picked: &mut Option<usize>| {
        ui.muted(&place);
        ui.fill();
        *picked = ui.search("buscar", hint, finder, &refs, 320.0);
        ui.scope("buscar", |ui| ui.tooltip("boton", hint));
    };
    if s.native_window {
        s.chrome = ui.title_bar("barra", "morfo · documentación", s.maximized, |ui| header(ui, &mut s.finder, &mut picked));
    } else {
        // In a browser tab the page has no window to move: a plain header.
        ui.row(|ui| {
            ui.title("morfo · documentación");
            header(ui, &mut s.finder, &mut picked);
        });
    }
    if let Some(i) = picked {
        match found[i].0.as_str() {
            "tema" => {
                s.light = !s.light;
                ui.set_theme(theme_of(s));
            }
            "aviso" => ui.toast("Aviso lanzado desde el buscador"),
            "paleta" => s.palette = true,
            "olvidar" => forget(ui, s),
            k => s.tab = k.trim_start_matches("ir-").parse().unwrap_or(0),
        }
    }

    // El temario vive en el borde izquierdo: se despliega al acercar el cursor.
    let modules: Vec<(&str, Vec<&str>)> = COURSE.iter().map(|(name, pages)| (*name, pages.iter().map(|&p| PAGES[p]).collect())).collect();
    let modules: Vec<(&str, &[&str])> = modules.iter().map(|(name, lessons)| (*name, lessons.as_slice())).collect();
    let mut at = lesson;
    if ui.course_nav("indice", "Curso de morfo", &modules, &mut at) {
        s.tab = order[at];
    }
    ui.muted("Documentación interactiva, hecha con la propia librería. Lleva el cursor al borde izquierdo para ver el temario.");
    ui.space(4.0);

    page(ui, s);

    // La paleta de comandos (desde el buscador o la página «Adaptativo»):
    // con nada escrito, lo que esta persona más usa aparece primero.
    let mut commands: Vec<(String, String)> = PAGES.iter().enumerate().map(|(i, p)| (format!("ir-{i}"), format!("Ir a {p}"))).collect();
    commands.push(("tema".into(), "Cambiar entre tema claro y oscuro".into()));
    commands.push(("aviso".into(), "Lanzar un aviso".into()));
    commands.push(("olvidar".into(), "Olvidar hábitos".into()));
    let refs: Vec<(&str, &str)> = commands.iter().map(|(k, l)| (k.as_str(), l.as_str())).collect();
    if let Some(i) = ui.command_palette("paleta", &mut s.palette, &refs) {
        match commands[i].0.as_str() {
            "tema" => {
                s.light = !s.light;
                ui.set_theme(theme_of(s));
            }
            "aviso" => ui.toast("Aviso lanzado desde la paleta"),
            "olvidar" => forget(ui, s),
            k => s.tab = k.trim_start_matches("ir-").parse().unwrap_or(0),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn first_font(env: &str, candidates: &[&str]) -> Option<Vec<u8>> {
    std::env::var(env).ok().into_iter().chain(candidates.iter().map(|s| s.to_string())).find_map(|p| std::fs::read(p).ok())
}

/// Fuente de interfaz del sistema (o la de `MORFO_FONT=/ruta/fuente.ttf`).
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
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

/// Renderizador por GPU con las fuentes del sistema ya cargadas.
#[cfg(not(target_arch = "wasm32"))]
pub fn gpu_renderer(device: &morfo_wgpu::wgpu::Device, format: morfo_wgpu::wgpu::TextureFormat) -> morfo_wgpu::Renderer {
    let mut r = morfo_wgpu::Renderer::new(device, format, load_font()).expect("fuente no válida");
    if let Some(mono) = load_mono_font() {
        let _ = r.set_mono_font(mono);
    }
    r
}

/// Renderizador con las fuentes del sistema ya cargadas.
#[cfg(not(target_arch = "wasm32"))]
pub fn renderer() -> morfo_skia::Renderer {
    let mut r = morfo_skia::Renderer::new(load_font()).expect("fuente no válida");
    if let Some(mono) = load_mono_font() {
        // Una fuente de código ilegible no impide abrir la documentación.
        let _ = r.set_mono_font(mono);
    }
    r
}
