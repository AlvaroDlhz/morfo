//! Choosing from lists: a drop-down select, a searchable combobox and a
//! command palette that learns which commands each person uses.

use crate::anim::SpringKind::{Smooth, Snappy};
use crate::buttons::{draw_icon, Icon};
use crate::geom::{Rect, Vec2};
use crate::id::Id;
use crate::overlay::Floating;
use crate::theme::{ACCENT, BORDER, MUTED, RAISED, TEXT};
use crate::ui::{Key, Ui, LINE_HEIGHT};

/// Lowercase without accents, for matching typed text against labels.
fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// How well `needle` matches `hay` (both folded), or `None`. A contiguous
/// match beats a scattered one, and an earlier one beats a later one.
fn fuzzy(hay: &str, needle: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    if let Some(pos) = hay.find(needle) {
        return Some(1000 - pos as i32);
    }
    let mut rest = hay.chars();
    let mut gaps = 0;
    for c in needle.chars() {
        loop {
            match rest.next() {
                Some(h) if h == c => break,
                Some(_) => gaps += 1,
                None => return None,
            }
        }
    }
    Some(500 - gaps)
}

impl Ui {
    /// One entry of a drop-down list. Returns `true` when clicked.
    pub(crate) fn option_row(&mut self, id: Id, label: &str, width: f32, highlighted: bool, checked: bool) -> bool {
        let font = self.theme.font;
        let target = self.alloc(Vec2::new(width, (self.theme.control * 0.9).round()));
        let (r, life) = self.enter(id, target, Snappy);
        let hit = self.hit(id, r);
        self.describe(id, crate::a11y::Role::ListItem, label, r, None, Some(checked));
        let (hover, _, mark) = self.drive3(id, hit.hovered || hit.held || highlighted, hit.held, checked as u8 as f32, Snappy);
        let a = self.alpha * life;
        let bg = if highlighted { self.pal[ACCENT].with_alpha(0.16) } else { self.pal[TEXT].with_alpha(0.08) };
        self.draw.rect(r, 10.0, bg.fade(hover * a));
        let pos = Vec2::new(r.x + self.theme.pad * 0.8, r.y + (r.h - font * LINE_HEIGHT) * 0.5);
        self.draw.add_text(pos, font, self.pal[TEXT].fade(a), label);
        if mark > 0.02 {
            let s = font * 0.9 * mark.min(1.2);
            draw_icon(&mut self.draw, Icon::Check, Vec2::new(r.right() - 18.0, r.center().y), s, self.pal[ACCENT].fade(a * mark.min(1.0)));
        }
        if hit.clicked {
            self.learn(id, label);
        }
        hit.clicked
    }

    /// A drop-down select showing the chosen option; it opens a list below
    /// itself, with a check on the current choice. Returns `true` when the
    /// selection changed.
    pub fn select(&mut self, key: &str, options: &[&str], selected: &mut usize) -> bool {
        let id = self.id(key);
        let (font, pad, h) = (self.theme.font, self.theme.pad * 0.8, self.theme.control);
        let widest = options.iter().map(|o| self.text.width(o, font)).fold(0.0, f32::max);
        let target = self.alloc(Vec2::new(widest + pad * 2.0 + 30.0, h));
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        self.describe(id, crate::a11y::Role::ComboBox, key, r, options.get(*selected).copied(), None);
        if hit.clicked {
            self.open = if self.open == Some(id) { None } else { Some(id) };
        }
        let shown = self.open == Some(id);
        let (hover, press, turn) = self.drive3(id, hit.hovered, hit.held, shown as u8 as f32, Smooth);
        let a = self.alpha * life;
        let r = r.scaled(1.0 - 0.03 * press);
        let stroke = self.pal[BORDER].mix(self.pal[TEXT], 0.3 * hover).mix(self.pal[ACCENT], turn.clamp(0.0, 1.0));
        self.draw.rect_stroked(r, self.theme.radius * 0.7, self.pal[RAISED].fade(a), stroke.fade(a), 1.0 + 0.6 * turn.clamp(0.0, 1.0));
        let current = options.get(*selected).copied().unwrap_or("");
        self.draw.add_text(Vec2::new(r.x + pad, r.y + (r.h - font * LINE_HEIGHT) * 0.5), font, self.pal[TEXT].fade(a), current);
        // The chevron flips as the list opens.
        let c = Vec2::new(r.right() - 20.0, r.center().y);
        let s = font * 0.8;
        let flip = turn.clamp(0.0, 1.0);
        self.draw.line(
            c + Vec2::new(-0.3 * s, (-0.12 + 0.24 * flip) * s),
            c + Vec2::new(0.0, (0.18 - 0.36 * flip) * s),
            2.0,
            self.pal[MUTED].fade(a),
        );
        self.draw.line(c + Vec2::new(0.0, (0.18 - 0.36 * flip) * s), c + Vec2::new(0.3 * s, (-0.12 + 0.24 * flip) * s), 2.0, self.pal[MUTED].fade(a));

        let width = r.w - 12.0;
        let mut picked = None;
        let cfg = self.menu_cfg(r);
        let dismissed = self.floating(id.child("panel"), shown, cfg, |ui| {
            for (i, label) in options.iter().enumerate() {
                if ui.option_row(id.child("option").with(i as u64), label, width, false, i == *selected) {
                    picked = Some(i);
                }
            }
        });
        if (picked.is_some() || dismissed) && self.open == Some(id) {
            self.open = None;
        }
        match picked {
            Some(i) if i != *selected => {
                *selected = i;
                self.learn(id, key);
                true
            }
            _ => false,
        }
    }

    /// A text field that suggests matching options as you type (accents
    /// and case don't matter). The arrows move through the suggestions,
    /// Enter or a click picks one. Returns `true` when an option was picked.
    pub fn combobox(&mut self, key: &str, options: &[&str], selected: &mut Option<usize>, query: &mut String, placeholder: &str, width: f32) -> bool {
        let id = self.id(key);
        let state = id.child("state");
        let (up, down) = (self.keys.iter().filter(|k| **k == Key::Up).count(), self.keys.iter().filter(|k| **k == Key::Down).count());
        let edit = self.text_field(key, query, placeholder, width);
        let anchor = self.rect_of(id).unwrap_or_default();
        // Once an option is chosen its full text is in the field: list all.
        let q = if selected.is_some_and(|i| options.get(i) == Some(&query.as_str())) { String::new() } else { fold(query) };
        let mut matches: Vec<(usize, i32)> = options.iter().enumerate().filter_map(|(i, o)| fuzzy(&fold(o), &q).map(|s| (i, s))).collect();
        matches.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let n = self.nodes.entry(state).or_insert_with(crate::ui::Node::new);
        n.touch(self.frame);
        if edit.changed {
            n.index = 0;
        }
        n.index = (n.index + down).saturating_sub(up).min(matches.len().saturating_sub(1));
        let highlight = n.index;
        let shown = edit.focused && !matches.is_empty();
        let mut picked = if edit.submitted && shown { Some(matches[highlight].0) } else { None };
        let cfg = self.menu_cfg(anchor);
        let row_w = anchor.w - 12.0;
        self.floating(id.child("panel"), shown, cfg, |ui| {
            for (n, &(i, _)) in matches.iter().take(8).enumerate() {
                if ui.option_row(id.child("option").with(i as u64), options[i], row_w, n == highlight, *selected == Some(i)) {
                    picked = Some(i);
                }
            }
        });
        if let Some(i) = picked {
            *selected = Some(i);
            query.clear();
            query.push_str(options[i]);
            self.focus = None;
            self.learn(id, key);
        }
        picked.is_some()
    }

    /// A command palette: a search box over `commands` (`(key, label)`),
    /// shown while `open`. Typing filters (accents and case don't matter,
    /// letters may be scattered), the arrows move, Enter or a click runs
    /// one, Escape or a click outside closes it.
    ///
    /// With nothing typed, the commands this person uses most come first:
    /// the palette learns. Returns the index of the command chosen.
    pub fn command_palette(&mut self, key: &str, open: &mut bool, commands: &[(&str, &str)]) -> Option<usize> {
        let id = self.id(key);
        let state = id.child("state");
        let field = id.child("query");
        let now = self.now;
        let n = self.nodes.entry(state).or_insert_with(crate::ui::Node::new);
        n.touch(self.frame);
        if *open && !n.open {
            // Just opened: empty query, focus in the search box.
            self.strings.insert(field, String::new());
            n.index = 0;
            self.focus = Some(field);
            self.cursor = 0;
        }
        n.open = *open;
        let mut index = n.index;
        let mut query = self.strings.remove(&field).unwrap_or_default();

        let q = fold(&query);
        let mut ranked: Vec<(usize, i32, f32)> = commands
            .iter()
            .enumerate()
            .filter_map(|(i, (k, label))| fuzzy(&fold(label), &q).map(|s| (i, s, self.profile.score(id.child(k), now))))
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.total_cmp(&a.2)).then(a.0.cmp(&b.0)));
        ranked.truncate(8);
        let (up, down) = (self.keys.iter().filter(|k| **k == Key::Up).count(), self.keys.iter().filter(|k| **k == Key::Down).count());
        index = (index + down).saturating_sub(up).min(ranked.len().saturating_sub(1));

        let vp = self.viewport();
        let width = (vp.x - 40.0).min(560.0);
        let pos = Vec2::new((vp.x - width) * 0.5, self.top_inset + 56.0);
        let cfg = Floating {
            from: Rect::new(vp.x * 0.5 - 60.0, pos.y, 120.0, 40.0),
            pos,
            center: false,
            width: Some(width),
            inset: 12.0,
            gap: 4.0,
            dim: true,
        };
        let inner = width - 24.0;
        let mut picked = None;
        let mut reset = false;
        let dismissed = self.floating(id.child("panel"), *open, cfg, |ui| {
            let saved = ui.scope;
            ui.scope = id;
            let edit = ui.text_field("query", &mut query, "Escribe un comando…", inner);
            ui.scope = saved;
            reset = edit.changed;
            if edit.submitted && !ranked.is_empty() {
                picked = Some(ranked[index.min(ranked.len() - 1)].0);
            }
            ui.space(2.0);
            for (n, &(i, _, _)) in ranked.iter().enumerate() {
                if ui.option_row(id.child(commands[i].0), commands[i].1, inner, n == index, false) {
                    picked = Some(i);
                }
            }
            if ranked.is_empty() {
                ui.muted("Ningún comando coincide.");
            }
        });
        if reset {
            index = 0;
        }
        if let Some(i) = picked {
            self.learn(id.child(commands[i].0), commands[i].1);
        }
        if picked.is_some() || dismissed {
            *open = false;
            if self.focus == Some(field) {
                self.focus = None;
            }
        }
        let n = self.nodes.get_mut(&state).unwrap();
        n.index = index;
        n.open = *open;
        self.strings.insert(field, query);
        picked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_ignores_accents_and_rewards_contiguity() {
        assert_eq!(fold("Ir a Selección"), "ir a seleccion");
        assert!(fuzzy("ir a seleccion", "selec").is_some());
        assert!(fuzzy("ir a botones", "btns").is_some());
        assert!(fuzzy("ir a botones", "xyz").is_none());
        assert!(fuzzy("botones", "bot") > fuzzy("ir a botones", "bot"));
        assert!(fuzzy("botones", "bot") > fuzzy("botones", "btn"));
    }
}
