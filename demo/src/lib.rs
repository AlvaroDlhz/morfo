//! La documentación interactiva de morfo, hecha con morfo: cada página
//! explica un grupo de componentes, lo muestra funcionando y enseña el código
//! que lo produce. La comparten la ventana (`docs`) y el simulador sin
//! ventana (`snapshot`).

mod pages;

use morfo::{Color, Icon, Profile, Size, Theme, TitleBar, Ui, Variant, Vec2};

/// Tamaño lógico recomendado de la ventana.
pub const WINDOW: Vec2 = Vec2::new(900.0, 780.0);

pub const PAGES: [&str; 11] =
    ["Inicio", "Botones", "Controles", "Selección", "Cajas", "Formas", "Flotantes", "Entrada", "Adaptativo", "Mutación", "Datos"];

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
        }
    }
}

/// Colores de acento que se pueden elegir en la página «Controles».
pub const ACCENTS: [u32; 6] = [0x7c6cff, 0x22c3a6, 0xff7a59, 0xf5b83d, 0xe5484d, 0x3b82f6];

/// El tema según las preferencias elegidas: claro u oscuro, y su acento.
fn theme_of(s: &State) -> Theme {
    let base = if s.light { Theme::light() } else { Theme::dark() };
    base.with_accent(Color::hex(ACCENTS[s.accent]))
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
    let height = (ui.viewport().y - ui.top_inset() - 100.0).max(120.0);
    ui.reveal(key, open, |ui| ui.scroll(scroll, height, f));
}

pub fn build(ui: &mut Ui, s: &mut State) {
    // La barra de la aplicación sustituye a la del gestor de ventanas: mueve,
    // minimiza, maximiza y cierra la ventana (eso lo hace el programa).
    let page_name = PAGES[s.tab];
    let mut open_palette = false;
    // Ctrl+K is wired up by the desktop host; browsers keep it for themselves.
    let hint = if s.native_window { "Paleta de comandos (Ctrl+K)" } else { "Paleta de comandos" };
    let header = |ui: &mut Ui, open_palette: &mut bool| {
        ui.muted(page_name);
        if ui.icon_button("comandos", Icon::Search, Variant::Ghost, Size::Sm).clicked {
            *open_palette = true;
        }
        ui.tooltip("comandos", hint);
    };
    if s.native_window {
        s.chrome = ui.title_bar("barra", "morfo · documentación", s.maximized, |ui| header(ui, &mut open_palette));
    } else {
        // In a browser tab the page has no window to move: a plain header.
        ui.row(|ui| {
            ui.title("morfo · documentación");
            header(ui, &mut open_palette);
        });
    }
    if open_palette {
        s.palette = true;
    }
    // El índice vive en el borde izquierdo: se despliega al acercar el cursor.
    ui.side_nav("indice", "morfo", &PAGES, &mut s.tab);
    ui.muted("Documentación interactiva, hecha con la propia librería. Lleva el cursor al borde izquierdo para abrir el índice.");
    ui.space(4.0);

    page(ui, "page-inicio", "scroll-inicio", s.tab == 0, |ui| pages::inicio(ui, s));

    page(ui, "page-botones", "scroll-botones", s.tab == 1, |ui| pages::botones(ui, s));

    page(ui, "page-controles", "scroll-controles", s.tab == 2, |ui| pages::controles(ui, s));

    page(ui, "page-seleccion", "scroll-seleccion", s.tab == 3, |ui| pages::seleccion(ui, s));

    page(ui, "page-contenedores", "scroll-contenedores", s.tab == 4, |ui| pages::cajas(ui, s));

    page(ui, "page-formas", "scroll-formas", s.tab == 5, |ui| pages::formas(ui, s));

    page(ui, "page-flotantes", "scroll-flotantes", s.tab == 6, |ui| pages::flotantes(ui, s));

    page(ui, "page-entrada", "scroll-entrada", s.tab == 7, |ui| pages::entrada(ui, s));

    page(ui, "page-adaptativo", "scroll-adaptativo", s.tab == 8, |ui| pages::adaptativo(ui, s));

    page(ui, "page-mutacion", "scroll-mutacion", s.tab == 9, |ui| pages::mutacion(ui, s));

    page(ui, "page-datos", "scroll-datos", s.tab == 10, |ui| pages::datos(ui, s));

    // La paleta de comandos (Ctrl+K o el botón de la barra): con nada
    // escrito, lo que esta persona más usa aparece primero.
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
