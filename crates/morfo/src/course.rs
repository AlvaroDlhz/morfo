//! A side index shaped like a course syllabus: modules, and inside them
//! numbered lessons strung along a timeline that fills up as they are
//! visited.

use std::fmt::Write;

use crate::anim::smoothstep;
use crate::anim::SpringKind::{Smooth, Snappy};
use crate::buttons::{draw_icon, Icon};
use crate::draw::DrawCmd;
use crate::geom::{Rect, Vec2};
use crate::overlay::lerp_rect;
use crate::theme::{ACCENT, BORDER, MUTED, ON_ACCENT, RAISED, SHADOW, TEXT};
use crate::ui::{Node, Ui, LINE_HEIGHT};

/// Width of the strip at the left edge that opens the index.
const EDGE: f32 = 18.0;
/// Seconds it stays open after the pointer leaves.
const LINGER: f64 = 0.3;
/// Height of a lesson row, a module header, and a lesson circle.
const ROW: f32 = 36.0;
const HEADER: f32 = 50.0;
const DOT: f32 = 24.0;

impl Ui {
    /// A course syllabus hidden at the left edge of the window, like
    /// [`side_nav`](Self::side_nav): a slim rail that unfolds into a panel
    /// when the pointer reaches the edge.
    ///
    /// `modules` are `(title, lessons)`. Lessons are numbered across the
    /// whole course and strung on a timeline: visited ones get a check and
    /// fill the line up to them, the current one is highlighted, the rest
    /// wait their turn. `selected` indexes the lessons in that order.
    /// Visits are remembered in the usage profile, so progress survives
    /// restarts when the profile is saved. Returns `true` when the
    /// selection changed.
    pub fn course_nav(&mut self, key: &str, title: &str, modules: &[(&str, &[&str])], selected: &mut usize) -> bool {
        let id = self.id(key);
        let vp = self.viewport();
        let theme = self.theme;
        let font = theme.font;
        let total: usize = modules.iter().map(|m| m.1.len()).sum();
        if total == 0 {
            return false;
        }
        *selected = (*selected).min(total - 1);

        // Where everything goes, in content coordinates (y from the top).
        let mut lessons = Vec::with_capacity(total); // (module, label, y)
        let mut headers = Vec::with_capacity(modules.len()); // y
        let intro = font * 1.15 * LINE_HEIGHT + font * 0.9 * LINE_HEIGHT + 34.0;
        let mut y = intro;
        for (m, (_, list)) in modules.iter().enumerate() {
            headers.push(y);
            y += HEADER;
            for label in list.iter() {
                lessons.push((m, *label, y));
                y += ROW;
            }
            y += 8.0;
        }
        let content_h = y + theme.pad * 0.5;
        let widest = lessons.iter().map(|l| self.text.width(l.1, font)).fold(self.text.width(title, font * 1.15), f32::max);
        let width = (widest + DOT + theme.pad * 2.0 + 64.0).clamp(260.0, (vp.x - 32.0).max(120.0));
        let top = self.top_inset;
        let avail = (vp.y - top - 16.0).max(0.0);
        let full = Rect::new(8.0, top + 8.0, width, content_h.min(avail));

        // The collapsed rail: one dot per lesson, a little gap between modules.
        let rail_h = total as f32 * 11.0 + (modules.len() - 1) as f32 * 8.0 + 16.0;
        let rail = Rect::new(5.0, (top + (vp.y - top - rail_h) * 0.5).max(top + 8.0), 9.0, rail_h);
        let rail_dot = |i: usize, m: usize| Vec2::new(rail.center().x, rail.y + 10.0 + i as f32 * 11.0 + m as f32 * 8.0);

        // Open while the pointer is at the edge or on the panel; linger a
        // moment after it leaves; after a choice, stay shut until it leaves.
        let (now, pointer) = (self.now, self.input.pointer);
        let n = self.nodes.entry(id).or_insert_with(Node::new);
        n.touch(self.frame);
        let at_edge = pointer.is_some_and(|p| (p.x < EDGE && p.y > top) || rail.inset(-6.0).contains(p));
        let on_panel = n.open && pointer.is_some_and(|p| full.inset(-12.0).contains(p));
        if n.latch && !at_edge && !on_panel {
            n.latch = false;
        }
        let wanted = (at_edge || on_panel) && !n.latch;
        if wanted {
            n.stamp = now;
        }
        let lingering = !wanted && !n.latch && n.open && now - n.stamp < LINGER;
        let was_open = n.open;
        n.open = wanted || lingering;
        let (open, since) = (n.open, n.stamp);
        if lingering {
            self.wake_at(since + LINGER);
        }

        // Scrolling, when the syllabus is taller than the window.
        let max_scroll = (content_h - full.h).max(0.0);
        let wheel = if open && on_panel { std::mem::take(&mut self.input.wheel) } else { 0.0 };
        let k = self.steps[Snappy as usize];
        let n = self.nodes.get_mut(&id).unwrap();
        if open && !was_open {
            // Opening: bring the current lesson into view.
            let cy = lessons[*selected].2 + ROW * 0.5;
            n.e1.snap((cy - full.h * 0.5).clamp(0.0, max_scroll));
        }
        n.e1.set((n.e1.target() - wheel).clamp(0.0, max_scroll));
        self.animating |= n.e1.tick(&k);
        let scroll = n.e1.get();

        // The current lesson counts as visited (once): progress is learned.
        let row_id = |i: usize| id.with(i as u64);
        if self.profile.count(row_id(*selected)) == 0 {
            let label = lessons[*selected].1;
            self.learn(row_id(*selected), label);
        }
        let visited: Vec<bool> = (0..total).map(|i| i == *selected || self.profile.count(row_id(i)) > 0).collect();
        let done = visited.iter().filter(|&&v| v).count();

        let p = self.presence_id(id.child("presence"), open, Smooth);
        let pc = p.clamp(0.0, 1.0);
        let ink = smoothstep((pc - 0.35) / 0.65);
        let below = self.draw.set_layer(1);
        let (alpha, clip, cull) = (self.alpha, self.clip, self.draw.cull.take());
        self.alpha = 1.0;
        self.clip = None;
        let pal = self.pal;

        // The rail grows into the panel.
        let panel = lerp_rect(rail, full, p);
        let radius = 4.5 + (theme.radius - 4.5) * pc;
        let hover_rail = at_edge && !open;
        self.draw.push(DrawCmd::Shadow { rect: panel, radius, blur: 30.0, offset: Vec2::new(6.0, 10.0), color: pal[SHADOW].fade(pc) });
        let fill = pal[BORDER].mix(pal[TEXT], if hover_rail { 0.2 } else { 0.0 }).mix(pal[RAISED], pc);
        self.draw.rect_stroked(panel, radius, fill, pal[BORDER].fade(pc), 1.0);
        self.draw.push(DrawCmd::PushClip { rect: panel, radius: 0.0 });
        self.clip = Some(panel);

        let x0 = full.x + theme.pad;
        let at = |cy: f32| full.y + theme.pad * 0.6 + cy - scroll;
        // Course header: title, size, and progress.
        let mut text = String::new();
        self.draw.add_text(Vec2::new(x0, at(0.0)), font * 1.15, pal[TEXT].fade(ink), title);
        let _ = write!(text, "{} módulos · {} lecciones", modules.len(), total);
        self.draw.add_text(Vec2::new(x0, at(font * 1.15 * LINE_HEIGHT)), font * 0.9, pal[MUTED].fade(ink), &text);
        let bar_y = at(font * 1.15 * LINE_HEIGHT + font * 0.9 * LINE_HEIGHT + 10.0);
        let bar_w = full.w - theme.pad * 2.0 - 64.0;
        let (_, _, shown_done) = {
            let pid = id.child("progreso");
            self.enter(pid, Rect::new(x0, bar_y, bar_w, 6.0), Snappy);
            self.drive3(pid, false, false, done as f32 / total as f32, Smooth)
        };
        self.draw.rect(Rect::new(x0, bar_y, bar_w, 6.0), 3.0, pal[BORDER].fade(ink));
        self.draw.rect(Rect::new(x0, bar_y, (bar_w * shown_done).max(6.0), 6.0), 3.0, pal[ACCENT].fade(ink));
        text.clear();
        let _ = write!(text, "{done}/{total}");
        let tw = self.text.width(&text, font * 0.85);
        self.draw.add_text(
            Vec2::new(full.right() - theme.pad - tw, bar_y - font * 0.85 * LINE_HEIGHT * 0.5 + 3.0),
            font * 0.85,
            pal[MUTED].fade(ink),
            &text,
        );

        // Where each lesson's circle sits, from the rail dot to the panel.
        let circle = |i: usize| {
            let (m, _, ly) = lessons[i];
            let open_c = Rect::new(x0, at(ly) + (ROW - DOT) * 0.5, DOT, DOT);
            let d = rail_dot(i, m);
            let closed_c = Rect::new(d.x - 2.5, d.y - 2.5, 5.0, 5.0);
            lerp_rect(closed_c, open_c, p)
        };

        // The timeline: a line through every circle, filled up to the last
        // lesson visited in a row from the start.
        let line_w = 2.0 + 0.5 * pc;
        // From the first module's milestone down to the first lesson.
        let first_mid = at(headers[0]) + 6.0 + font * 0.72 * LINE_HEIGHT + font * 0.98 * LINE_HEIGHT * 0.5;
        let start = Vec2::new(x0 + DOT * 0.5, first_mid);
        let to = circle(0).center();
        self.draw.line(start, Vec2::new(start.x, to.y), line_w, pal[ACCENT].with_alpha(0.75).fade(ink));
        for i in 0..total - 1 {
            let (a, b) = (circle(i).center(), circle(i + 1).center());
            let reached = visited[i] && visited[i + 1];
            let color = if reached { pal[ACCENT].with_alpha(0.75) } else { pal[BORDER].mix(pal[MUTED], 0.25) };
            self.draw.line(a, b, line_w, color.fade(0.6 + 0.4 * pc));
        }

        // Module headers, aligned with the lessons; on the line, a milestone
        // that fills in once the whole module has been seen.
        let mut first = 0;
        let text_x = x0 + DOT + 12.0;
        for (m, (name, list)) in modules.iter().enumerate() {
            let hy = at(headers[m]) + 6.0;
            let seen = (first..first + list.len()).filter(|&i| visited[i]).count();
            let complete = seen == list.len();
            let mid = hy + font * 0.72 * LINE_HEIGHT + font * 0.98 * LINE_HEIGHT * 0.5;
            let marker = Rect::new(x0 + DOT * 0.5 - 7.0, mid - 7.0, 14.0, 14.0);
            let (fill, stroke) = if complete { (pal[ACCENT], pal[ACCENT]) } else { (pal[RAISED], pal[ACCENT].mix(pal[BORDER], 0.4)) };
            self.draw.rect_stroked(marker, 4.0, fill.fade(ink), stroke.fade(ink), 2.0);
            text.clear();
            let _ = write!(text, "MÓDULO {}", m + 1);
            self.draw.add_text(Vec2::new(text_x, hy), font * 0.72, pal[ACCENT].mix(pal[MUTED], 0.3).fade(ink), &text);
            self.draw.add_text(Vec2::new(text_x, hy + font * 0.72 * LINE_HEIGHT), font * 0.98, pal[TEXT].fade(ink), name);
            text.clear();
            let _ = write!(text, "{seen}/{}", list.len());
            let tw = self.text.width(&text, font * 0.8);
            self.draw.add_text(
                Vec2::new(full.right() - theme.pad - tw, hy + font * 0.72 * LINE_HEIGHT + 2.0),
                font * 0.8,
                pal[MUTED].fade(ink),
                &text,
            );
            first += list.len();
        }

        // Lessons.
        let (mut changed, mut chosen) = (false, false);
        let snappy = self.steps[Snappy as usize];
        for (i, &(_, label, ly)) in lessons.iter().enumerate() {
            let rid = row_id(i);
            let row = Rect::new(full.x + 8.0, at(ly) + 1.0, full.w - 16.0, ROW - 2.0);
            let interactive = open && pc > 0.6 && row.y >= full.y && row.bottom() <= full.bottom();
            let hit = if interactive { Some(self.hit(rid, row)) } else { None };
            self.describe(rid, crate::a11y::Role::Link, label, row, if visited[i] { Some("vista") } else { None }, Some(*selected == i));
            if hit.as_ref().is_some_and(|h| h.clicked) {
                chosen = true;
                changed = *selected != i;
                *selected = i;
                self.learn(rid, label);
            }
            let hovered = hit.as_ref().is_some_and(|h| h.hovered);
            let n = self.nodes.entry(rid).or_insert_with(Node::new);
            let fresh = n.touch(self.frame);
            n.rect.snap(row);
            let anim = n.aux.drive((*selected == i) as u8 as f32, fresh, &snappy) | n.hover.drive(hovered as u8 as f32, fresh, &snappy);
            let (cur, hov) = (n.aux.get().clamp(0.0, 1.0), n.hover.get().clamp(0.0, 1.0));
            self.animating |= anim;

            // Row highlight, current lesson first.
            let bg = pal[TEXT].with_alpha(0.06 * hov).mix(pal[ACCENT].with_alpha(0.13), cur);
            self.draw.rect(row, 10.0, bg.fade(ink));

            // The circle: checked if visited, numbered otherwise; the
            // current one carries a halo.
            let c = circle(i);
            if cur > 0.01 {
                self.draw.rect(c.inset(-4.0 * pc), c.w, pal[ACCENT].with_alpha(0.25 * cur));
            }
            let (fill, stroke) = if visited[i] || cur > 0.5 {
                (pal[ACCENT], pal[ACCENT])
            } else {
                (pal[RAISED].mix(pal[MUTED], 1.0 - pc), pal[BORDER].mix(pal[MUTED], 0.5))
            };
            self.draw.rect_stroked(c, c.w, fill, stroke, 1.5 * pc);
            let mark = ink * (c.w / DOT).min(1.0);
            if visited[i] && cur < 0.5 {
                draw_icon(&mut self.draw, Icon::Check, c.center(), DOT * 0.5, pal[ON_ACCENT].fade(mark));
            } else {
                text.clear();
                let _ = write!(text, "{}", i + 1);
                let size = font * 0.78;
                let tw = self.text.width(&text, size);
                let color = if cur > 0.5 { pal[ON_ACCENT] } else { pal[MUTED] };
                self.draw.add_text(Vec2::new(c.center().x - tw * 0.5, c.center().y - size * LINE_HEIGHT * 0.5), size, color.fade(mark), &text);
            }

            // The lesson's name.
            let ink_c = if cur > 0.5 {
                pal[ACCENT].mix(pal[TEXT], 0.25)
            } else if visited[i] {
                pal[TEXT]
            } else {
                pal[TEXT].mix(pal[MUTED], 0.55)
            };
            let tx = x0 + DOT + 12.0;
            self.draw.add_text(Vec2::new(tx, row.y + (row.h - font * LINE_HEIGHT) * 0.5), font, ink_c.fade(ink), label);
        }

        // A thin scroll indicator when the syllabus does not fit.
        if max_scroll > 0.0 && pc > 0.01 {
            let thumb = (full.h * full.h / content_h).max(24.0);
            let ty = full.y + (full.h - thumb) * (scroll / max_scroll);
            self.draw.rect(Rect::new(full.right() - 5.0, ty, 3.0, thumb), 2.0, pal[MUTED].with_alpha(0.35 * ink));
        }
        self.draw.push(DrawCmd::PopClip);
        if open {
            self.blockers.push((1, panel));
        }
        if chosen {
            // A choice was made: fold away even though the pointer is there.
            let n = self.nodes.get_mut(&id).unwrap();
            n.latch = true;
            n.open = false;
            self.animating = true;
        }
        self.draw.set_layer(below);
        self.draw.cull = cull;
        self.alpha = alpha;
        self.clip = clip;
        changed
    }
}
