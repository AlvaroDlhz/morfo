//! The components. Each one only computes *targets* (where it should be, what
//! it should look like) and lets its node's springs do the rest, so every
//! change of state, variant, size, order or theme is a morph.

use crate::adapt::Prominence;
use crate::anim::SpringKind::{self, Bouncy, Gentle, Smooth, Snappy};
use crate::draw::DrawCmd;
use crate::geom::{Color, Rect, Vec2};
use crate::id::Id;
use crate::theme::{Palette, ACCENT, BORDER, MUTED, ON_ACCENT, RAISED, SHADOW, SURFACE, TEXT};
use crate::ui::{Dir, Node, Response, Ui, LINE_HEIGHT};

/// Visual treatment of a control. Changing it at runtime morphs the control.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Variant {
    /// Filled with the accent color: the primary action.
    #[default]
    Solid,
    /// Tinted background.
    Soft,
    /// Transparent with a border.
    Outline,
    /// Text only until hovered.
    Ghost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Size {
    Sm,
    #[default]
    Md,
    Lg,
}

/// (fill, hovered fill, stroke, ink)
fn variant_colors(pal: &Palette, v: Variant) -> (Color, Color, Color, Color) {
    let (accent, text) = (pal[ACCENT], pal[TEXT]);
    match v {
        Variant::Solid => (accent, accent.mix(Color::WHITE, 0.14), accent.with_alpha(0.0), pal[ON_ACCENT]),
        Variant::Soft => (accent.with_alpha(0.16), accent.with_alpha(0.28), accent.with_alpha(0.0), accent.mix(text, 0.45)),
        Variant::Outline => (text.with_alpha(0.0), text.with_alpha(0.07), pal[BORDER], text),
        Variant::Ghost => (text.with_alpha(0.0), text.with_alpha(0.07), pal[BORDER].with_alpha(0.0), pal[MUTED]),
    }
}

impl Ui {
    /// Declares node `id` at `target`; returns its on-screen rect and
    /// entrance progress.
    pub(crate) fn enter(&mut self, id: Id, target: Rect, kind: SpringKind) -> (Rect, f32) {
        let (k, life) = (self.steps[kind as usize], self.steps[Gentle as usize]);
        let n = self.nodes.entry(id).or_insert_with(Node::new);
        self.animating |= n.begin(self.frame, target, &k, &life);
        (n.rect.get(), n.life.get())
    }

    /// Drives the hover, press and `aux` springs of an entered node and
    /// returns their current values.
    pub(crate) fn drive3(&mut self, id: Id, hover: bool, press: bool, aux: f32, aux_kind: SpringKind) -> (f32, f32, f32) {
        let (snappy, k) = (self.steps[Snappy as usize], self.steps[aux_kind as usize]);
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        let anim = n.hover.drive(hover as u8 as f32, fresh, &snappy)
            | n.press.drive(press as u8 as f32, fresh, &snappy)
            | n.aux.drive(aux, fresh, &k);
        let out = (n.hover.get().clamp(0.0, 1.0), n.press.get(), n.aux.get());
        self.animating |= anim;
        out
    }

    // ----- text ------------------------------------------------------------

    pub fn label(&mut self, text: &str) {
        self.text_node(text, self.theme.font, TEXT);
    }

    pub fn muted(&mut self, text: &str) {
        self.text_node(text, self.theme.font * 0.9, MUTED);
    }

    pub fn heading(&mut self, text: &str) {
        self.text_node(text, self.theme.font * 1.7, TEXT);
    }

    pub fn title(&mut self, text: &str) {
        self.text_node(text, self.theme.font * 1.15, TEXT);
    }

    /// Text at any font size, for displays such as clocks or big numbers.
    pub fn text(&mut self, text: &str, size: f32) {
        self.text_node(text, size, TEXT);
    }

    fn text_node(&mut self, text: &str, size: f32, color: usize) {
        let id = self.auto_id();
        let w = self.text.width(text, size);
        let target = self.alloc(Vec2::new(w, size * LINE_HEIGHT));
        let (r, life) = self.enter(id, target, Smooth);
        self.draw.add_text(r.min(), size, self.pal[color].fade(self.alpha * life), text);
    }

    // ----- surfaces --------------------------------------------------------

    /// A bare morphing shape: declare the same `key` with another size, radius
    /// or color (or in another place) and it flows into the new form.
    pub fn surface(&mut self, key: &str, size: Vec2, radius: f32, fill: Color) -> Rect {
        let id = self.id(key);
        let target = self.alloc(size);
        let (r, life) = self.enter(id, target, Smooth);
        let k = self.steps[Smooth as usize];
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        let anim = n.fill.drive(fill, fresh, &k) | n.radius.drive(radius, fresh, &k);
        let (fill, radius) = (n.fill.get(), n.radius.get());
        self.animating |= anim;
        let r = r.scaled(0.9 + 0.1 * life);
        self.draw.rect(r, radius, fill.fade(self.alpha * life));
        r
    }

    /// A raised container as wide as the space available.
    pub fn card(&mut self, key: &str, f: impl FnOnce(&mut Ui)) {
        let w = self.available_width();
        self.card_w(key, w, f);
    }

    /// A raised container of fixed width. Its height follows its content,
    /// fluidly.
    pub fn card_w(&mut self, key: &str, width: f32, f: impl FnOnce(&mut Ui)) {
        let id = self.id(key);
        let pad = self.theme.pad;
        let shadow_slot = self.draw.reserve();
        let bg_slot = self.draw.reserve();
        let gap = self.theme.gap;
        let (origin, mut size) = self.group(id, Dir::Column, Vec2::splat(pad), gap, None, f);
        size.x = width;
        self.sizes.insert(id, size);
        self.advance(size);
        let (r, life) = self.enter(id, Rect::from_min_size(origin, size), Smooth);
        let r = r.scaled(0.97 + 0.03 * life);
        let a = self.alpha * life;
        let radius = self.theme.radius;
        self.draw.set(
            shadow_slot,
            DrawCmd::Shadow { rect: r, radius, blur: 28.0, offset: Vec2::new(0.0, 10.0), color: self.pal[SHADOW].fade(a) },
        );
        self.draw.set(
            bg_slot,
            DrawCmd::Rect { rect: r, radius, fill: self.pal[SURFACE].fade(a), stroke: self.pal[BORDER].fade(a), stroke_width: 1.0 },
        );
    }

    /// A block of source code, one string per line, in the monospaced font.
    /// Lines starting with `//` are dimmed as comments.
    pub fn code(&mut self, lines: &[&str]) {
        let id = self.auto_id();
        let size = self.theme.font * 0.86;
        let (line_h, pad) = (size * 1.55, self.theme.pad * 0.8);
        let target = self.alloc(Vec2::new(self.available_width(), lines.len() as f32 * line_h + pad * 2.0));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        self.draw.rect_stroked(r, self.theme.radius * 0.7, self.pal[RAISED].fade(a * 0.6), self.pal[BORDER].fade(a), 1.0);
        let ink = self.pal[TEXT].mix(self.pal[MUTED], 0.25).fade(a);
        let comment = self.pal[MUTED].fade(a);
        for (i, line) in lines.iter().enumerate() {
            let pos = Vec2::new(r.x + pad, r.y + pad + i as f32 * line_h + (line_h - size * LINE_HEIGHT) * 0.5);
            let color = if line.trim_start().starts_with("//") { comment } else { ink };
            self.draw.add_code(pos, size, color, line);
        }
    }

    /// Thin horizontal rule.
    pub fn separator(&mut self) {
        let id = self.auto_id();
        let target = self.alloc(Vec2::new(self.available_width(), 1.0));
        let (r, life) = self.enter(id, target, Smooth);
        self.draw.rect(r, 0.0, self.pal[BORDER].fade(self.alpha * life));
    }

    // ----- controls --------------------------------------------------------

    pub fn button(&mut self, key: &str, label: &str) -> Response {
        self.button_with(key, label, Variant::Solid, Size::Md)
    }

    pub fn button_with(&mut self, key: &str, label: &str, variant: Variant, size: Size) -> Response {
        let id = self.id(key);
        self.button_id(id, label, variant, size)
    }

    pub(crate) fn button_id(&mut self, id: Id, label: &str, variant: Variant, size: Size) -> Response {
        let t = self.theme;
        let (font, h, pad) = match size {
            Size::Sm => (t.font * 0.87, t.control * 0.78, t.pad * 0.7),
            Size::Md => (t.font, t.control, t.pad),
            Size::Lg => (t.font * 1.18, t.control * 1.25, t.pad * 1.45),
        };
        let text_w = self.text.width(label, font);
        let target = self.alloc(Vec2::new(text_w + pad * 2.0, h));
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        let (fill, fill_hover, stroke, ink) = variant_colors(&self.pal, variant);

        let (snappy, smooth) = (self.steps[Snappy as usize], self.steps[Smooth as usize]);
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        let anim = n.hover.drive(hit.hovered as u8 as f32, fresh, &snappy)
            | n.press.drive(hit.held as u8 as f32, fresh, &snappy)
            | n.fill.drive(fill, fresh, &smooth)
            | n.stroke.drive(stroke, fresh, &smooth)
            | n.ink.drive(ink, fresh, &smooth)
            | n.aux.drive(font, fresh, &smooth);
        let (hover, press) = (n.hover.get().clamp(0.0, 1.0), n.press.get());
        let (fill, stroke, ink, font_now) = (n.fill.get(), n.stroke.get(), n.ink.get(), n.aux.get());
        self.animating |= anim;

        let a = self.alpha * life;
        let r = r.scaled((0.92 + 0.08 * life) * (1.0 - 0.045 * press));
        let ink = if variant == Variant::Ghost { ink.mix(self.pal[TEXT], hover) } else { ink };
        self.draw.rect_stroked(r, r.h * 0.5, fill.mix(fill_hover, hover).fade(a), stroke.fade(a), 1.0);
        // The label rides the animated box and font size, so it stays centered mid-morph.
        let w_now = text_w * font_now / font;
        let pos = Vec2::new(r.x + (r.w - w_now) * 0.5, r.y + (r.h - font_now * LINE_HEIGHT) * 0.5);
        self.draw.add_text(pos, font_now, ink.fade(a), label);

        if hit.clicked {
            self.profile.record(id, self.now);
        }
        Response { clicked: hit.clicked, hovered: hit.hovered, held: hit.held, rect: r }
    }

    /// On/off switch. Returns `true` when the user flipped it.
    pub fn toggle(&mut self, key: &str, value: &mut bool) -> bool {
        let id = self.id(key);
        let h = (self.theme.control * 0.65).round();
        let target = self.alloc(Vec2::new((h * 1.75).round(), h));
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        if hit.clicked {
            *value = !*value;
            self.profile.record(id, self.now);
        }
        let (snappy, bouncy) = (self.steps[Snappy as usize], self.steps[Bouncy as usize]);
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        let anim = n.aux.drive(*value as u8 as f32, fresh, &bouncy)
            | n.hover.drive(hit.hovered as u8 as f32, fresh, &snappy)
            | n.press.drive(hit.held as u8 as f32, fresh, &snappy);
        let (t, hover, press) = (n.aux.get(), n.hover.get(), n.press.get());
        self.animating |= anim;

        let a = self.alpha * life;
        let r = r.scaled(0.9 + 0.1 * life);
        let track = self.pal[BORDER].mix(self.pal[ACCENT], t.clamp(0.0, 1.0)).mix(Color::WHITE, 0.08 * hover);
        self.draw.rect(r, r.h * 0.5, track.fade(a));
        // The knob stretches while pressed and overshoots a little on release.
        let inset = 3.0;
        let d = r.h - 2.0 * inset;
        let kw = d + 6.0 * press;
        let knob = Rect::new(r.x + inset + (r.w - 2.0 * inset - kw) * t, r.y + inset, kw, d);
        self.draw.push(DrawCmd::Shadow {
            rect: knob,
            radius: d * 0.5,
            blur: 6.0,
            offset: Vec2::new(0.0, 2.0),
            color: Color::BLACK.with_alpha(0.25 * a),
        });
        self.draw.rect(knob, d * 0.5, Color::WHITE.fade(a));
        hit.clicked
    }

    /// Slider over `0..=1`. Returns `true` while the user is changing it.
    pub fn slider(&mut self, key: &str, value: &mut f32, width: f32) -> bool {
        let id = self.id(key);
        let h = (self.theme.control * 0.6).round();
        let target = self.alloc(Vec2::new(width, h));
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        let thumb = h * 0.7;
        let mut changed = false;
        if hit.held {
            if let Some(p) = self.input.pointer {
                let v = ((p.x - r.x - thumb * 0.5) / (r.w - thumb).max(1.0)).clamp(0.0, 1.0);
                changed = v != *value;
                *value = v;
            }
        }
        if hit.released {
            self.profile.record(id, self.now);
        }
        let snappy = self.steps[Snappy as usize];
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        let anim = n.aux.drive(value.clamp(0.0, 1.0), fresh, &snappy)
            | n.hover.drive((hit.hovered || hit.held) as u8 as f32, fresh, &snappy)
            | n.press.drive(hit.held as u8 as f32, fresh, &snappy);
        let (t, hover, press) = (n.aux.get(), n.hover.get(), n.press.get());
        self.animating |= anim;

        let a = self.alpha * life;
        let cx = r.x + thumb * 0.5 + (r.w - thumb) * t;
        let cy = r.y + r.h * 0.5;
        let bar = 6.0 + 2.0 * hover;
        self.draw.rect(Rect::new(r.x, cy - bar * 0.5, r.w, bar), bar * 0.5, self.pal[BORDER].fade(a));
        self.draw.rect(Rect::new(r.x, cy - bar * 0.5, cx - r.x, bar), bar * 0.5, self.pal[ACCENT].fade(a));
        let d = thumb * (0.8 + 0.2 * hover - 0.1 * press) * life;
        let knob = Rect::new(cx - d * 0.5, cy - d * 0.5, d, d);
        self.draw.push(DrawCmd::Shadow {
            rect: knob,
            radius: d * 0.5,
            blur: 8.0,
            offset: Vec2::new(0.0, 2.0),
            color: Color::BLACK.with_alpha(0.3 * a),
        });
        self.draw.rect(knob, d * 0.5, Color::WHITE.fade(a));
        changed
    }

    /// Progress bar over `0..=1`; the fill glides to each new value.
    pub fn progress(&mut self, value: f32, width: f32) {
        let id = self.auto_id();
        let target = self.alloc(Vec2::new(width, 8.0));
        let (r, life) = self.enter(id, target, Smooth);
        let k = self.steps[Gentle as usize];
        let n = self.nodes.get_mut(&id).unwrap();
        let fresh = n.fresh();
        let anim = n.aux.drive(value.clamp(0.0, 1.0), fresh, &k);
        let t = n.aux.get();
        self.animating |= anim;
        let a = self.alpha * life;
        self.draw.rect(r, r.h * 0.5, self.pal[BORDER].fade(a));
        self.draw.rect(Rect::new(r.x, r.y, (r.w * t).max(r.h), r.h), r.h * 0.5, self.pal[ACCENT].fade(a));
    }

    /// Segmented control; the selection indicator slides and stretches
    /// between options. Returns `true` when the selection changed.
    pub fn segmented(&mut self, key: &str, labels: &[&str], selected: &mut usize) -> bool {
        let id = self.id(key);
        let font = self.theme.font * 0.93;
        let (h, inset, seg_pad) = (self.theme.control, 4.0, self.theme.pad * 0.8);
        let total: f32 = labels.iter().map(|l| self.text.width(l, font) + seg_pad * 2.0).sum();
        let target = self.alloc(Vec2::new(total + inset * 2.0, h));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        self.draw.rect(r, r.h * 0.5, self.pal[RAISED].fade(a));
        let indicator_slot = self.draw.reserve();

        let mut changed = false;
        let mut x = inset;
        let mut sel_local = Rect::default();
        let snappy = self.steps[Snappy as usize];
        for (i, label) in labels.iter().enumerate() {
            let text_w = self.text.width(label, font);
            let w = text_w + seg_pad * 2.0;
            let local = Rect::new(x, inset, w, h - inset * 2.0);
            let seg_id = id.with(i as u64);
            let hit = self.hit(seg_id, local.translate(r.min()));
            if hit.clicked && *selected != i {
                *selected = i;
                changed = true;
                self.profile.record(seg_id, self.now);
            }
            let on = *selected == i;
            if on {
                sel_local = local;
            }
            let n = self.nodes.entry(seg_id).or_insert_with(Node::new);
            let fresh = n.fresh();
            let level = if on { 1.0 } else if hit.hovered { 0.5 } else { 0.0 };
            let anim = n.begin(self.frame, local.translate(r.min()), &snappy, &snappy) | n.hover.drive(level, fresh, &snappy);
            let ink = self.pal[MUTED].mix(self.pal[TEXT], n.hover.get());
            self.animating |= anim;
            let pos = Vec2::new(r.x + x + seg_pad, r.y + (r.h - font * LINE_HEIGHT) * 0.5);
            self.draw.add_text(pos, font, ink.fade(a), label);
            x += w;
        }

        // The indicator lives in the control's local space, so it follows the
        // control rigidly while springing between options.
        let ind_id = id.with(u64::MAX);
        let (bouncy, gentle) = (self.steps[Bouncy as usize], self.steps[Gentle as usize]);
        let n = self.nodes.entry(ind_id).or_insert_with(Node::new);
        let anim = n.begin(self.frame, sel_local, &bouncy, &gentle);
        let ind = n.rect.get().translate(r.min());
        self.animating |= anim;
        self.draw.set(
            indicator_slot,
            DrawCmd::Rect { rect: ind, radius: ind.h * 0.5, fill: self.pal[SURFACE].mix(self.pal[TEXT], 0.1).fade(a), stroke: Color::TRANSPARENT, stroke_width: 0.0 },
        );
        changed
    }

    // ----- adaptive --------------------------------------------------------

    /// A row of actions that reshapes itself around how *this* user works:
    /// the most used actions drift to the front and grow, rarely used ones
    /// shrink. `items` are `(key, label)`; returns the index (into `items`)
    /// of the action clicked this frame.
    ///
    /// Two users with different habits end up with different toolbars, and
    /// every change is a spring-driven morph rather than a jump. Nothing moves
    /// while the pointer is over the group, so a target never slides away
    /// from under a click.
    pub fn adaptive_actions(&mut self, key: &str, items: &[(&str, &str)]) -> Option<usize> {
        let group = self.id(key);
        let mut order = self.orders.remove(&group).unwrap_or_default();
        if order.len() != items.len() {
            order.clear();
            order.extend((0..items.len() as u32).map(|i| (i, Prominence::Regular)));
        }
        let now = self.now;
        // Never rearrange under the user's hand: while the pointer is over
        // the group it stays put, and adapts once the pointer moves away.
        let busy = self.rect_of(group).zip(self.input.pointer).is_some_and(|(r, p)| r.inset(-10.0).contains(p));
        if !busy {
            self.profile.arrange(&mut order, now, 0.2, |(i, _)| group.child(items[i as usize].0));
            // Once the group has seen real use, whatever is being ignored recedes.
            let total: f32 = items.iter().map(|(k, _)| self.profile.score(group.child(k), now)).sum();
            for (i, level) in order.iter_mut() {
                let id = group.child(items[*i as usize].0);
                *level = self.profile.prominence(id, now);
                if *level == Prominence::Regular && total >= 6.0 && self.profile.score(id, now) < total * 0.04 {
                    *level = Prominence::Compact;
                }
            }
        }
        let mut clicked = None;
        let row = self.auto_id();
        let gap = self.theme.gap;
        let (origin, size) = self.group(row, Dir::Row, Vec2::ZERO, gap, None, |ui| {
            for &(i, level) in &order {
                let (item_key, label) = items[i as usize];
                let (variant, size) = match level {
                    Prominence::Featured => (Variant::Solid, Size::Lg),
                    Prominence::Regular => (Variant::Soft, Size::Md),
                    Prominence::Compact => (Variant::Ghost, Size::Sm),
                };
                if ui.button_id(group.child(item_key), label, variant, size).clicked {
                    clicked = Some(i as usize);
                }
            }
        });
        self.advance(size);
        // Remember where the group is, for the hover check above.
        self.enter(group, Rect::from_min_size(origin, size), Snappy);
        self.orders.insert(group, order);
        clicked
    }

    /// Like [`menu`](Self::menu), but shaped by how this user works: the
    /// options they use come first, and once there is enough history the
    /// ones they never touch fold away under "Más…". The order is decided
    /// while the menu is closed, so nothing moves while it is open.
    pub fn adaptive_menu(&mut self, key: &str, label: &str, items: &[(&str, &str)]) -> Option<usize> {
        const MORE: &str = "Más…";
        let id = self.id(key);
        let (shown, anchor) = self.menu_trigger(id, label);
        let mut order = self.orders.remove(&id).unwrap_or_default();
        if order.len() != items.len() {
            order.clear();
            order.extend((0..items.len() as u32).map(|i| (i, Prominence::Regular)));
        }
        let now = self.now;
        if !shown {
            self.profile.arrange(&mut order, now, 0.2, |(i, _)| id.child(items[i as usize].0));
            let total: f32 = items.iter().map(|(k, _)| self.profile.score(id.child(k), now)).sum();
            for (n, (i, level)) in order.iter_mut().enumerate() {
                let used = self.profile.score(id.child(items[*i as usize].0), now) >= total * 0.08;
                *level = if total < 6.0 || used || n == 0 { Prominence::Regular } else { Prominence::Compact };
            }
        }
        let folded = order.iter().any(|(_, level)| *level == Prominence::Compact);
        let width = self.menu_width(anchor, items.iter().map(|(_, l)| *l).chain([MORE]));
        let mut picked = None;
        let dismissed = self.floating(id.child("panel"), shown, self.menu_cfg(anchor), |ui| {
            for &(i, level) in &order {
                let (item, label) = items[i as usize];
                if level != Prominence::Compact && ui.menu_row(id.child(item), label, width, TEXT) {
                    picked = Some(i as usize);
                }
            }
            if folded {
                if !ui.menu_more && ui.menu_row(id.child("morfo-more"), MORE, width, MUTED) {
                    ui.menu_more = true;
                }
                let more = ui.menu_more;
                ui.reveal_id(id.child("morfo-rest"), more, |ui| {
                    ui.layouts.last_mut().unwrap().gap = 2.0;
                    for &(i, level) in &order {
                        let (item, label) = items[i as usize];
                        if level == Prominence::Compact && ui.menu_row(id.child(item), label, width, TEXT) {
                            picked = Some(i as usize);
                        }
                    }
                });
            }
        });
        if (picked.is_some() || dismissed) && self.open == Some(id) {
            self.open = None;
        }
        self.orders.insert(id, order);
        picked
    }

    /// Shortcuts that appear by themselves: up to `max` of the actions this
    /// user relies on most in the group `source` (an
    /// [`adaptive_actions`](Self::adaptive_actions) or menu key), with the
    /// same `items`. Shows nothing until a habit exists. Returns the index
    /// of the shortcut clicked.
    pub fn suggestions(&mut self, key: &str, title: &str, source: &str, items: &[(&str, &str)], max: usize) -> Option<usize> {
        let id = self.id(key);
        let src = self.id(source);
        let now = self.now;
        let mut picks = self.orders.remove(&id).unwrap_or_default();
        let busy = self.rect_of(id).zip(self.input.pointer).is_some_and(|(r, p)| r.inset(-10.0).contains(p));
        if !busy {
            picks.clear();
            picks.extend(
                (0..items.len() as u32).filter(|&i| self.profile.score(src.child(items[i as usize].0), now) >= 2.0).map(|i| (i, Prominence::Regular)),
            );
            let score = |i: u32| self.profile.score(src.child(items[i as usize].0), now);
            picks.sort_by(|a, b| score(b.0).total_cmp(&score(a.0)));
            picks.truncate(max);
        }
        let mut clicked = None;
        self.reveal_id(id.child("reveal"), !picks.is_empty(), |ui| {
            let row = ui.auto_id();
            let gap = ui.theme.gap;
            let (origin, size) = ui.group(row, Dir::Row, Vec2::ZERO, gap, None, |ui| {
                ui.muted(title);
                for &(i, _) in &picks {
                    let (item, label) = items[i as usize];
                    if ui.button_id(id.child(item), label, Variant::Soft, Size::Sm).clicked {
                        // A shortcut counts as using the action itself.
                        ui.profile.record(src.child(item), now);
                        clicked = Some(i as usize);
                    }
                }
            });
            ui.advance(size);
            ui.enter(id, Rect::from_min_size(origin, size), Snappy);
        });
        self.orders.insert(id, picks);
        clicked
    }
}
