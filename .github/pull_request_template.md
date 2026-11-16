## Qué cambia

<!-- Una o dos frases. Si cierra un issue: "Closes #123". -->

## Por qué

<!-- El problema que resuelve o la mejora que aporta. -->

## Cómo se ha probado

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets` sin avisos
- [ ] `cargo test --workspace`
- [ ] `cargo run --release -p morfo-demo --bin snapshot` (recorrido visual)
- [ ] Probado a mano en la documentación (`--bin docs`) si cambia algo visible

## Para componentes nuevos o cambios visibles

- [ ] Se anima con muelles (nada salta) y no rompe el reposo a coste cero
- [ ] Funciona con teclado y se describe para lectores de pantalla
- [ ] Tiene test y ejemplo en la documentación
- [ ] Capturas antes/después adjuntas
