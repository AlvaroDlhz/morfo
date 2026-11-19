//! Components inside components: any content as one pressable thing,
//! whole parts of the interface switched off at once, and a style that the
//! components inside inherit.

use crate::a11y::Role;
use crate::anim::SpringKind::{Smooth, Snappy};
use crate::draw::DrawCmd;
use crate::geom::{Color, Rect, Vec2};
use crate::theme::{ACCENT, BORDER, ON_ACCENT, TEXT};
use crate::ui::{Dir, Node, Response, Ui};
use crate::widgets::{Size, Variant};

impl Ui {
    /// Any content as one thing that can be pressed: a whole card, a photo
    /// with its caption, a row of a list. It lifts under the pointer, sinks
    /// a little when pressed, takes the focus with Tab and is activated with
    /// Enter or Space, like a button. Controls inside it keep working and
    /// take the press first: a click on them is not a click on it. `label`
    /// is its name for screen readers and in [`habits`](Self::habits).
    /// Keys inside `f` are namespaced under `key`.
    pub fn pressable(&mut self, key: &str, label: &str, f: impl FnOnce(&mut Ui)) -> Response {
        let id = self.declare(key);
        let plate = self.draw.reserve();
        let mark = self.draw.mark();
        let gap = self.theme.gap;
        let (origin, size) = self.group(id, Dir::Column, Vec2::ZERO, gap, None, |ui| ui.within(id, f));
        self.advance(size);
        let bounds = Rect::from_min_size(origin, size);
        // After the content: what is inside claimed the press already.
        let (_, life) = self.enter(id, bounds, Smooth);
        let hit = self.hit(id, bounds);
        self.describe(id, Role::Button, label, bounds, None, None);
        if hit.clicked {
            self.learn(id, label);
        }
        let (hover, press, _) = self.drive3(id, hit.hovered, hit.held, 0.0, Snappy);
        let press = press.clamp(0.0, 1.0);
        let a = self.alpha * life;
        let radius = self.theme.radius;
        let area = bounds.inset(-6.0);
        self.draw.set(
            plate,
            DrawCmd::Rect {
                rect: area,
                radius,
                fill: self.pal[TEXT].with_alpha((0.05 * hover + 0.04 * press) * a),
                stroke: self.pal[BORDER].mix(self.pal[ACCENT], 0.5).with_alpha(0.6 * hover * a),
                stroke_width: 1.0,
            },
        );
        // Lifted by the pointer, pressed down by the click.
        if hover > 0.001 || press > 0.001 {
            let to = bounds.scaled(1.0 - 0.015 * press).translate(Vec2::new(0.0, -2.0 * hover * (1.0 - press)));
            self.transform_from(mark, bounds, to, 1.0);
        }
        Response { clicked: hit.clicked, hovered: hit.hovered, held: hit.held, long_pressed: hit.long_pressed, rect: bounds }
    }

    /// Switches off everything `f` builds when `on` is `false`: it fades to
    /// a dimmer version of itself and stops responding to the pointer, the
    /// keyboard (Tab skips it, and it lets go of the focus) and screen
    /// reader actions, which see it as disabled. Turning it back on fades it
    /// in again. Whatever components `f` holds, nested to any depth.
    pub fn enabled(&mut self, on: bool, f: impl FnOnce(&mut Ui)) {
        let id = self.auto_id().child("morfo-activo");
        let (k, frame) = (self.steps[Smooth as usize], self.frame);
        let n = self.nodes.entry(id).or_insert_with(Node::new);
        if n.touch(frame) {
            n.aux.snap(on as u8 as f32);
        }
        n.aux.set(on as u8 as f32);
        self.animating |= n.aux.tick(&k);
        let level = n.aux.get().clamp(0.0, 1.0);
        let (alpha, disabled) = (self.alpha, self.disabled);
        self.alpha *= 0.4 + 0.6 * level;
        self.disabled |= !on;
        f(self);
        self.alpha = alpha;
        self.disabled = disabled;
    }

    /// Builds `f` with `color` as the accent: every component inside that
    /// uses the accent (solid buttons, switches, sliders, selections, focus
    /// rings…) takes it, and the text drawn on it stays readable.
    pub fn with_accent(&mut self, color: Color, f: impl FnOnce(&mut Ui)) {
        let saved = (self.pal[ACCENT], self.pal[ON_ACCENT]);
        self.pal[ACCENT] = color;
        self.pal[ON_ACCENT] = crate::theme::on_color(color);
        f(self);
        (self.pal[ACCENT], self.pal[ON_ACCENT]) = saved;
    }

    /// Builds `f` at size `size`: every component inside it (controls,
    /// fields, text, spacing, corners) is laid out smaller (`Sm`) or larger
    /// (`Lg`) than the theme's (`Md`). The sizes components are given inside
    /// are relative to it: a `Size::Sm` button in a `Size::Lg` block is a
    /// little smaller than the block's own `Md` buttons.
    pub fn with_size(&mut self, size: Size, f: impl FnOnce(&mut Ui)) {
        let factor = match size {
            Size::Sm => 0.86,
            Size::Md => 1.0,
            Size::Lg => 1.15,
        };
        let saved = self.theme;
        self.theme = self.theme.scaled(factor);
        f(self);
        self.theme = saved;
    }

    /// Builds `f` with `variant` as the variant of the buttons that do not
    /// name one ([`button`](Self::button)): a toolbar of quiet `Ghost`
    /// buttons without repeating it on each.
    pub fn with_variant(&mut self, variant: Variant, f: impl FnOnce(&mut Ui)) {
        let saved = self.style_variant.replace(variant);
        f(self);
        self.style_variant = saved;
    }

    /// Content that can be picked up and dropped on a
    /// [`drop_target`](Self::drop_target), carrying `payload` (an index,
    /// an id of yours). The mouse drags it once it moves a few pixels with
    /// the button held; a finger, after a long press, so that swiping
    /// still scrolls. While dragged it floats under the pointer, and a
    /// faint outline keeps its place.
    ///
    /// Dragging is no way in for everyone (WCAG 2.5.7): offer the same
    /// move with a click or the keyboard too, a "Move to…" menu say.
    pub fn draggable(&mut self, key: &str, payload: u64, f: impl FnOnce(&mut Ui)) -> Response {
        let id = self.declare(key);
        let mark = self.draw.mark();
        let gap = self.theme.gap;
        let (origin, size) = self.group(id, Dir::Column, Vec2::ZERO, gap, None, |ui| ui.within(id, f));
        self.advance(size);
        let bounds = Rect::from_min_size(origin, size);
        self.enter(id, bounds, Smooth);
        let hit = self.hit_inert(id, bounds);
        // Pressed anywhere on it, on a control inside included: the drag
        // takes the press from that control, which then does not click.
        let pressed_here = self.input.down && self.dragging.is_none() && self.input.press_at.is_some_and(|a| bounds.contains(a));
        let moved = self.input.press_at.zip(self.input.pointer).is_some_and(|(a, p)| (p - a).length() >= crate::ui::DRAG_START);
        let held = self.now - self.input.press_time >= crate::ui::LONG_PRESS;
        if pressed_here && self.input.touch && !moved && !held {
            self.wake_at(self.input.press_time + crate::ui::LONG_PRESS);
        }
        if pressed_here && self.drag_scroll.is_none() && if self.input.touch { held && !moved } else { moved } {
            let grab = self.input.press_at.unwrap_or(bounds.center()) - bounds.min();
            self.dragging = Some((id, payload, grab));
            self.active = Some(id);
        }
        if let (Some((src, _, grab)), Some(p)) = (self.dragging, self.input.pointer) {
            if src == id {
                // Lifted out of the page: drawn above everything, under the
                // pointer, with an outline left behind.
                let to = Rect::from_min_size(p - grab, bounds.size()).scaled(1.03);
                self.transform_from(mark, bounds, to, 0.92);
                let mut lifted = Vec::new();
                self.draw.drain_from(mark, &mut lifted);
                let outline = self.pal[BORDER].mix(self.pal[ACCENT], 0.5).with_alpha(0.8 * self.alpha);
                self.draw.rect_stroked(bounds, self.theme.radius, Color::TRANSPARENT, outline, 1.5);
                let layer = self.draw.set_layer(1);
                for c in lifted {
                    self.draw.push(c);
                }
                self.draw.set_layer(layer);
                self.animating = true;
            }
        }
        // Let go after a drag: that was no click.
        let clicked = hit.clicked && self.dragging.is_none_or(|(src, _, _)| src != id);
        Response { clicked, hovered: hit.hovered, held: hit.held, long_pressed: hit.long_pressed, rect: bounds }
    }

    /// The payload of what is being dragged, if anything is.
    pub fn dragging(&self) -> Option<u64> {
        self.dragging.map(|(_, p, _)| p)
    }

    /// A place things picked up with [`draggable`](Self::draggable) can be
    /// dropped on. `f` builds its content and is told whether something is
    /// held over it (to show it would land there); it is outlined in the
    /// accent color then. Returns the payload dropped on it, the frame it
    /// is dropped.
    pub fn drop_target(&mut self, key: &str, f: impl FnOnce(&mut Ui, bool)) -> Option<u64> {
        let id = self.declare(key);
        let plate = self.draw.reserve();
        // Whether it is held over, by where the target was last frame.
        let over = self.dragging.is_some() && self.layout_rect(id).zip(self.input.pointer).is_some_and(|(r, p)| r.contains(p));
        let gap = self.theme.gap;
        let (origin, size) = self.group(id, Dir::Column, Vec2::ZERO, gap, None, |ui| ui.within(id, |ui| f(ui, over)));
        self.advance(size);
        let bounds = Rect::from_min_size(origin, size);
        let (_, life) = self.enter(id, bounds, Smooth);
        let (glow, _, _) = self.drive3(id, over, false, 0.0, Snappy);
        let a = self.alpha * life * glow.clamp(0.0, 1.0);
        if a > 0.002 {
            let accent = self.pal[ACCENT];
            self.draw.set(
                plate,
                DrawCmd::Rect {
                    rect: bounds.inset(-4.0),
                    radius: self.theme.radius,
                    fill: accent.with_alpha(0.08 * a),
                    stroke: accent.with_alpha(a),
                    stroke_width: 2.0,
                },
            );
        }
        let dropped = self.input.released && self.layout_rect(id).zip(self.input.pointer).is_some_and(|(r, p)| r.contains(p));
        dropped.then(|| self.dragging.map(|(_, p, _)| p)).flatten()
    }
}
