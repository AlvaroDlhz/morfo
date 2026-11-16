//! Mutation: any component, or group of components, turning into another.
//!
//! [`Ui::morph`] wraps content whose *form* can change. Every frame it keeps
//! a snapshot of what that content drew. When the form changes, the old
//! snapshot and the new content are drawn together, both stretched onto one
//! spring-driven rectangle that travels from the old bounds to the new ones,
//! while the old one fades out and the new one fades in. When either form
//! has a background shape (a button's pill, a card, a field's box), that
//! shape is not cross-faded but morphed: position, size, radius and colors.

use std::hash::{Hash, Hasher};

use crate::anim::smoothstep;
use crate::anim::SpringKind::{Gentle, Smooth};
use crate::draw::{DrawCmd, DrawList};
use crate::geom::{Color, Rect, Vec2};
use crate::ui::{Dir, Node, Ui};

#[derive(Default)]
pub(crate) struct Morph {
    form: u64,
    /// What the current form drew last, at its natural place.
    now: Snapshot,
    /// What the form being left drew, frozen when the change happened.
    was: Snapshot,
}

#[derive(Default)]
pub(crate) struct Snapshot {
    cmds: Vec<DrawCmd>,
    text: String,
    points: Vec<Vec2>,
    pub(crate) bounds: Rect,
    /// Index in `cmds` of the background shape, if there is one.
    shell: Option<usize>,
}

impl Snapshot {
    /// Copies `cmds` (and the strings they use) so they can be replayed in
    /// later frames. Reuses its buffers: no allocation once warmed up.
    pub(crate) fn capture(&mut self, cmds: &[DrawCmd], list: &DrawList, bounds: Rect) {
        self.cmds.clear();
        self.text.clear();
        self.points.clear();
        self.bounds = bounds;
        for &cmd in cmds {
            let mut cmd = cmd;
            match &mut cmd {
                DrawCmd::Text { start, len, .. } => {
                    let s = list.text(*start, *len);
                    *start = self.text.len() as u32;
                    self.text.push_str(s);
                }
                DrawCmd::Polygon { start, len, .. } => {
                    let pts = list.points(*start, *len);
                    *start = self.points.len() as u32;
                    self.points.extend_from_slice(pts);
                }
                _ => {}
            }
            self.cmds.push(cmd);
        }
        self.shell = find_shell(&self.cmds, bounds);
    }
}

impl Ui {
    /// Content that can mutate into a different component.
    ///
    /// `form` is any value with `Hash` (a `bool`, a number, an enum) telling
    /// which form the content has now. While it stays the same, `f` is laid
    /// out and drawn as usual. When it changes, the previous component
    /// transforms into the new one: the shape travels and resizes, and the
    /// contents cross-fade, while the elements around it make room
    /// smoothly. Keys declared inside `f` are namespaced under `key`.
    ///
    /// ```
    /// # let mut ui = morfo::Ui::new(morfo::Theme::dark());
    /// # let (mut open, mut query) = (false, String::new());
    /// # ui.begin(0.0, morfo::Vec2::new(800.0, 600.0));
    /// ui.morph("search", open, |ui| {
    ///     if !open {
    ///         if ui.button("open", "Search").clicked {
    ///             open = true;
    ///             ui.request_focus("field");
    ///         }
    ///     } else {
    ///         ui.text_field("field", &mut query, "Type…", 320.0);
    ///     }
    /// });
    /// # ui.end();
    /// ```
    pub fn morph(&mut self, key: &str, form: impl Hash, f: impl FnOnce(&mut Ui)) {
        let id = self.id(key);
        let form = {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            form.hash(&mut h);
            h.finish()
        };
        let mut st = self.morphs.remove(&id).unwrap_or_default();
        let n = self.nodes.entry(id).or_insert_with(Node::new);
        let absent = n.touch(self.frame);
        let changed = !absent && st.form != form;
        st.form = form;
        if changed {
            // The current snapshot becomes the outgoing one.
            std::mem::swap(&mut st.was, &mut st.now);
            n.aux.snap(0.0);
            n.aux.set(1.0);
        }
        let in_motion = changed || !n.aux.is_resting() || !n.rect.is_resting();

        let mark = self.draw.mark();
        let saved = self.scope;
        self.scope = id;
        let gap = self.theme.gap;
        let (origin, size) = self.group(id, Dir::Column, Vec2::ZERO, gap, None, f);
        self.scope = saved;
        let target = Rect::from_min_size(origin, size);

        let (smooth, gentle) = (self.steps[Smooth as usize], self.steps[Gentle as usize]);
        let n = self.nodes.get_mut(&id).unwrap();
        let moving = if absent || !in_motion {
            n.rect.snap(target);
            n.aux.snap(1.0);
            false
        } else {
            n.rect.set(target);
            n.rect.tick(&smooth) | n.aux.tick(&gentle) | changed
        };
        let (r, t) = (n.rect.get(), n.aux.get().clamp(0.0, 1.0));
        self.animating |= moving;
        st.now.capture(self.draw.cmds_from(mark), &self.draw, target);

        if !moving {
            self.advance(size);
            self.morphs.insert(id, st);
            return;
        }
        // Siblings get the final layout at once: their own springs are the
        // same as the shape's, so they travel in step with it.
        self.advance(size);

        // Repaint the content: shell, then the old form, then the new one.
        let mut new = std::mem::take(&mut self.morph_buf);
        new.clear();
        self.draw.drain_from(mark, &mut new);
        let fade_out = 1.0 - smoothstep(t / 0.6);
        let fade_in = smoothstep((t - 0.2) / 0.8);
        let old_shell = st.was.shell.map(|i| map(st.was.cmds[i], st.was.bounds, r, 1.0));
        let new_shell = st.now.shell.map(|i| map(st.now.cmds[i], target, r, 1.0));
        if let Some(shell) = blend_shells(old_shell, new_shell, t) {
            emit(&mut self.draw, shell);
        }
        self.replay(&st.was, r, fade_out, st.was.shell);
        for (i, &cmd) in new.iter().enumerate() {
            if Some(i) == st.now.shell {
                continue;
            }
            match map(cmd, target, r, fade_in) {
                DrawCmd::Polygon { start, len, color } => {
                    let mapped: Vec<Vec2> = self.draw.points(start, len).iter().map(|&p| map_point(p, target, r)).collect();
                    self.draw.polygon(&mapped, color);
                }
                cmd => emit(&mut self.draw, cmd),
            }
        }
        self.morph_buf = new;
        self.morphs.insert(id, st);
    }
}

impl Ui {
    /// Draws a snapshot again, stretched from where it was onto `to` and
    /// faded by `fade`, skipping command `skip`.
    pub(crate) fn replay(&mut self, snap: &Snapshot, to: Rect, fade: f32, skip: Option<usize>) {
        for (i, &cmd) in snap.cmds.iter().enumerate() {
            if Some(i) == skip {
                continue;
            }
            match map(cmd, snap.bounds, to, fade) {
                DrawCmd::Text { pos, size, color, start, len, mono } => {
                    if color.a > 0.002 && size > 0.5 {
                        let text = &snap.text[start as usize..(start + len) as usize];
                        self.draw.add_text_styled(pos, size, color, text, mono);
                    }
                }
                DrawCmd::Polygon { start, len, color } => {
                    let pts = &snap.points[start as usize..(start + len) as usize];
                    let mapped: Vec<Vec2> = pts.iter().map(|&p| map_point(p, snap.bounds, to)).collect();
                    self.draw.polygon(&mapped, color);
                }
                cmd => emit(&mut self.draw, cmd),
            }
        }
    }
}

/// The largest visible rectangle covering most of `bounds`: the background
/// of a button, card or field.
fn find_shell(cmds: &[DrawCmd], bounds: Rect) -> Option<usize> {
    let area = bounds.w * bounds.h;
    let outer = bounds.inset(-6.0);
    let mut best = None;
    let mut best_area = area * 0.6;
    for (i, cmd) in cmds.iter().enumerate() {
        if let DrawCmd::Rect { rect, fill, stroke, stroke_width, .. } = *cmd {
            let visible = fill.a > 0.01 || (stroke_width > 0.0 && stroke.a > 0.01);
            let inside = rect.x >= outer.x && rect.y >= outer.y && rect.right() <= outer.right() && rect.bottom() <= outer.bottom();
            let a = rect.w * rect.h;
            if visible && inside && a > best_area {
                best = Some(i);
                best_area = a;
            }
        }
    }
    best
}

/// Places point `v` of rectangle `from` at the same relative spot in `to`.
fn map_point(v: Vec2, from: Rect, to: Rect) -> Vec2 {
    let (sx, sy) = (to.w / from.w.max(1.0), to.h / from.h.max(1.0));
    Vec2::new(to.x + (v.x - from.x) * sx, to.y + (v.y - from.y) * sy)
}

/// Places `cmd`, drawn inside `from`, proportionally inside `to`, with its
/// opacity multiplied by `fade`.
fn map(cmd: DrawCmd, from: Rect, to: Rect, fade: f32) -> DrawCmd {
    let sx = to.w / from.w.max(1.0);
    let sy = to.h / from.h.max(1.0);
    let s = sx.min(sy);
    let p = |v: Vec2| Vec2::new(to.x + (v.x - from.x) * sx, to.y + (v.y - from.y) * sy);
    let r = |q: Rect| Rect::from_min_size(p(q.min()), Vec2::new(q.w * sx, q.h * sy));
    match cmd {
        DrawCmd::Rect { rect, radius, fill, stroke, stroke_width } => {
            DrawCmd::Rect { rect: r(rect), radius: radius * s, fill: fill.fade(fade), stroke: stroke.fade(fade), stroke_width }
        }
        DrawCmd::Shadow { rect, radius, blur, offset, color } => DrawCmd::Shadow {
            rect: r(rect),
            radius: radius * s,
            blur: blur * s,
            offset: Vec2::new(offset.x * sx, offset.y * sy),
            color: color.fade(fade),
        },
        // Text is never stretched, and only scaled within limits: a word
        // blown up or shrunk to a speck reads as noise, not as motion.
        DrawCmd::Text { pos, size, color, start, len, mono } => {
            DrawCmd::Text { pos: p(pos), size: size * s.clamp(0.6, 1.4), color: color.fade(fade), start, len, mono }
        }
        DrawCmd::Line { from: a, to: b, width, color } => DrawCmd::Line { from: p(a), to: p(b), width: width * s, color: color.fade(fade) },
        DrawCmd::PushClip { rect, radius } => DrawCmd::PushClip { rect: r(rect), radius: radius * s },
        // Points are remapped by the caller, which owns them.
        DrawCmd::Polygon { start, len, color } => DrawCmd::Polygon { start, len, color: color.fade(fade) },
        other => other,
    }
}

/// The background shape part way between the old one and the new one. A
/// form without a background behaves as an invisible copy of the other's.
fn blend_shells(a: Option<DrawCmd>, b: Option<DrawCmd>, t: f32) -> Option<DrawCmd> {
    let clear = |c: DrawCmd| match c {
        DrawCmd::Rect { rect, radius, fill, stroke, stroke_width } => {
            DrawCmd::Rect { rect, radius, fill: fill.with_alpha(0.0), stroke: stroke.with_alpha(0.0), stroke_width }
        }
        c => c,
    };
    let (a, b) = match (a, b) {
        (Some(a), Some(b)) => (a, b),
        (Some(a), None) => (a, clear(a)),
        (None, Some(b)) => (clear(b), b),
        (None, None) => return None,
    };
    let (
        DrawCmd::Rect { rect: r0, radius: q0, fill: f0, stroke: s0, stroke_width: w0 },
        DrawCmd::Rect { rect: r1, radius: q1, fill: f1, stroke: s1, stroke_width: w1 },
    ) = (a, b)
    else {
        return None;
    };
    // An invisible color takes the other's hue, so fading in or out never
    // passes through black.
    let toward = |x: Color, y: Color| if x.a < 0.01 { y.with_alpha(0.0) } else { x };
    let l = |x: f32, y: f32| x + (y - x) * t;
    Some(DrawCmd::Rect {
        rect: Rect::new(l(r0.x, r1.x), l(r0.y, r1.y), l(r0.w, r1.w), l(r0.h, r1.h)),
        radius: l(q0, q1),
        fill: toward(f0, f1).mix(toward(f1, f0), t),
        stroke: toward(s0, s1).mix(toward(s1, s0), t),
        stroke_width: l(w0, w1),
    })
}

fn emit(list: &mut DrawList, cmd: DrawCmd) {
    let visible = match cmd {
        DrawCmd::Rect { fill, stroke, stroke_width, .. } => fill.a > 0.002 || (stroke_width > 0.0 && stroke.a > 0.002),
        DrawCmd::Shadow { color, .. } => color.a > 0.004,
        DrawCmd::Text { color, size, .. } => color.a > 0.002 && size > 0.5,
        DrawCmd::Line { color, .. } => color.a > 0.002,
        DrawCmd::Polygon { color, .. } => color.a > 0.002,
        _ => true,
    };
    if visible {
        list.push(cmd);
    }
}
