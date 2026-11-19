//! Text editing shared by single- and multi-line fields: typing, pasting,
//! selecting with Shift or the pointer, the clipboard and caret movement.

use crate::id::{Id, IdMap};
use crate::input::floor_boundary;
use crate::ui::Key;

/// What a text field was before each edit, to go back and forth.
#[derive(Default)]
pub(crate) struct History {
    undo: Vec<(String, usize)>,
    redo: Vec<(String, usize)>,
    /// When the last edit was, and whether it was plain typing.
    last: f64,
    typing: bool,
}

/// Most edits a field remembers.
const HISTORY: usize = 100;

/// Undoes or redoes, if this frame's keys ask to, on the field `id` whose
/// caret is at `cursor`. Returns where the caret goes.
pub(crate) fn undo_redo(histories: &mut IdMap<History>, id: Id, keys: &[Key], value: &mut String, cursor: usize) -> Option<usize> {
    let mut caret = None;
    let mut cursor = cursor;
    for &k in keys {
        let h = histories.entry(id).or_default();
        let (from, to) = match k {
            Key::Undo => (&mut h.undo, &mut h.redo),
            Key::Redo => (&mut h.redo, &mut h.undo),
            _ => continue,
        };
        if let Some((text, at)) = from.pop() {
            let now = std::mem::replace(value, text);
            // Going the other way puts the caret back where it is now.
            to.push((now, cursor));
            cursor = at.min(value.len());
            caret = Some(cursor);
            // The next keystroke starts a step of its own.
            h.typing = false;
        }
    }
    caret
}

/// Remembers what field `id` was (`before`, caret at `at`) before an edit.
/// Typing without pausing for a second is one step.
pub(crate) fn remember(histories: &mut IdMap<History>, id: Id, before: String, at: usize, now: f64, typing: bool) {
    let h = histories.entry(id).or_default();
    let same_run = typing && h.typing && now - h.last < 1.0;
    if !same_run {
        h.undo.push((before, at));
        if h.undo.len() > HISTORY {
            h.undo.remove(0);
        }
    }
    h.redo.clear();
    h.typing = typing;
    h.last = now;
}

/// The caret and selection of the focused field, plus what it produced.
pub(crate) struct EditState {
    pub cursor: usize,
    pub anchor: Option<usize>,
    pub shift: bool,
    /// Ctrl is held: the arrows, Backspace and Delete go by whole words.
    pub word: bool,
    /// Text copied or cut, for the system clipboard.
    pub copied: Option<String>,
}

#[derive(Default)]
pub(crate) struct EditOutcome {
    pub changed: bool,
    pub submitted: bool,
    pub escaped: bool,
    pub touched: bool,
}

/// The selected byte range, if any.
pub(crate) fn selection(at: usize, anchor: Option<usize>) -> Option<(usize, usize)> {
    anchor.filter(|&a| a != at).map(|a| (a.min(at), a.max(at)))
}

fn prev(s: &str, at: usize) -> Option<usize> {
    s[..at].char_indices().next_back().map(|(j, _)| j)
}

fn next(s: &str, at: usize) -> Option<usize> {
    s[at..].chars().next().map(|c| at + c.len_utf8())
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The start of the word before `at` (skipping the spaces before it).
pub(crate) fn word_before(s: &str, at: usize) -> usize {
    let mut i = at;
    let mut seen = false;
    for (j, c) in s[..at].char_indices().rev() {
        if is_word(c) {
            seen = true;
        } else if seen {
            break;
        }
        i = j;
    }
    i
}

/// The end of the word after `at` (skipping the spaces before it).
pub(crate) fn word_after(s: &str, at: usize) -> usize {
    let mut seen = false;
    for (j, c) in s[at..].char_indices() {
        if is_word(c) {
            seen = true;
        } else if seen {
            return at + j;
        }
    }
    s.len()
}

/// The word around `at`: the range to select on a double click.
pub(crate) fn word_at(s: &str, at: usize) -> (usize, usize) {
    let start = s[..at].char_indices().rev().take_while(|&(_, c)| is_word(c)).last().map_or(at, |(j, _)| j);
    let end = s[at..].char_indices().find(|&(_, c)| !is_word(c)).map_or(s.len(), |(j, _)| at + j);
    (start, end)
}

/// Moves the caret to `to`, extending the selection if `extend`.
fn move_to(at: &mut usize, anchor: &mut Option<usize>, to: usize, extend: bool) {
    if extend {
        anchor.get_or_insert(*at);
    } else {
        *anchor = None;
    }
    *at = to;
}

/// Replaces the selection (or inserts at the caret) with `text`.
fn replace(value: &mut String, at: &mut usize, anchor: &mut Option<usize>, text: &str) {
    if let Some((a, b)) = selection(*at, *anchor) {
        value.replace_range(a..b, "");
        *at = a;
    }
    *anchor = None;
    value.insert_str(*at, text);
    *at += text.len();
}

/// Applies this frame's typing, pasting and keys to `value`. `nav` resolves
/// Home, End, Up and Down to a new caret position (or `None` to ignore).
pub(crate) fn edit_text(
    st: &mut EditState,
    keys: &[Key],
    typed: &str,
    pasted: &str,
    value: &mut String,
    multiline: bool,
    mut nav: impl FnMut(&str, usize, Key) -> Option<usize>,
) -> EditOutcome {
    let mut out = EditOutcome::default();
    let mut at = floor_boundary(value, st.cursor);
    let mut anchor = st.anchor.map(|a| floor_boundary(value, a));
    if !typed.is_empty() {
        replace(value, &mut at, &mut anchor, typed);
        out.changed = true;
        out.touched = true;
    }
    if !pasted.is_empty() {
        let text = if multiline { pasted.to_string() } else { pasted.replace('\n', " ") };
        replace(value, &mut at, &mut anchor, &text);
        out.changed = true;
        out.touched = true;
    }
    for &key in keys {
        out.touched = true;
        let sel = selection(at, anchor);
        match key {
            Key::Backspace | Key::Delete => {
                if let Some((a, b)) = sel {
                    value.replace_range(a..b, "");
                    at = a;
                    anchor = None;
                    out.changed = true;
                } else if key == Key::Backspace {
                    let j = if st.word { Some(word_before(value, at)).filter(|&j| j < at) } else { prev(value, at) };
                    if let Some(j) = j {
                        value.replace_range(j..at, "");
                        at = j;
                        out.changed = true;
                    }
                } else {
                    let j = if st.word { Some(word_after(value, at)).filter(|&j| j > at) } else { next(value, at) };
                    if let Some(j) = j {
                        value.replace_range(at..j, "");
                        out.changed = true;
                    }
                }
            }
            Key::Left | Key::Right => match (sel, st.shift) {
                // Without Shift, an arrow collapses the selection to that side.
                (Some((a, b)), false) => {
                    at = if key == Key::Left { a } else { b };
                    anchor = None;
                }
                _ => {
                    let to = match (key == Key::Left, st.word) {
                        (true, true) => word_before(value, at),
                        (false, true) => word_after(value, at),
                        (true, false) => prev(value, at).unwrap_or(at),
                        (false, false) => next(value, at).unwrap_or(at),
                    };
                    move_to(&mut at, &mut anchor, to, st.shift);
                }
            },
            Key::Home | Key::End | Key::Up | Key::Down | Key::PageUp | Key::PageDown => {
                if let Some(to) = nav(value, at, key) {
                    move_to(&mut at, &mut anchor, to, st.shift);
                }
            }
            Key::Enter => {
                if multiline {
                    replace(value, &mut at, &mut anchor, "\n");
                    out.changed = true;
                } else {
                    out.submitted = true;
                }
            }
            Key::Escape => out.escaped = true,
            Key::SelectAll => {
                anchor = Some(0);
                at = value.len();
            }
            Key::Copy | Key::Cut => {
                if let Some((a, b)) = sel {
                    st.copied = Some(value[a..b].to_string());
                    if key == Key::Cut {
                        value.replace_range(a..b, "");
                        at = a;
                        anchor = None;
                        out.changed = true;
                    }
                }
            }
            // Undo and redo need the field's history: the field does them.
            Key::Tab | Key::Space | Key::Undo | Key::Redo => {}
        }
    }
    st.cursor = at;
    st.anchor = anchor;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        value: &str,
        cursor: usize,
        anchor: Option<usize>,
        shift: bool,
        keys: &[Key],
        typed: &str,
    ) -> (String, usize, Option<usize>, Option<String>) {
        let mut v = value.to_string();
        let mut st = EditState { cursor, anchor, shift, word: false, copied: None };
        edit_text(&mut st, keys, typed, "", &mut v, false, |v, _, k| match k {
            Key::Home => Some(0),
            Key::End => Some(v.len()),
            _ => None,
        });
        (v, st.cursor, st.anchor, st.copied)
    }

    #[test]
    fn selecting_and_replacing() {
        // Shift+Left twice selects "lo"; typing replaces it.
        let (v, at, _, _) = run("hello", 5, None, true, &[Key::Left, Key::Left], "");
        assert_eq!((v.as_str(), at), ("hello", 3));
        let (v, _, anchor, _) = run("hello", 3, Some(5), false, &[], "p!");
        assert_eq!((v.as_str(), anchor), ("help!", None));
    }

    #[test]
    fn clipboard_and_select_all() {
        let (_, _, _, copied) = run("añadir", 0, None, false, &[Key::SelectAll, Key::Copy], "");
        assert_eq!(copied.as_deref(), Some("añadir"));
        let (v, at, _, copied) = run("uno dos", 4, Some(7), false, &[Key::Cut], "");
        assert_eq!((v.as_str(), at, copied.as_deref()), ("uno ", 4, Some("dos")));
        // An arrow without Shift collapses the selection.
        let (_, at, anchor, _) = run("abcdef", 1, Some(4), false, &[Key::Right], "");
        assert_eq!((at, anchor), (4, None));
    }

    #[test]
    fn words_move_delete_and_select() {
        let s = "hola, mundo_bonito  ya";
        assert_eq!(word_after(s, 0), 4);
        assert_eq!(word_after(s, 4), 18);
        assert_eq!(word_before(s, s.len()), 20);
        assert_eq!(word_before(s, 20), 6);
        assert_eq!(word_at(s, 8), (6, 18));
        let mut v = "uno dos tres".to_string();
        let mut st = EditState { cursor: v.len(), anchor: None, shift: false, word: true, copied: None };
        edit_text(&mut st, &[Key::Backspace], "", "", &mut v, false, |_, _, _| None);
        assert_eq!((v.as_str(), st.cursor), ("uno dos ", 8));
        edit_text(&mut st, &[Key::Left], "", "", &mut v, false, |_, _, _| None);
        assert_eq!(st.cursor, 4);
    }
}
