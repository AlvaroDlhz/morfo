# Cómo contribuir a morfo

> **English:** contributions are welcome in English or Spanish. Open an issue
> before large changes, keep pull requests focused, and make sure
> `cargo fmt`, `cargo clippy` and `cargo test` pass. Details below.

Gracias por querer mejorar morfo. Esta guía explica cómo proponer cambios
para que se puedan revisar y aceptar rápido.

## Antes de empezar

- **Errores:** abre un *issue* con la plantilla «Error». Incluye cómo
  reproducirlo, qué esperabas y qué pasó.
- **Ideas y componentes nuevos:** abre primero un *issue* con la plantilla
  «Propuesta» o una conversación en *Discussions*. Así acordamos el enfoque
  antes de que inviertas tiempo en código.
- **Dudas de uso:** usa *Discussions*, no los *issues*.
- **Fallos de seguridad:** no los publiques; sigue [SECURITY.md](SECURITY.md).

Los *issues* con la etiqueta `good first issue` son buenos para empezar.

## Preparar el entorno

```bash
git clone https://github.com/<tu-usuario>/morfo
cd morfo
cargo test --workspace
cargo run --release -p morfo-demo --bin docs
```

Para la versión web: `rustup target add wasm32-unknown-unknown`,
`cargo install wasm-bindgen-cli --version <la de Cargo.lock>` y `./web/build.sh`.

## Flujo de trabajo

1. Haz un *fork* y crea una rama desde `main` con un nombre descriptivo
   (`feat/selector-de-fecha`, `fix/foco-en-dialogo`).
2. Haz cambios pequeños y enfocados: un PR, un propósito.
3. Antes de abrir el PR, comprueba en local lo mismo que la CI:

   ```bash
   cargo fmt --all --check
   cargo clippy --workspace --all-targets
   cargo clippy -p morfo-web -p morfo-web-demo --target wasm32-unknown-unknown
   cargo test --workspace
   cargo run --release -p morfo-demo --bin snapshot
   ```

4. Abre el PR contra `main` y rellena la plantilla.

`main` está protegida: los cambios entran solo por PR, con la CI en verde, la
revisión de un responsable del código y todas las conversaciones resueltas.
Los PR se fusionan con *squash*, así que el título del PR será el mensaje del
commit: que sea claro.

## Criterios para aceptar un cambio

- **Fluidez:** todo cambio visual pasa por los muelles; nada salta.
- **Coste cero en reposo:** si no hay animación ni entrada, `ui.end()` debe
  informar de que no hace falta dibujar.
- **El núcleo sin dependencias:** `crates/morfo` no añade dependencias.
- **Teclado y accesibilidad:** los controles nuevos son enfocables con Tab,
  responden a Enter/Espacio (y a las flechas si tienen valor) y se describen
  con `describe`.
- **Tests:** cada componente nuevo lleva al menos un test en
  `crates/morfo/tests/ui.rs`, y si se ve, un ejemplo en la documentación
  (`demo/src/pages.rs`).
- **Los dos renderers de escritorio coinciden:** si tocas el dibujo, la
  comparación CPU/GPU (`cargo test -p morfo-wgpu`) debe seguir pasando.

## Estilo

- `cargo fmt` con la configuración del proyecto (`rustfmt.toml`).
- Comentarios de código en inglés; la documentación interactiva y el README
  en español.
- Nada de `unsafe` en el núcleo.

## Licencia

Al contribuir aceptas que tu aportación se publique bajo la doble licencia
del proyecto, MIT o Apache-2.0, a elección de quien la use.
