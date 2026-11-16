# morfo

Componentes de interfaz en Rust donde **todo se transforma con fluidez** y la
interfaz **se adapta a cada persona según cómo la usa**. Dos usuarios del mismo
programa pueden acabar viendo interfaces distintas.

| Documentación interactiva | Índice lateral desplegado |
| --- | --- |
| ![inicio](docs/inicio.png) | ![indice](docs/indice.png) |
| **Interfaz ya adaptada al uso** | **Tipos de botón y sus variantes** |
| ![adaptado](docs/adaptado.png) | ![botones](docs/botones.png) |
| **Controles, con otro color de acento** | **Paleta de comandos (Ctrl+K)** |
| ![controles](docs/controles.png) | ![paleta](docs/paleta.png) |
| **Tabla ordenada por precio** | |
| ![tabla](docs/tabla.png) | |
| **Diálogo que crece desde su botón** | **Controles de selección** |
| ![dialogo](docs/dialogo.png) | ![seleccion](docs/seleccion.png) |
| **Un botón mutando en tarjeta** | **Mutado según el uso** |
| ![mutacion](docs/mutacion.png) | ![mutacion-uso](docs/mutacion-uso.png) |

## Ideas clave

- **Identidad = morph.** Cada elemento tiene un `Id` estable. El layout solo
  calcula *destinos*; unos muelles llevan hasta ellos la posición, el tamaño,
  el radio, los colores y la tipografía. Reordenar, redimensionar, cambiar de
  variante o de tema es siempre una transición continua, sin escribir código
  de animación.
- **Mutación.** Un componente puede convertirse en otro distinto (un botón en
  un campo de texto, una casilla en una tarjeta): `ui.morph(clave, forma, …)`
  hace viajar la forma de fondo de uno a otro (posición, tamaño, radio y
  color) mientras el contenido se funde, y los vecinos se apartan al mismo
  ritmo. La «forma» puede ser cualquier valor con `Hash`, incluido el peso que
  el uso le ha dado a un elemento.
- **Muelles analíticos.** La solución cerrada del muelle amortiguado es lineal
  en posición y velocidad, así que sus 4 coeficientes se calculan una vez por
  frame y cada valor animado avanza con 4 multiplicaciones. Exacto a cualquier
  frecuencia de refresco, interrumpible en cualquier momento conservando la
  velocidad.
- **Motor adaptativo.** `Profile` guarda por elemento una puntuación de uso
  que decae con el tiempo. De ahí salen el orden (`arrange`) y el peso visual
  (`prominence`), ambos con histéresis para que nada baile. Nada se mueve
  mientras el puntero está encima. El perfil se serializa a texto (unos 200
  bytes en la demo) para guardarlo por usuario.
- **Coste cero en reposo.** `ui.end()` indica si algo sigue moviéndose; si no,
  el programa no dibuja ni calcula nada hasta el siguiente evento.
- **Independiente del renderizador.** El núcleo (`morfo`) no tiene ninguna
  dependencia y produce una lista plana de primitivas. `morfo-skia` las pinta
  por CPU; se puede escribir otro backend (GPU, canvas web) sin tocar el núcleo.

## Uso

```rust
use morfo::{Theme, Ui, Vec2};

let mut ui = Ui::new(Theme::dark());
let mut oscuro = true;

// cada frame:
ui.begin(tiempo_en_segundos, Vec2::new(ancho, alto));
ui.heading("Hola");
if ui.button("guardar", "Guardar").clicked { /* ... */ }
ui.toggle("oscuro", &mut oscuro);
// Barra que se reordena y redimensiona según el uso de esta persona:
if let Some(i) = ui.adaptive_actions("acciones", &[("nuevo", "Nuevo"), ("abrir", "Abrir")]) { /* ... */ }
let frame = ui.end();

renderer.render(ui.draw_list(), &mut pixmap, escala, ui.background());
if frame.animating { /* pedir otro frame */ }
```

La entrada se pasa con `pointer_moved`, `pointer_pressed`, `pointer_released`,
`pointer_left`, `scroll_wheel`, `key_pressed` y `text_input`. Si
`frame.animating` es falso, `frame.wake_in` dice en cuántos segundos hará
falta un frame sin que haya entrada (un aviso que caduca, el cursor de texto).
Para guardar y restaurar la forma de cada usuario: `ui.profile().encode()` y
`ui.set_profile(Profile::decode(texto)?)`.

### Componentes

| Grupo | Componentes |
| --- | --- |
| Texto | `label`, `muted`, `title`, `heading`, `text` (tamaño libre), `code`, `paragraph` y `paragraph_muted` (se ajustan al ancho) |
| Botones | `button`, `button_with`, `icon_button` (solo icono), `button_icon` (icono y texto), `toggle_button` (se queda pulsado), `loading_button` (se encoge en un indicador mientras carga), `hold_button` (mantener para confirmar). Todos con las variantes `Solid`, `Soft`, `Outline`, `Ghost` y los tamaños `Sm`, `Md`, `Lg` |
| Iconos | `Icon`: 18 iconos vectoriales (flechas, cerrar, buscar, inicio, descargar, reproducir, maximizar…) |
| Controles | `toggle`, `slider`, `progress`, `segmented`, `range_slider` (rango con dos manijas), `knob` (perilla), `xy_pad` (pad de dos ejes), `rating` (estrellas), `swatches` (muestras de color) |
| Selección | `checkbox`, `radio`, `chip`, `stepper`, `accordion` |
| Cajas | `row`, `column`, `card`, `separator`, `reveal`, `scroll`, `scope`, `surface`, `presence` |
| Flotantes | `dialog`, `menu`, `tooltip`, `toast`, `close` |
| Navegación | `side_nav` (índice oculto en el borde izquierdo que se despliega al acercar el cursor) |
| Ventana | `title_bar` (barra propia de la aplicación en lugar de la del gestor de ventanas: título, contenido propio, minimizar, maximizar/restaurar, cerrar, arrastrar, doble clic y bordes para redimensionar) |
| Entrada | `text_field` (foco, cursor, Tab, Enter, Escape), `text_area` (multilínea con ajuste de línea y scroll), `select` (desplegable), `combobox` (campo con sugerencias, sin distinguir tildes) |
| Datos | `table` (ordenable por columnas; las filas viajan al ordenar), `reorderable` (lista que se reordena arrastrando) |
| Adaptativos | `adaptive_actions`, `adaptive_menu`, `suggestions`, `command_palette` (paleta de comandos que pone primero lo que más usa cada persona) |
| Mutación | `morph` (cualquier componente o grupo se transforma en otro), `request_focus` |

Los flotantes se dibujan en capas superiores, bloquean lo que tapan y animan
también su salida. Para adaptar componentes propios: `ui.affinity(key)`,
`ui.prominence(key)`, `ui.record(key)` y `Profile::arrange`.

## Documentación con ejemplos

```bash
cargo run --release -p morfo-demo --bin docs
```

La documentación es una app hecha con morfo. Tiene once páginas (Inicio,
Botones, Controles, Selección, Cajas, Formas, Flotantes, Entrada, Adaptativo,
Mutación, Datos), con su propia barra de ventana en lugar de la del sistema, y a
las que se llega con un índice lateral: un riel en el borde
izquierdo que se despliega al acercar el cursor. Cada página explica un
grupo de componentes, lo muestra funcionando y enseña el código que lo
produce. Su fuente, `demo/src/lib.rs`, sirve también de ejemplo completo.
El perfil de uso se guarda al cerrar en `~/.config/morfo-demo/profile.txt`.

```bash
cargo run --release -p morfo-demo --bin snapshot
```

Recorre la documentación simulando a un usuario, sin ventana; guarda frames
PNG en `out/` y mide el trabajo real.

```bash
cargo test -p morfo
```

## Medidas

Con la documentación (`--release`, ventana de 900×780 a 2× = 1800×1560 px,
portátil de 4 núcleos con otros programas abiertos). Las produce
`cargo run --release -p morfo-demo --bin snapshot`:

| Situación | Pintado por frame | Filas repintadas |
| --- | --- | --- |
| En reposo | nada | 0 % |
| Mover el ratón sobre botones | ~1 ms | 6 % |
| Escribir en un campo de texto | ~1 ms | 4–7 % |
| Cambiar de página (todo se mueve) | ~11 ms | 86 % |
| Abrir un diálogo (oscurece la pantalla) | ~12 ms | 72 % |

Construir la interfaz y avanzar todas las animaciones cuesta unos 40 µs por
frame.

El renderizador repinta solo las filas que cambian respecto al frame
anterior, las reparte en franjas entre hilos y dibuja rectángulos y sombras
con un rasterizador propio que solo calcula el contorno. El simulador
comprueba que el resultado incremental es idéntico, píxel a píxel, a un
repintado completo.

## Estructura

| Ruta | Qué es |
| --- | --- |
| `crates/morfo` | Núcleo sin dependencias: muelles, perfil adaptativo, layout, componentes, lista de dibujo |
| `crates/morfo-skia` | Renderizador por CPU (tiny-skia + ab_glyph) |
| `demo` | Documentación interactiva en ventana (winit + softbuffer) y simulador sin ventana |

## Limitaciones actuales

- Los campos de texto no tienen selección, portapapeles ni IME. Los botones y
  demás controles aún no se manejan con teclado, y no hay accesibilidad.
- Una fuente normal y una monoespaciada; sin negritas ni cursivas.
- La tabla dibuja todas sus filas; para miles de filas hace falta paginarla.
- El scroll es vertical y con rueda (sin arrastrar la barra); dibuja solo lo
  visible, pero recorre el layout de todo el contenido.
- Layout de filas y columnas con tamaños intrínsecos; sin rejilla.
- Fuera de `reveal`, los flotantes y `presence`, un elemento que deja de
  declararse desaparece sin animación de salida.
- Durante una mutación, los controles de la forma nueva ya responden en su
  posición final, aunque aún se estén dibujando a medio camino. Lo que esa
  forma dibuje en capas flotantes (tooltips, diálogos) no participa en el morph.
- `title_bar` no mueve ni cierra la ventana por sí misma: devuelve lo que
  pidió el usuario y el programa lo hace (en la demo, con winit). Arrastrar
  y redimensionar dependen de que el gestor de ventanas lo permita; las
  sombras y esquinas redondeadas de la ventana no se dibujan.
- Solo hay backend por CPU; no hay backend GPU ni web todavía. Las
  transiciones a pantalla completa a 2× rondan los 11 ms: caben en 60 fps,
  pero con poco margen en máquinas lentas o pantallas mayores.
- El backend recorta las formas por bandas de filas, no por rectángulos
  arbitrarios (el texto sí se recorta exacto).

## Licencia

MIT o Apache-2.0, a tu elección.
