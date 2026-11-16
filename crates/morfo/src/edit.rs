//! Text editing shared by single- and multi-line fields: typing, pasting,
//! selecting with Shift or the pointer, the clipboard and caret movement.

use crate::input::floor_boundary;
use crate::ui::Key;

/// The caret and selection of the focused field, plus what it produced.
pub(crate) struct EditState {
    pub cursor: usize,
    pub anchor: Option<usize>,
    pub shift: bool,
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
                    if let Some(j) = prev(value, at) {
                        value.replace_range(j..at, "");
                        at = j;
                        out.changed = true;
                    }
                } else if let Some(j) = next(value, at) {
                    value.replace_range(at..j, "");
                    out.changed = true;
                }
            }
            Key::Left | Key::Right => match (sel, st.shift) {
                // Without Shift, an arrow collapses the selection to that side.
                (Some((a, b)), false) => {
                    at = if key == Key::Left { a } else { b };
                    anchor = None;
                }
                _ => {
                    let to = if key == Key::Left { prev(value, at) } else { next(value, at) }.unwrap_or(at);
                    move_to(&mut at, &mut anchor, to, st.shift);
                }
            },
            Key::Home | Key::End | Key::Up | Key::Down => {
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
            Key::Tab | Key::Space => {}
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
        let mut st = EditState { cursor, anchor, shift, copied: None };
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
}
