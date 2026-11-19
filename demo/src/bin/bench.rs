//! Measures what building a frame costs: time, memory allocations and draw
//! commands, page by page of the documentation and for scenes of the
//! library alone (cards, a long scroll, a virtual list of 100 000 rows).
//!
//! `cargo run --release -p morfo-demo --bin bench` prints the table;
//! `-- --check` also fails if a steady frame of a library scene allocates,
//! or if a virtual list costs more with more rows. Times are only reported:
//! on a shared CI machine they are too noisy to fail on.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Instant;

use morfo::{Theme, Ui};
use morfo_demo::{build, State, PAGES, WINDOW};

/// The system allocator, counting.
struct Counting;
static ALLOCS: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// What steady frames of a scene cost, on average.
struct Cost {
    micros: f64,
    allocs: f64,
    cmds: usize,
    nodes: usize,
}

/// Runs `f` until the interface rests, then measures `n` more frames.
fn measure(ui: &mut Ui, t: &mut f64, n: u32, mut f: impl FnMut(&mut Ui)) -> Cost {
    for _ in 0..600 {
        *t += 1.0 / 60.0;
        ui.begin(*t, WINDOW);
        f(ui);
        if !ui.end().animating {
            break;
        }
    }
    let a0 = ALLOCS.load(Relaxed);
    let t0 = Instant::now();
    for _ in 0..n {
        *t += 1.0 / 60.0;
        ui.begin(*t, WINDOW);
        f(ui);
        ui.end();
    }
    Cost {
        micros: t0.elapsed().as_secs_f64() * 1e6 / n as f64,
        allocs: (ALLOCS.load(Relaxed) - a0) as f64 / n as f64,
        cmds: ui.draw_list().len(),
        nodes: ui.node_count(),
    }
}

fn row(name: &str, c: &Cost) {
    println!("  {name:<34} {:>8.0} µs {:>8.1} allocs {:>6} cmds {:>7} nodes", c.micros, c.allocs, c.cmds, c.nodes);
}

const KEYS: [&str; 10] = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];

fn main() {
    let check = std::env::args().any(|a| a == "--check");
    let renderer = morfo_demo::renderer();
    let fresh = || {
        let mut ui = Ui::new(Theme::dark());
        ui.set_text_measure(Box::new(renderer.measure()));
        ui
    };
    let mut t = 0.0;
    let mut failures = Vec::new();

    println!("documentation pages (the demo's own code allocates its strings):");
    let mut ui = fresh();
    let mut s = State::default();
    let mut total = 0.0;
    for (i, name) in PAGES.iter().enumerate() {
        s.tab = i;
        let c = measure(&mut ui, &mut t, 200, |ui| build(ui, &mut s));
        total += c.micros;
        row(name, &c);
    }
    println!("  {:<34} {:>8.0} µs", "average", total / PAGES.len() as f64);

    println!("library scenes (a steady frame must not allocate):");
    let keys: Vec<String> = (0..2000).map(|i| format!("k{i}")).collect();
    for cards in [20usize, 200, 2000] {
        let mut ui = fresh();
        let c = measure(&mut ui, &mut t, 200, |ui| {
            ui.scroll("s", 600.0, |ui| {
                for key in &keys[..cards] {
                    ui.card(key, |ui| {
                        ui.label("Una tarjeta");
                        ui.row(|ui| {
                            for k in KEYS {
                                ui.button(k, "Botón");
                            }
                        });
                    });
                }
            });
        });
        row(&format!("{cards} cards x 10 buttons"), &c);
        if c.allocs > 0.0 {
            failures.push(format!("{cards} cards: {} allocations per frame", c.allocs));
        }
    }
    let mut list = Vec::new();
    for rows in [100usize, 100_000] {
        let mut ui = fresh();
        let texts: Vec<String> = (0..64).map(|i| format!("Fila {i}")).collect();
        let c = measure(&mut ui, &mut t, 200, |ui| {
            ui.virtual_list("v", rows, 32.0, 500.0, |ui, i| ui.label(&texts[i % texts.len()]));
        });
        row(&format!("virtual list, {rows} rows"), &c);
        if c.allocs > 0.0 {
            failures.push(format!("virtual list of {rows}: {} allocations per frame", c.allocs));
        }
        list.push(c);
    }
    // The same rows are built whatever the length: the same commands, the
    // same elements.
    if list[0].cmds != list[1].cmds || list[1].nodes > list[0].nodes + 2 {
        failures.push(format!("a virtual list grows with its length: {} vs {} nodes", list[0].nodes, list[1].nodes));
    }
    let mut ui = fresh();
    let mut on = [true; KEYS.len()];
    let c = measure(&mut ui, &mut t, 200, |ui| {
        ui.heading("Quieto");
        for (k, on) in KEYS.iter().zip(on.iter_mut()) {
            ui.paragraph("Un párrafo que no cambia y que, una vez partido en líneas, no se vuelve a medir.");
            ui.toggle(k, on);
        }
    });
    row("paragraphs and toggles", &c);
    if c.allocs > 0.0 {
        failures.push(format!("paragraphs: {} allocations per frame", c.allocs));
    }

    if check {
        if failures.is_empty() {
            println!("check: ok");
        } else {
            for f in &failures {
                eprintln!("check failed: {f}");
            }
            std::process::exit(1);
        }
    }
}
