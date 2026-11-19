//! Finding your way: tabs, breadcrumbs, a tree, a paginator and a drawer
//! that slides in from the side of the window.

use std::fmt::Write;

use crate::a11y::Role;
use crate::anim::smoothstep;
use crate::anim::SpringKind::{Bouncy, Gentle, Smooth, Snappy};
use crate::buttons::{draw_icon, Icon};
use crate::draw::DrawCmd;
use crate::geom::{Color, Rect, Vec2};
use crate::id::Id;
use crate::theme::{ACCENT, BORDER, MUTED, ON_ACCENT, RAISED, SHADOW, TEXT};
use crate::ui::{Key, Node, Ui, LINE_HEIGHT};
use crate::widgets::{Size, Variant};

/// The edge of the window a [`Ui::drawer`] comes from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    #[default]
    Right,
}

/// `text` cut with "…" to fit `width` at font size `font`.
pub(crate) fn ellipsize(ui: &Ui, text: &str, width: f32, font: f32) -> String {
    if ui.text.width(text, font) <= width {
        return text.to_string();
    }
    let mut out = String::new();
    for c in text.chars() {
        out.push(c);
        out.push('…');
        if ui.text.width(&out, font) > width {
            out.pop();
            out.pop();
            break;
        }
        out.pop();
    }
    out.push('…');
    out
}

/// A small ">" between breadcrumbs.
fn chevron(ui: &mut Ui, c: Vec2, size: f32, color: Color) {
    ui.draw.line(c + Vec2::new(-0.18 * size, -0.32 * size), c + Vec2::new(0.18 * size, 0.0), 1.6, color);
    ui.draw.line(c + Vec2::new(0.18 * size, 0.0), c + Vec2::new(-0.18 * size, 0.32 * size), 1.6, color);
}

impl Ui {
    /// A row of tabs over a line, with an underline that slides and stretches
    /// to the chosen one. The arrows move between tabs when one has the
    /// focus. Pair it with [`morph`](Self::morph) to transform the content
    /// below. Returns `true` when the selection changed.
    pub fn tabs(&mut self, key: &str, labels: &[&str], selected: &mut usize) -> bool {
        let font = self.theme.font;
        let pad = self.theme.pad * 0.85;
        let widths: Vec<f32> = labels.iter().map(|l| self.text.width(l, font) + pad * 2.0).collect();
        self.tabs_impl(key, labels.len(), selected, &widths, labels, |ui, i, cell, t, hover, a| {
            let ink = ui.pal[MUTED].mix(ui.pal[TEXT], t.clamp(0.0, 1.0).max(0.6 * hover));
            let pos = Vec2::new(cell.x + pad, cell.y + (cell.h - font * LINE_HEIGHT) * 0.5);
            ui.draw.add_text(pos, font, ink.fade(a), labels[i]);
        })
    }

    /// Like [`tabs`](Self::tabs), but each tab shows what `f(ui, i,
    /// selected)` builds: an icon and a word, a count badge, two lines.
    /// `names` are the tabs' names for screen readers and habits. A tab is
    /// as wide as its content (it learns the width from the previous
    /// frame), and the content of tab `i` is namespaced under it.
    pub fn tabs_with(&mut self, key: &str, names: &[&str], selected: &mut usize, mut f: impl FnMut(&mut Ui, usize, bool)) -> bool {
        let pad = self.theme.pad * 0.85;
        let id = self.scope.child(key);
        let widths: Vec<f32> =
            (0..names.len()).map(|i| self.sizes.get(&id.with(i as u64).child("morfo-contenido")).map_or(60.0, |s| s.x) + pad * 2.0).collect();
        let current = *selected;
        self.tabs_impl(key, names.len(), selected, &widths, names, |ui, i, cell, _, _, _| {
            let tid = id.with(i as u64);
            let cid = tid.child("morfo-contenido");
            let h = ui.sizes.get(&cid).map_or(0.0, |s| s.y);
            let origin = Vec2::new(cell.x + pad, cell.y + ((cell.h - h) * 0.5).max(0.0));
            let gap = ui.theme.gap * 0.6;
            ui.group(cid, crate::ui::Dir::Row, Vec2::ZERO, gap, Some(origin), |ui| ui.within(tid, |ui| f(ui, i, current == i)));
        })
    }

    /// What [`tabs`](Self::tabs) and [`tabs_with`](Self::tabs_with) share:
    /// the bar, the hits, the keyboard and the underline. `draw(ui, i,
    /// cell, selected, hover, alpha)` paints tab `i`'s content.
    #[allow(clippy::too_many_arguments)]
    fn tabs_impl(
        &mut self,
        key: &str,
        count: usize,
        selected: &mut usize,
        widths: &[f32],
        names: &[&str],
        mut draw: impl FnMut(&mut Ui, usize, Rect, f32, f32, f32),
    ) -> bool {
        let id = self.declare(key);
        let pad = self.theme.pad * 0.85;
        let h = (self.theme.control * 1.05).round();
        let target = self.alloc(Vec2::new(self.available_width(), h));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        self.draw.rect(Rect::new(r.x, r.bottom() - 1.0, r.w, 1.0), 0.0, self.pal[BORDER].fade(a));
        let mut changed = false;
        let mut x = 0.0;
        let mut under = Rect::default();
        if self.a11y_on {
            self.describe(id, Role::TabList, key, r, None, None);
            self.a11y_parents.push(id);
        }
        for (i, &w) in widths.iter().enumerate().take(count) {
            let name = names.get(i).copied().unwrap_or("");
            let tid = id.with(i as u64);
            let cell = Rect::new(r.x + x, r.y, w, h - 2.0);
            self.enter(tid, cell, Snappy);
            let hit = self.hit(tid, cell);
            if hit.clicked && *selected != i {
                *selected = i;
                changed = true;
                self.learn(tid, name);
            }
            if self.focus == Some(tid) && self.arrows(tid) != 0.0 {
                let to = (*selected as i32 + self.arrows(tid) as i32).clamp(0, count as i32 - 1) as usize;
                if to != *selected {
                    *selected = to;
                    changed = true;
                    self.focus = Some(id.with(to as u64));
                }
            }
            let on = *selected == i;
            self.describe(tid, Role::Tab, name, cell, None, Some(on));
            let (hover, _, t) = self.drive3(tid, hit.hovered, hit.held, on as u8 as f32, Smooth);
            self.draw.rect(cell.inset(4.0), 10.0, self.pal[TEXT].with_alpha(0.06 * hover * a));
            draw(self, i, cell, t, hover, a);
            if on {
                under = Rect::new(x + pad * 0.5, h - 3.0, w - pad, 3.0);
            }
            x += w;
        }
        if self.a11y_on {
            self.a11y_parents.pop();
        }
        // The underline lives in the bar's own space, so it follows the bar
        // rigidly while springing from tab to tab.
        let (bouncy, gentle) = (self.steps[Bouncy as usize], self.steps[Gentle as usize]);
        let n = self.nodes.entry(id.with(u64::MAX)).or_insert_with(Node::new);
        self.animating |= n.begin(self.frame, under, &bouncy, &gentle);
        let line = n.rect.get().translate(r.min());
        self.draw.rect(line, 2.0, self.pal[ACCENT].fade(a));
        changed
    }

    /// Where you are, as a path: each step but the last can be clicked to go
    /// back up to it. When the path does not fit, its middle folds into
    /// "…", which unfolds when clicked. Steps slide as the path changes.
    /// Returns the index of the step clicked.
    pub fn breadcrumbs(&mut self, key: &str, path: &[&str]) -> Option<usize> {
        let id = self.declare(key);
        let font = self.theme.font * 0.95;
        let (pad, sep) = (8.0, 18.0);
        let h = (self.theme.control * 0.85).round();
        let width = |ui: &Ui, s: &str| ui.text.width(s, font) + pad * 2.0;
        let avail = self.available_width();
        let expanded = self.nodes.get(&id).is_some_and(|n| n.open);
        // What is shown: every step, or the first, "…" and the last ones.
        let mut shown: Vec<Option<usize>> = (0..path.len()).map(Some).collect();
        let total = |ui: &Ui, s: &[Option<usize>]| -> f32 {
            s.iter().map(|i| i.map_or(width(ui, "…"), |i| width(ui, path[i]))).sum::<f32>() + sep * s.len().saturating_sub(1) as f32
        };
        if !expanded && total(self, &shown) > avail && path.len() > 2 {
            let mut keep = 1;
            while keep + 1 < path.len() {
                let mut trial = vec![Some(0), None];
                trial.extend((path.len() - keep - 1..path.len()).map(Some));
                if total(self, &trial) > avail {
                    break;
                }
                keep += 1;
            }
            shown = vec![Some(0), None];
            shown.extend((path.len() - keep..path.len()).map(Some));
        }
        let target = self.alloc(Vec2::new(total(self, &shown).min(avail.max(0.0)), h));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        let mut clicked = None;
        let mut x = r.x;
        for (k, item) in shown.iter().enumerate() {
            if k > 0 {
                chevron(self, Vec2::new(x + sep * 0.5, r.center().y), font, self.pal[MUTED].fade(a * 0.8));
                x += sep;
            }
            let (text, sid) = match item {
                Some(i) => (path[*i], id.child(path[*i]).with(*i as u64)),
                None => ("…", id.child("morfo-fold")),
            };
            let w = width(self, text);
            let (sr, sl) = self.enter(sid, Rect::new(x, r.y, w, h), Smooth);
            let last = *item == Some(path.len() - 1);
            let hit = if last { None } else { Some(self.hit(sid, sr)) };
            let hovered = hit.as_ref().is_some_and(|h| h.hovered);
            if hit.as_ref().is_some_and(|h| h.clicked) {
                match item {
                    Some(i) => {
                        clicked = Some(*i);
                        self.learn(sid, text);
                    }
                    None => self.nodes.get_mut(&id).unwrap().open = true,
                }
            }
            self.describe(sid, if last { Role::Label } else { Role::Link }, text, sr, None, None);
            let (hover, _, _) = self.drive3(sid, hovered, false, 0.0, Snappy);
            let sa = a * sl.clamp(0.0, 1.0);
            self.draw.rect(sr, 8.0, self.pal[TEXT].with_alpha(0.07 * hover * sa));
            let ink = if last { self.pal[TEXT] } else { self.pal[MUTED].mix(self.pal[TEXT], hover) };
            self.draw.add_text(Vec2::new(sr.x + pad, sr.y + (h - font * LINE_HEIGHT) * 0.5), font, ink.fade(sa), text);
            x += w;
        }
        // A new path folds again.
        if clicked.is_some() {
            self.nodes.get_mut(&id).unwrap().open = false;
        }
        clicked
    }

    /// A tree of nested items given as `(depth, label)` in reading order: an
    /// item's children follow it with a greater depth. Branches open and
    /// close with a click (or the right and left arrows), their children
    /// sliding out; the library remembers which ones are open. Returns
    /// `true` when the selection changed; `selected` indexes `items`.
    pub fn tree(&mut self, key: &str, items: &[(u8, &str)], selected: &mut Option<usize>) -> bool {
        let id = self.declare(key);
        if self.a11y_on {
            let rect = self.layout_rect(id).unwrap_or_default();
            self.describe(id, Role::Tree, key, rect, None, None);
            self.a11y_parents.push(id);
        }
        let mut changed = false;
        // A selection set from outside (or the first one) opens its branch.
        let n = self.nodes.entry(id).or_insert_with(Node::new);
        n.touch(self.frame);
        let mark = selected.map_or(0, |s| s + 1);
        if n.index != mark {
            n.index = mark;
            if let Some(s) = selected.filter(|&s| s < items.len()) {
                let mut depth = items[s].0;
                let mut parents = Vec::new();
                for j in (0..s).rev() {
                    if items[j].0 < depth {
                        depth = items[j].0;
                        parents.push(j);
                    }
                }
                let mut path = id;
                for &j in parents.iter().rev() {
                    path = path.child(items[j].1);
                    self.nodes.entry(path).or_insert_with(Node::new).open = true;
                }
            }
        }
        let depth = items.first().map_or(0, |i| i.0);
        let gap = self.layouts.last().unwrap().gap;
        self.layouts.last_mut().unwrap().gap = 2.0;
        self.tree_level(id, items, 0, depth, selected, &mut changed);
        self.layouts.last_mut().unwrap().gap = gap;
        if changed {
            // Chosen here: nothing to open.
            self.nodes.get_mut(&id).unwrap().index = selected.map_or(0, |s| s + 1);
        }
        if self.a11y_on {
            self.a11y_parents.pop();
        }
        changed
    }

    fn tree_level(&mut self, parent: Id, items: &[(u8, &str)], start: usize, depth: u8, selected: &mut Option<usize>, changed: &mut bool) -> usize {
        const INDENT: f32 = 20.0;
        let font = self.theme.font * 0.95;
        let h = (self.theme.control * 0.82).round();
        let mut i = start;
        while i < items.len() && items[i].0 >= depth {
            let (d, label) = items[i];
            let mut end = i + 1;
            while end < items.len() && items[end].0 > d {
                end += 1;
            }
            let branch = end > i + 1;
            let nid = parent.child(label);
            let target = self.alloc(Vec2::new(self.available_width(), h));
            let (r, life) = self.enter(nid, target, Smooth);
            let hit = self.hit(nid, r);
            let n = self.nodes.get_mut(&nid).unwrap();
            let mut open = n.open;
            if hit.clicked {
                if branch {
                    open = !open;
                }
                if *selected != Some(i) {
                    *selected = Some(i);
                    *changed = true;
                }
                self.learn(nid, label);
            }
            if branch && self.focus == Some(nid) {
                if self.keys.contains(&Key::Right) {
                    open = true;
                }
                if self.keys.contains(&Key::Left) {
                    open = false;
                }
            }
            self.nodes.get_mut(&nid).unwrap().open = open;
            let on = *selected == Some(i);
            self.describe(nid, Role::TreeItem, label, r, None, Some(on));
            if branch {
                self.describe_expanded(open);
            }
            let (hover, _, t) = self.drive3(nid, hit.hovered, hit.held, open as u8 as f32, Smooth);
            let sel = {
                let k = self.steps[Snappy as usize];
                let n = self.nodes.get_mut(&nid).unwrap();
                let fresh = n.fresh();
                self.animating |= n.e1.drive(on as u8 as f32, fresh, &k);
                n.e1.get().clamp(0.0, 1.0)
            };
            let a = self.alpha * life;
            let bg = self.pal[TEXT].with_alpha(0.06 * hover).mix(self.pal[ACCENT].with_alpha(0.16), sel);
            self.draw.rect(r, 8.0, bg.fade(a));
            // Guides: one thin line per level above this one.
            let level = (d - items[0].0.min(d)) as f32;
            for g in 0..level as usize {
                let gx = r.x + 14.0 + g as f32 * INDENT;
                self.draw.line(Vec2::new(gx, r.y - 1.0), Vec2::new(gx, r.bottom() + 1.0), 1.0, self.pal[BORDER].fade(a));
            }
            let c = Vec2::new(r.x + 14.0 + level * INDENT, r.center().y);
            let ink = self.pal[MUTED].mix(self.pal[TEXT], (hover + sel).min(1.0));
            if branch {
                // ">" turning into "v" as the branch opens.
                let turn = |closed: (f32, f32), opened: (f32, f32)| {
                    c + Vec2::new(closed.0 + (opened.0 - closed.0) * t, closed.1 + (opened.1 - closed.1) * t)
                };
                let (p0, p1, p2) = (turn((-2.5, -5.0), (-5.0, -2.5)), turn((2.5, 0.0), (0.0, 2.5)), turn((-2.5, 5.0), (5.0, -2.5)));
                self.draw.line(p0, p1, 2.0, ink.fade(a));
                self.draw.line(p1, p2, 2.0, ink.fade(a));
            } else {
                self.draw.rect(Rect::new(c.x - 2.5, c.y - 2.5, 5.0, 5.0), 5.0, ink.fade(a * 0.7));
            }
            let text_color = self.pal[TEXT].mix(self.pal[ACCENT], 0.35 * sel);
            self.draw.add_text(Vec2::new(c.x + 14.0, r.y + (h - font * LINE_HEIGHT) * 0.5), font, text_color.fade(a), label);
            if branch {
                self.reveal_id(nid.child("morfo-children"), open, |ui| {
                    ui.layouts.last_mut().unwrap().gap = 2.0;
                    ui.tree_level(nid, items, i + 1, d + 1, selected, changed);
                });
            }
            i = end;
        }
        i
    }

    /// Page numbers with arrows: `‹ 1 … 4 5 6 … 20 ›`. The highlight springs
    /// to the current page and the numbers slide as the window of pages
    /// moves. `current` counts from 0. Returns `true` when it changed.
    pub fn pagination(&mut self, key: &str, pages: usize, current: &mut usize) -> bool {
        let id = self.declare(key);
        if pages == 0 {
            return false;
        }
        *current = (*current).min(pages - 1);
        let d = (self.theme.control * 0.9).round();
        let gap = 4.0;
        // Which pages are shown: the ends, the current one and its
        // neighbours, and "…" over what is skipped (always 7 slots at most).
        let mut shown: Vec<Option<usize>> = Vec::new();
        if pages <= 7 {
            shown.extend((0..pages).map(Some));
        } else {
            let (lo, hi) = if *current <= 3 {
                (1, 4)
            } else if *current >= pages - 4 {
                (pages - 5, pages - 2)
            } else {
                (*current - 1, *current + 1)
            };
            shown.push(Some(0));
            shown.push(if lo > 1 { None } else { Some(1) });
            shown.extend((lo.max(2)..=hi.min(pages - 3)).map(Some));
            shown.push(if hi < pages - 2 { None } else { Some(pages - 2) });
            shown.push(Some(pages - 1));
            shown.dedup();
        }
        let slots = shown.len() + 2;
        let target = self.alloc(Vec2::new(slots as f32 * d + (slots - 1) as f32 * gap, d));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        let font = self.theme.font * 0.95;
        let mut go = None;
        let slot = |k: usize| Rect::new(r.x + k as f32 * (d + gap), r.y, d, d);
        let indicator = slot(1 + shown.iter().position(|s| *s == Some(*current)).unwrap_or(0));
        let (bouncy, gentle) = (self.steps[Bouncy as usize], self.steps[Gentle as usize]);
        let n = self.nodes.entry(id.child("morfo-current")).or_insert_with(Node::new);
        self.animating |= n.begin(self.frame, indicator.translate(Vec2::new(-r.x, -r.y)), &bouncy, &gentle);
        let ind = n.rect.get().translate(r.min());
        self.draw.rect(ind, d, self.pal[ACCENT].fade(a));

        let mut text = String::new();
        for (k, item) in shown.iter().enumerate() {
            let cell = slot(k + 1);
            match item {
                Some(p) => {
                    let pid = id.child("page").with(*p as u64);
                    let (pr, pl) = self.enter(pid, cell, Smooth);
                    let hit = self.hit(pid, pr);
                    text.clear();
                    let _ = write!(text, "{}", p + 1);
                    self.describe(pid, Role::Button, &text, pr, None, Some(*p == *current));
                    if hit.clicked {
                        go = Some(*p);
                    }
                    let (hover, press, on) = self.drive3(pid, hit.hovered, hit.held, (*p == *current) as u8 as f32, Snappy);
                    let pa = a * pl.clamp(0.0, 1.0);
                    self.draw.rect(pr.scaled(1.0 - 0.08 * press), d, self.pal[TEXT].with_alpha(0.08 * hover * (1.0 - on) * pa));
                    let ink = self.pal[TEXT].mix(self.pal[ON_ACCENT], on.clamp(0.0, 1.0));
                    let w = self.text.width(&text, font);
                    self.draw.add_text(Vec2::new(pr.center().x - w * 0.5, pr.y + (d - font * LINE_HEIGHT) * 0.5), font, ink.fade(pa), &text);
                }
                None => {
                    let gid = id.child("gap").with((k > shown.len() / 2) as u64);
                    let (gr, gl) = self.enter(gid, cell, Smooth);
                    let w = self.text.width("…", font);
                    self.draw.add_text(
                        Vec2::new(gr.center().x - w * 0.5, gr.y + (d - font * LINE_HEIGHT) * 0.5),
                        font,
                        self.pal[MUTED].fade(a * gl),
                        "…",
                    );
                }
            }
        }
        for (k, icon, to) in [(0, Icon::ArrowLeft, current.checked_sub(1)), (slots - 1, Icon::ArrowRight, Some(*current + 1).filter(|&p| p < pages))]
        {
            let aid = id.child("arrow").with(k as u64);
            let cell = slot(k);
            self.enter(aid, cell, Snappy);
            let hit = self.hit(aid, cell);
            let name = if k == 0 { self.locale.previous } else { self.locale.next };
            self.describe(aid, Role::Button, name, cell, None, None);
            if hit.clicked && to.is_some() {
                go = to;
            }
            let enabled = to.is_some();
            let (hover, _, _) = self.drive3(aid, hit.hovered && enabled, false, 0.0, Snappy);
            self.draw.rect(cell, d, self.pal[TEXT].with_alpha(0.08 * hover * a));
            let ink = self.pal[TEXT].fade(a * if enabled { 0.9 } else { 0.25 });
            draw_icon(&mut self.draw, icon, cell.center(), font * 0.95, ink);
        }
        match go {
            Some(to) if to != *current => {
                *current = to;
                self.learn(id, self.locale.page);
                true
            }
            _ => false,
        }
    }

    /// A panel that slides in from one side of the window over the content,
    /// which darkens behind it. It has a title and a close button; clicking
    /// outside it or pressing Escape also closes it (`open` becomes false).
    /// Keys declared in `f` are namespaced under `key`.
    pub fn drawer(&mut self, key: &str, title: &str, side: Side, open: &mut bool, width: f32, f: impl FnOnce(&mut Ui)) {
        let id = self.declare(key);
        let p = self.presence_id(id.child("presence"), *open, Smooth);
        if p <= 0.001 && !*open {
            return;
        }
        let pc = p.clamp(0.0, 1.0);
        let vp = self.viewport();
        let top = self.top_inset;
        let w = width.min(vp.x - 24.0).max(0.0);
        let shift = (1.0 - p) * (w + 40.0);
        let panel = match side {
            Side::Right => Rect::new(vp.x - w + shift, top, w, vp.y - top),
            Side::Left => Rect::new(-shift, top, w, vp.y - top),
        };
        let screen = Rect::new(0.0, top, vp.x, vp.y - top);
        let pad = self.theme.pad;
        let mut close = false;
        let shown = *open;
        let saved = self.scope;
        self.detached(1, id.with(0), panel.min() + Vec2::splat(pad), w - pad * 2.0, |ui| {
            ui.draw.rect(screen, 0.0, Color::BLACK.with_alpha(0.35 * pc));
            // The outer corners go past the window edge: only the inner ones show.
            let radius = ui.theme.radius;
            let body = match side {
                Side::Right => Rect::new(panel.x, panel.y, panel.w + radius, panel.h),
                Side::Left => Rect::new(panel.x - radius, panel.y, panel.w + radius, panel.h),
            };
            let dx = if side == Side::Right { -8.0 } else { 8.0 };
            ui.draw.push(DrawCmd::Shadow { rect: body, radius, blur: 36.0, offset: Vec2::new(dx, 0.0), color: ui.pal[SHADOW].fade(pc) });
            ui.draw.rect_stroked(body, radius, ui.pal[RAISED], ui.pal[BORDER], 1.0);
            ui.draw.push(DrawCmd::PushClip { rect: panel, radius: 0.0 });
            ui.clip = Some(panel);
            ui.alpha = smoothstep((pc - 0.2) / 0.8);
            ui.scope = id;
            ui.row(|ui| {
                ui.title(title);
                ui.fill();
                close = ui.icon_button("morfo-close", Icon::Close, Variant::Ghost, Size::Sm).clicked;
            });
            ui.separator();
            ui.describe_group(id, Role::Dialog, title, f);
            ui.draw.push(DrawCmd::PopClip);
            ui.clip = None;
            if shown {
                ui.blockers.push((1, screen));
                ui.hit_inert(id.with(1), panel);
                close |= ui.hit_inert(id.with(2), screen).clicked;
            }
        });
        self.scope = saved;
        if shown && (close || self.keys.contains(&Key::Escape)) {
            *open = false;
            self.animating = true;
        }
    }
}
