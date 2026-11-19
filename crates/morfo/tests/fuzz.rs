//! Fuzzing without a fuzzer: thousands of deterministic mutations of valid
//! saved texts (a usage profile, an interface state) fed back to the code
//! that reads them. Whatever arrives from disk, reading it must never
//! panic, and what it accepts must survive being saved again.

use morfo::{Profile, Theme, Ui, Vec2};

/// A small, fast, deterministic random generator (xorshift64*).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Bytes worth trying: separators, signs, digits, huge and odd numbers.
const PIECES: [&str; 16] =
    ["\n", " ", "\t", "-", "1e309", "NaN", "inf", "0", "18446744073709551616", "-1", "é", "\u{0}", "morfo2", "9.9e-46", "", "  "];

/// `text` with a few random edits: bytes removed, duplicated or replaced,
/// lines swapped, pieces inserted. Always valid UTF-8.
fn mutate(rng: &mut Rng, text: &str) -> String {
    let mut s: Vec<char> = text.chars().collect();
    for _ in 0..1 + rng.below(4) {
        let at = rng.below(s.len() + 1);
        match rng.below(5) {
            0 if !s.is_empty() => {
                let end = (at + rng.below(8)).min(s.len());
                s.drain(at.min(s.len())..end);
            }
            1 => {
                let piece = PIECES[rng.below(PIECES.len())];
                for (i, c) in piece.chars().enumerate() {
                    s.insert((at + i).min(s.len()), c);
                }
            }
            2 if !s.is_empty() => {
                let i = rng.below(s.len());
                s[i] = char::from_u32(0x20 + rng.below(0x60) as u32).unwrap_or(' ');
            }
            3 if !s.is_empty() => {
                let i = rng.below(s.len());
                let c = s[i];
                s.insert(i, c);
            }
            _ => s.truncate(at),
        }
    }
    s.into_iter().collect()
}

/// A profile with some history, encoded.
fn sample_profile() -> String {
    let mut ui = Ui::new(Theme::dark());
    ui.profile_mut().time_of_day = Some(3600.0);
    let mut t = 1_000.0;
    for i in 0..40 {
        t += 7.0;
        ui.begin(t, Vec2::new(800.0, 600.0));
        ui.record_named(["guardar", "abrir", "exportar"][i % 3], "Una acción");
        ui.end();
    }
    ui.profile().encode()
}

#[test]
fn reading_a_damaged_profile_never_panics() {
    let valid = sample_profile();
    assert!(Profile::decode(&valid).is_some());
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut accepted = 0;
    for _ in 0..20_000 {
        let text = mutate(&mut rng, &valid);
        if let Some(p) = Profile::decode(&text) {
            accepted += 1;
            // What is accepted can be saved and read again.
            assert!(Profile::decode(&p.encode()).is_some(), "re-encoding failed for {text:?}");
        }
    }
    assert!(accepted > 0, "some mutations should still be readable");
}

#[test]
fn restoring_a_damaged_state_never_panics() {
    let build = |ui: &mut Ui| {
        ui.scroll("lista", 200.0, |ui| {
            for i in 0..40 {
                ui.label(&format!("Fila {i}"));
            }
        });
    };
    let mut ui = Ui::new(Theme::dark());
    let mut t = 0.0;
    for _ in 0..30 {
        t += 1.0 / 60.0;
        ui.begin(t, Vec2::new(800.0, 600.0));
        build(&mut ui);
        ui.end();
    }
    ui.pointer_moved(Vec2::new(100.0, 100.0));
    ui.scroll_wheel(-300.0);
    for _ in 0..60 {
        t += 1.0 / 60.0;
        ui.begin(t, Vec2::new(800.0, 600.0));
        build(&mut ui);
        ui.end();
    }
    let valid = ui.save_state();
    let mut rng = Rng(0xdead_beef_cafe_f00d);
    for _ in 0..5_000 {
        let text = mutate(&mut rng, &valid);
        let mut next = Ui::new(Theme::dark());
        next.restore_state(&text);
        // And the interface still builds with whatever was accepted.
        next.begin(1.0, Vec2::new(800.0, 600.0));
        build(&mut next);
        next.end();
    }
}

#[test]
fn arbitrary_text_is_never_read_as_valid_by_accident() {
    let mut rng = Rng(42);
    for _ in 0..5_000 {
        let len = rng.below(200);
        let text: String = (0..len).map(|_| char::from_u32(rng.below(0x250) as u32).unwrap_or('?')).collect();
        // Must not panic; and without the header nothing is accepted.
        let p = Profile::decode(&text);
        if !text.starts_with("morfo") {
            assert!(p.is_none(), "headerless text accepted: {text:?}");
        }
        Ui::new(Theme::dark()).restore_state(&text);
    }
}
