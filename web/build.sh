#!/bin/sh
# Builds the web version of the documentation into web/pkg.
# Needs: rustup target add wasm32-unknown-unknown
#        cargo install wasm-bindgen-cli --version <the wasm-bindgen in Cargo.lock>
set -e
cd "$(dirname "$0")/.."
cargo build -p morfo-web-demo --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/morfo_web_demo.wasm
echo "Listo: sirve la carpeta web/ (por ejemplo: python3 -m http.server -d web 8000)"
