//! Right-to-left text, the simple way: a line is split into runs of one
//! direction (Hebrew or Arabic letters one way, Latin letters and digits the
//! other), neutral characters take the direction around them, and the runs
//! are put in the order they are read, as the Unicode bidirectional
//! algorithm does for the common cases. Explicit embedding controls,
//! numbers' own separators and Arabic letter joining (shaping) are not
//! handled: Arabic is drawn with its letters unjoined.
//!
//! Text with no right-to-left letter costs one pass over its bytes.

use std::borrow::Cow;

/// Whether `c` is written right to left (Hebrew, Arabic, Syriac, Thaana,
/// N'Ko and their presentation forms).
pub fn is_rtl(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
}

/// Whether `text` has a right-to-left letter. Every one of them is encoded
/// from byte `0xD6` on, so plain text is answered byte by byte.
pub fn has_rtl(text: &str) -> bool {
    text.bytes().any(|b| b >= 0xD6) && text.chars().any(is_rtl)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    L,
    R,
    Neutral,
}

fn class(c: char) -> Class {
    if is_rtl(c) {
        Class::R
    } else if c.is_alphanumeric() {
        Class::L
    } else {
        Class::Neutral
    }
}

/// The mirror image of a bracket, for brackets inside right-to-left runs.
fn mirrored(c: char) -> char {
    match c {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        '‹' => '›',
        '›' => '‹',
        c => c,
    }
}

/// `text` (one line) in the order it is drawn, left to right, on a line
/// whose base direction is right to left (`rtl`) or left to right.
/// Unchanged text is borrowed.
pub fn visual(text: &str, rtl: bool) -> Cow<'_, str> {
    if !has_rtl(text) {
        // All left to right: only neutrals at the ends of a right-to-left
        // line move (`Hello!` reads `!Hello`).
        let edge = |c: Option<char>| c.is_none_or(|c| class(c) != Class::Neutral);
        if !rtl || (edge(text.chars().next()) && edge(text.chars().next_back())) {
            return Cow::Borrowed(text);
        }
    }
    let chars: Vec<char> = text.chars().collect();
    let base = if rtl { Class::R } else { Class::L };
    let mut classes: Vec<Class> = chars.iter().map(|&c| class(c)).collect();
    // Neutrals between two runs of one direction take it; others, the base.
    let mut i = 0;
    while i < classes.len() {
        if classes[i] != Class::Neutral {
            i += 1;
            continue;
        }
        let start = i;
        while i < classes.len() && classes[i] == Class::Neutral {
            i += 1;
        }
        let before = if start == 0 { base } else { classes[start - 1] };
        let after = if i == classes.len() { base } else { classes[i] };
        let resolved = if before == after { before } else { base };
        classes[start..i].fill(resolved);
    }
    // Levels: even is left to right. On a right-to-left line, left-to-right
    // runs go one level up.
    let levels: Vec<u8> = classes
        .iter()
        .map(|&c| match (rtl, c) {
            (false, Class::R) => 1,
            (false, _) => 0,
            (true, Class::L) => 2,
            (true, _) => 1,
        })
        .collect();
    let mut order: Vec<usize> = (0..chars.len()).collect();
    let top = levels.iter().copied().max().unwrap_or(0);
    // From the highest level down to the lowest odd one, reverse every
    // stretch at that level or above.
    for level in (1..=top).rev() {
        let mut i = 0;
        while i < order.len() {
            if levels[order[i]] < level {
                i += 1;
                continue;
            }
            let start = i;
            while i < order.len() && levels[order[i]] >= level {
                i += 1;
            }
            order[start..i].reverse();
        }
    }
    Cow::Owned(order.into_iter().map(|k| if levels[k] % 2 == 1 { mirrored(chars[k]) } else { chars[k] }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_left_alone() {
        assert!(matches!(visual("Hello, world", false), Cow::Borrowed(_)));
        assert!(matches!(visual("Hello", true), Cow::Borrowed(_)));
        assert_eq!(visual("Hello!", true), "!Hello");
    }

    #[test]
    fn hebrew_reads_right_to_left() {
        // שלום עולם: "hello world", stored in reading order.
        assert_eq!(visual("שלום עולם", true), "םלוע םולש");
        assert_eq!(visual("שלום עולם", false), "םלוע םולש");
    }

    #[test]
    fn mixed_runs_keep_their_own_order() {
        // A Latin word inside Hebrew stays readable, in its place.
        assert_eq!(visual("אבג morfo דה", true), "הד morfo גבא");
        // Hebrew inside an English sentence.
        assert_eq!(visual("say אבג now", false), "say גבא now");
        // Numbers inside right-to-left text keep their digits' order.
        assert_eq!(visual("אב 123 גד", true), "דג 123 בא");
    }

    #[test]
    fn brackets_mirror_inside_right_to_left_runs() {
        assert_eq!(visual("(אב)", true), "(בא)");
    }
}
