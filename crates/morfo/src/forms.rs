//! Richer input: a date picker, a color picker, a tag field, a multiple
//! choice with search and a field that records a keyboard shortcut.

use std::fmt;

use crate::a11y::Role;
use crate::anim::SpringKind::{Bouncy, Gentle, Smooth, Snappy};
use crate::buttons::{draw_icon, Icon};
use crate::draw::DrawCmd;
use crate::geom::{Color, Hsv, Rect, Vec2};
use crate::pick::{fold, fuzzy};
use crate::theme::{ACCENT, BORDER, MUTED, ON_ACCENT, RAISED, TEXT};
use crate::ui::{Dir, Key, Node, Ui, LINE_HEIGHT};
use crate::widgets::{Size, Variant};

use crate::locale::Locale;

/// A day of the (proleptic Gregorian) calendar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: i32,
    /// `1..=12`
    pub month: u8,
    /// `1..=31`
    pub day: u8,
}

impl Date {
    /// The date, with the month and day brought into range.
    pub fn new(year: i32, month: u8, day: u8) -> Date {
        let month = month.clamp(1, 12);
        Date { year, month, day: day.clamp(1, Date::days_in_month(year, month)) }
    }

    pub fn is_leap(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    pub fn days_in_month(year: i32, month: u8) -> u8 {
        match month {
            2 if Date::is_leap(year) => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        }
    }

    /// Days since 1 January 1970.
    pub fn to_days(self) -> i64 {
        let y = self.year as i64 - (self.month <= 2) as i64;
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = self.month as i64;
        let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + self.day as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The date `days` days after 1 January 1970.
    pub fn from_days(days: i64) -> Date {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        Date { year: (yoe + era * 400 + (month <= 2) as i64) as i32, month, day }
    }

    /// The day (in UTC) of a Unix time in seconds.
    pub fn from_unix(secs: f64) -> Date {
        Date::from_days((secs / 86_400.0).floor() as i64)
    }

    /// Day of the week, from 0 (Monday) to 6 (Sunday).
    pub fn weekday(self) -> u8 {
        // 1 January 1970 was a Thursday.
        (self.to_days() + 3).rem_euclid(7) as u8
    }

    pub fn add_days(self, days: i64) -> Date {
        Date::from_days(self.to_days() + days)
    }

    /// The same day `months` months later (or earlier), kept within the
    /// month: 31 January plus one month is 28 or 29 February.
    pub fn add_months(self, months: i32) -> Date {
        let m = self.year * 12 + self.month as i32 - 1 + months;
        Date::new(m.div_euclid(12), (m.rem_euclid(12) + 1) as u8, self.day)
    }

    /// Months since year 0: an index for whole months.
    fn month_index(self) -> i32 {
        self.year * 12 + self.month as i32 - 1
    }
}

impl fmt::Display for Date {
    /// `Nov 16, 2026` (in English; see [`Locale::date`] for others).
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&Locale::ENGLISH.date(*self))
    }
}

/// A key combination such as Ctrl+Shift+K, as recorded by
/// [`Ui::shortcut_input`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub ctrl: bool,
    pub shift: bool,
    /// The key itself: a letter or digit in upper case, or the name of a
    /// key in English ("Enter", "Space", "Left", "Right", "Up", "Down",
    /// "Home", "End"), whatever the language it is shown in.
    pub key: String,
}

impl Shortcut {
    /// Its keys in order, one per key cap, named in `locale`.
    pub fn parts_in<'a>(&'a self, locale: &Locale) -> Vec<&'a str> {
        let mut parts = Vec::with_capacity(3);
        if self.ctrl {
            parts.push(locale.keys.ctrl);
        }
        if self.shift {
            parts.push(locale.keys.shift);
        }
        parts.push(locale.key_name(&self.key));
        parts
    }

    /// Its keys in order, in English.
    pub fn parts(&self) -> Vec<&str> {
        self.parts_in(&Locale::ENGLISH)
    }

    /// `Ctrl+Mayús+K` and the like, in `locale`.
    pub fn label(&self, locale: &Locale) -> String {
        self.parts_in(locale).join("+")
    }
}

impl fmt::Display for Shortcut {
    /// `Ctrl+Shift+K`
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.label(&Locale::ENGLISH))
    }
}

/// A calendar glyph: a box with two rings on top and a line.
fn calendar_icon(ui: &mut Ui, c: Vec2, s: f32, color: Color) {
    let r = Rect::new(c.x - s * 0.5, c.y - s * 0.4, s, s * 0.85);
    ui.draw.rect_stroked(r, 3.0, Color::TRANSPARENT, color, 1.5);
    ui.draw.line(Vec2::new(r.x, r.y + s * 0.28), Vec2::new(r.right(), r.y + s * 0.28), 1.5, color);
    for x in [0.3, 0.7] {
        ui.draw.line(Vec2::new(r.x + r.w * x, r.y - s * 0.12), Vec2::new(r.x + r.w * x, r.y + s * 0.1), 1.5, color);
    }
}

/// A field-like box: raised, with a border that warms up on hover and turns
/// accent when `on`.
fn field_box(ui: &mut Ui, r: Rect, hover: f32, on: f32, a: f32) {
    let stroke = ui.pal[BORDER].mix(ui.pal[TEXT], 0.3 * hover).mix(ui.pal[ACCENT], on.clamp(0.0, 1.0));
    ui.draw.rect_stroked(r, ui.theme.radius * 0.7, ui.pal[RAISED].fade(a), stroke.fade(a), 1.0 + 0.6 * on.clamp(0.0, 1.0));
}

impl Ui {
    /// A form field: `label` above whatever `f` builds (a text field, a
    /// picker), and below it `error` when there is one, in a red that
    /// reads well on the theme's background. Screen readers hear the field
    /// as a group named by its label, and the error as soon as it appears
    /// (WCAG 3.3.1). Keys inside `f` are namespaced under `key`.
    ///
    /// ```
    /// # use morfo::{Theme, Ui, Vec2};
    /// # let mut ui = Ui::new(Theme::dark());
    /// # ui.begin(0.0, Vec2::new(800.0, 600.0));
    /// # let mut email = String::new();
    /// let error = (!email.is_empty() && !email.contains('@')).then_some("An email address has an @");
    /// ui.field("email", "Email", error, |ui| {
    ///     ui.text_field("input", &mut email, "you@example.com", 280.0);
    /// });
    /// # ui.end();
    /// ```
    pub fn field(&mut self, key: &str, label: &str, error: Option<&str>, f: impl FnOnce(&mut Ui)) {
        let id = self.declare(key);
        let gap = self.theme.gap * 0.4;
        let (origin, size) = self.group(id, Dir::Column, Vec2::ZERO, gap, None, |ui| {
            ui.within(id, |ui| {
                ui.describe_group(id, Role::Group, label, |ui| {
                    ui.muted(label);
                    f(ui);
                    if let Some(e) = error {
                        ui.error_text(e);
                    }
                })
            })
        });
        self.advance(size);
        let _ = origin;
    }

    /// A line of error text, announced when it appears.
    fn error_text(&mut self, text: &str) {
        let id = self.auto_id();
        let size = self.theme.font * 0.9;
        let w = self.text.width(text, size);
        let target = self.alloc(Vec2::new(w, size * LINE_HEIGHT));
        let (r, life) = self.enter(id, target, Smooth);
        self.describe(id, Role::Status, text, r, None, None);
        let bg = self.pal[crate::theme::BG];
        let light = Color::hex(0xff6b6b);
        let red = if crate::theme::contrast(light, bg) >= 4.5 { light } else { Color::hex(0xb3261e) };
        self.draw.add_text(r.min(), size, red.fade(self.alpha * life), text);
    }

    /// A field showing a date that opens a month calendar below itself.
    /// The arrows of the calendar change month (the days slide across);
    /// the selection springs from day to day. With the field focused, the
    /// arrow keys move the date by days and weeks. Returns `true` when the
    /// date changed.
    pub fn date_picker(&mut self, key: &str, date: &mut Date) -> bool {
        let id = self.declare(key);
        let (font, pad, h) = (self.theme.font, self.theme.pad * 0.8, self.theme.control);
        let widest = (1..=12).map(|m| self.text.width(&self.locale.date(Date::new(2026, m, 28)), font)).fold(0.0, f32::max);
        let target = self.alloc(Vec2::new(widest + pad * 2.0 + 34.0, h));
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        let mut text = self.locale.date(*date);
        self.describe(id, Role::ComboBox, key, r, Some(&text), None);
        if hit.clicked {
            self.open = if self.open == Some(id) { None } else { Some(id) };
        }
        let shown = self.open == Some(id);
        let before = *date;
        // Keyboard: days and weeks.
        if self.focus == Some(id) {
            for k in &self.keys {
                *date = match k {
                    Key::Left => date.add_days(-1),
                    Key::Right => date.add_days(1),
                    Key::Up => date.add_days(-7),
                    Key::Down => date.add_days(7),
                    _ => continue,
                };
            }
        }
        // The month on view: the date's own when opening or moving by keys.
        let n = self.nodes.get_mut(&id).unwrap();
        if (shown && !n.open) || *date != before {
            n.index = date.month_index() as usize;
        }
        n.open = shown;
        let (hover, press, turn) = self.drive3(id, hit.hovered, hit.held, shown as u8 as f32, Smooth);
        let a = self.alpha * life;
        let rr = r.scaled(1.0 - 0.03 * press);
        field_box(self, rr, hover, turn, a);
        text = self.locale.date(*date);
        self.draw.add_text(Vec2::new(rr.x + pad, rr.y + (rr.h - font * LINE_HEIGHT) * 0.5), font, self.pal[TEXT].fade(a), &text);
        let icon = self.pal[MUTED].mix(self.pal[ACCENT], turn.clamp(0.0, 1.0));
        calendar_icon(self, Vec2::new(rr.right() - 20.0, rr.center().y), font, icon.fade(a));

        // The calendar.
        let cell = (self.theme.control * 0.9).round();
        let inner = cell * 7.0;
        let viewed = self.nodes[&id].index as i32;
        let (mut step, mut picked) = (0, None);
        let mut cfg = self.menu_cfg(r);
        cfg.width = Some(inner + 24.0);
        cfg.inset = 12.0;
        cfg.gap = 6.0;
        let selected = *date;
        let saved = self.scope;
        let dismissed = self.floating(id.child("panel"), shown, cfg, |ui| {
            ui.scope = id;
            let (year, month) = (viewed.div_euclid(12), (viewed.rem_euclid(12) + 1) as u8);
            let name = ui.locale.months[month as usize - 1];
            let mut chars = name.chars();
            let title = format!("{}{} {year}", chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default(), chars.as_str());
            // The month centered between the two arrows.
            let button = (ui.theme.control * 0.78).round();
            let room = (inner - 2.0 * button - ui.text.width(&title, ui.theme.font) - 2.0 * ui.theme.gap) * 0.5;
            ui.row(|ui| {
                if ui.icon_button("anterior", Icon::ArrowLeft, Variant::Ghost, Size::Sm).clicked {
                    step = -1;
                }
                ui.space(room.max(0.0));
                ui.label(&title);
                ui.space(room.max(0.0));
                if ui.icon_button("siguiente", Icon::ArrowRight, Variant::Ghost, Size::Sm).clicked {
                    step = 1;
                }
            });
            ui.scope = saved;
            let small = ui.theme.font * 0.8;
            let head = ui.alloc(Vec2::new(inner, small * LINE_HEIGHT));
            let first_day = ui.locale.first_weekday as usize % 7;
            let names = ui.locale.weekdays;
            for i in 0..7 {
                let d = names[(i + first_day) % 7];
                let w = ui.text.width(d, small);
                let x = head.x + i as f32 * cell + (cell - w) * 0.5;
                ui.draw.add_text(Vec2::new(x, head.y), small, ui.pal[MUTED].fade(ui.alpha), d);
            }
            let grid = ui.alloc(Vec2::new(inner, cell * 6.0));
            // A change of month slides the days across.
            let k = ui.steps[Gentle as usize];
            let n = ui.nodes.entry(id.child("slide")).or_insert_with(Node::new);
            if n.touch(ui.frame) {
                n.index = viewed as usize;
            }
            if n.index != viewed as usize {
                let dir = (viewed - n.index as i32).signum() as f32;
                n.index = viewed as usize;
                n.e1.snap(dir);
                n.e1.set(0.0);
            }
            ui.animating |= n.e1.tick(&k);
            let slide = n.e1.get();
            let first = Date::new(year, month, 1);
            let offset = (first.weekday() as usize + 7 - first_day) % 7;
            let days = Date::days_in_month(year, month) as usize;
            let a = ui.alpha * (1.0 - slide.abs()).clamp(0.0, 1.0);
            let dx = slide * cell * 2.0;
            let mut sel_cell = None;
            for day in 1..=days {
                let slot = offset + day - 1;
                let c = Rect::new(grid.x + (slot % 7) as f32 * cell + dx, grid.y + (slot / 7) as f32 * cell, cell, cell);
                let did = id.child("day").with(day as u64);
                let hit = ui.hit_inert(did, c);
                let this = Date::new(year, month, day as u8);
                if hit.clicked {
                    picked = Some(this);
                }
                let on = this == selected;
                if on {
                    sel_cell = Some(c);
                }
                let n = ui.nodes.entry(did).or_insert_with(Node::new);
                let fresh = n.touch(ui.frame);
                n.rect.snap(c);
                let snappy = ui.steps[Snappy as usize];
                ui.animating |= n.hover.drive(hit.hovered as u8 as f32, fresh, &snappy);
                let hover = n.hover.get();
                ui.draw.rect(c.inset(3.0), cell, ui.pal[TEXT].with_alpha(0.08 * hover * a));
                let label = day.to_string();
                let size = ui.theme.font * 0.92;
                let w = ui.text.width(&label, size);
                let weekend = (slot + first_day) % 7 >= 5;
                let ink = if on {
                    ui.pal[ON_ACCENT]
                } else if weekend {
                    ui.pal[MUTED]
                } else {
                    ui.pal[TEXT]
                };
                ui.draw.add_text(Vec2::new(c.center().x - w * 0.5, c.y + (cell - size * LINE_HEIGHT) * 0.5), size, ink.fade(a), &label);
            }
            // The selection springs from day to day (and fades on other months).
            let (bouncy, smooth) = (ui.steps[Bouncy as usize], ui.steps[Smooth as usize]);
            let n = ui.nodes.entry(id.child("pill")).or_insert_with(Node::new);
            let fresh = n.touch(ui.frame);
            if let Some(c) = sel_cell {
                let jump = fresh || n.aux.get() < 0.05;
                ui.animating |= n.rect.drive(c.inset(3.0), jump, &bouncy);
            }
            ui.animating |= n.aux.drive(sel_cell.is_some() as u8 as f32, fresh, &smooth);
            let (pill, on) = (n.rect.get(), n.aux.get().clamp(0.0, 1.0));
            if on > 0.01 {
                ui.draw.rect(pill, cell, ui.pal[ACCENT].fade(ui.alpha * on));
                // Repaint the selected number on top of the pill.
                if sel_cell.is_some() {
                    let label = selected.day.to_string();
                    let size = ui.theme.font * 0.92;
                    let w = ui.text.width(&label, size);
                    let c = pill.center();
                    ui.draw.add_text(Vec2::new(c.x - w * 0.5, c.y - size * LINE_HEIGHT * 0.5), size, ui.pal[ON_ACCENT].fade(ui.alpha * on), &label);
                }
            }
        });
        self.scope = saved;
        if step != 0 {
            let n = self.nodes.get_mut(&id).unwrap();
            n.index = (n.index as i32 + step) as usize;
        }
        if let Some(d) = picked {
            *date = d;
            self.open = None;
            // The keyboard goes back to the field, where the arrows move the date.
            self.focus = Some(id);
        }
        if dismissed && self.open == Some(id) {
            self.open = None;
        }
        if *date != before {
            self.learn(id, key);
            true
        } else {
            false
        }
    }

    /// A color picker: a saturation and brightness square over a hue bar,
    /// with a preview and the color's hex code. Drag in either, or use the
    /// arrow keys on the square. Returns `true` while the color changes.
    pub fn color_picker(&mut self, key: &str, color: &mut Color, width: f32) -> bool {
        const STRIPS: usize = 48;
        let id = self.declare(key);
        let font = self.theme.font;
        let (sq_h, bar_h, gap) = ((width * 0.62).round(), 14.0, 12.0);
        let foot = self.theme.control * 0.8;
        let target = self.alloc(Vec2::new(width, sq_h + gap + bar_h + gap + foot));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        let square = Rect::new(r.x, r.y, r.w, sq_h);
        let bar = Rect::new(r.x, square.bottom() + gap, r.w, bar_h);
        let (sid, hid) = (id.child("sv"), id.child("hue"));
        self.enter(sid, square, Smooth);
        self.enter(hid, bar.inset(-4.0), Smooth);
        let (sq_hit, bar_hit) = (self.hit(sid, square), self.hit(hid, bar.inset(-4.0)));

        // The picker keeps its own hue, which a grey color alone would lose;
        // it takes the color again when it changes from outside.
        let hex = color.to_hex() as usize;
        let n = self.nodes.get_mut(&id).unwrap();
        let mut hsv = Hsv { h: n.e1.target(), s: n.e2.target(), v: n.aux.target() };
        if n.fresh() || n.index != hex + 1 {
            let from = color.to_hsv();
            hsv = Hsv { h: if from.s > 0.001 && from.v > 0.001 { from.h } else { hsv.h }, s: from.s, v: from.v };
        }
        let before = hsv;
        if let (true, Some(p)) = (sq_hit.held, self.input.pointer) {
            hsv.s = ((p.x - square.x) / square.w).clamp(0.0, 1.0);
            hsv.v = 1.0 - ((p.y - square.y) / square.h).clamp(0.0, 1.0);
        }
        if let (true, Some(p)) = (bar_hit.held, self.input.pointer) {
            hsv.h = ((p.x - bar.x) / bar.w).clamp(0.0, 0.9999);
        }
        if self.focus == Some(sid) {
            for k in &self.keys {
                match k {
                    Key::Left => hsv.s = (hsv.s - 0.05).max(0.0),
                    Key::Right => hsv.s = (hsv.s + 0.05).min(1.0),
                    Key::Up => hsv.v = (hsv.v + 0.05).min(1.0),
                    Key::Down => hsv.v = (hsv.v - 0.05).max(0.0),
                    _ => {}
                }
            }
        }
        hsv.h += self.arrows(hid) * 0.02;
        hsv.h = hsv.h.rem_euclid(1.0);
        let changed = hsv != before;
        if changed {
            let alpha = if color.a > 0.0 { color.a } else { 1.0 };
            *color = Color::from_hsv(hsv).with_alpha(alpha);
        }
        if sq_hit.released || bar_hit.released {
            self.learn(id, key);
        }
        let n = self.nodes.get_mut(&id).unwrap();
        n.e1.snap(hsv.h);
        n.e2.snap(hsv.s);
        n.aux.snap(hsv.v);
        n.index = color.to_hex() as usize + 1;
        // The thumbs follow the pointer closely and glide to outside changes.
        let k = self.steps[Snappy as usize];
        let n = self.nodes.get_mut(&sid).unwrap();
        let first = n.fresh();
        let fresh = first || sq_hit.held;
        let anim =
            n.e1.drive(hsv.s, fresh, &k) | n.e2.drive(hsv.v, fresh, &k) | n.hover.drive((sq_hit.hovered || sq_hit.held) as u8 as f32, first, &k);
        let (s, v, sq_hover) = (n.e1.get(), n.e2.get(), n.hover.get());
        let n = self.nodes.get_mut(&hid).unwrap();
        let first = n.fresh();
        let fresh = first || bar_hit.held;
        let anim = anim | n.e1.drive(hsv.h, fresh, &k) | n.hover.drive((bar_hit.hovered || bar_hit.held) as u8 as f32, first, &k);
        let (h, bar_hover) = (n.e1.get(), n.hover.get());
        self.animating |= anim;
        if self.a11y_on {
            let value = format!("#{:06X}", color.to_hex());
            self.describe(sid, Role::Slider, key, square, Some(&value), None);
        }

        // Saturation runs left to right (white to the hue), brightness top
        // to bottom (to black): two stacks of translucent strips.
        let radius = 10.0;
        self.draw.push(DrawCmd::PushClip { rect: square, radius });
        self.draw.rect(square, 0.0, Color::from_hsv(Hsv { h: hsv.h, s: 1.0, v: 1.0 }).fade(a));
        // Strips meet on whole pixels without overlapping: an overlap would
        // show as a darker line.
        let edge = |from: f32, len: f32, i: usize| (from + len * i as f32 / STRIPS as f32).round();
        for i in 0..STRIPS {
            let t = (i as f32 + 0.5) / STRIPS as f32;
            let (x0, x1) = (edge(square.x, square.w, i), edge(square.x, square.w, i + 1));
            self.draw.rect(Rect::new(x0, square.y, x1 - x0, square.h), 0.0, Color::WHITE.with_alpha((1.0 - t) * a));
        }
        for j in 0..STRIPS {
            let t = (j as f32 + 0.5) / STRIPS as f32;
            let (y0, y1) = (edge(square.y, square.h, j), edge(square.y, square.h, j + 1));
            self.draw.rect(Rect::new(square.x, y0, square.w, y1 - y0), 0.0, Color::BLACK.with_alpha(t * a));
        }
        self.draw.push(DrawCmd::PopClip);
        self.draw.rect_stroked(square, radius, Color::TRANSPARENT, self.pal[BORDER].fade(a), 1.0);
        let c = Vec2::new(square.x + square.w * s, square.y + square.h * (1.0 - v));
        let d = 16.0 + 4.0 * sq_hover;
        let ring = Rect::new(c.x - d * 0.5, c.y - d * 0.5, d, d);
        self.draw.push(DrawCmd::Shadow { rect: ring, radius: d, blur: 6.0, offset: Vec2::new(0.0, 1.0), color: Color::BLACK.with_alpha(0.35 * a) });
        self.draw.rect_stroked(ring, d, color.with_alpha(a), Color::WHITE.fade(a), 2.5);

        // The hue bar.
        self.draw.push(DrawCmd::PushClip { rect: bar, radius: bar_h * 0.5 });
        let segs = 36;
        let bw = bar.w / segs as f32;
        for i in 0..segs {
            let hue = (i as f32 + 0.5) / segs as f32;
            self.draw.rect(Rect::new(bar.x + i as f32 * bw, bar.y, bw + 1.0, bar.h), 0.0, Color::from_hsv(Hsv { h: hue, s: 1.0, v: 1.0 }).fade(a));
        }
        self.draw.push(DrawCmd::PopClip);
        let x = bar.x + bar.w * h;
        let d = bar_h + 6.0 + 3.0 * bar_hover;
        let knob = Rect::new(x - d * 0.5, bar.center().y - d * 0.5, d, d);
        self.draw.push(DrawCmd::Shadow { rect: knob, radius: d, blur: 6.0, offset: Vec2::new(0.0, 1.0), color: Color::BLACK.with_alpha(0.35 * a) });
        self.draw.rect_stroked(knob, d, Color::from_hsv(Hsv { h, s: 1.0, v: 1.0 }).fade(a), Color::WHITE.fade(a), 2.5);

        // Preview and hex code.
        let fy = bar.bottom() + gap;
        let sw = foot;
        self.draw.rect_stroked(Rect::new(r.x, fy, sw, sw), sw * 0.3, color.with_alpha(a), self.pal[BORDER].fade(a), 1.0);
        let hex = format!("#{:06X}", color.to_hex());
        self.draw.add_code(Vec2::new(r.x + sw + 12.0, fy + (sw - font * LINE_HEIGHT) * 0.5), font, self.pal[TEXT].fade(a), &hex);
        let rgb = color.to_rgba8();
        let text = format!("{} · {} · {}", rgb[0], rgb[1], rgb[2]);
        let w = self.text.width(&text, font * 0.85);
        self.draw.add_text(Vec2::new(r.right() - w, fy + (sw - font * 0.85 * LINE_HEIGHT) * 0.5), font * 0.85, self.pal[MUTED].fade(a), &text);
        changed
    }

    /// A field that turns what is typed into tags: Enter or a comma adds
    /// one (it flies out of the field into place), the × on a tag or
    /// Backspace on an empty field removes one. Tags wrap onto new lines
    /// and the field grows with them. Returns `true` when the tags changed.
    pub fn tag_input(&mut self, key: &str, tags: &mut Vec<String>, placeholder: &str, width: f32) -> bool {
        let id = self.declare(key);
        let field = id.child("campo");
        let font = self.theme.font * 0.9;
        let (pad, line_h, chip_h, gap) = (6.0, self.theme.control, (self.theme.control * 0.7).round(), 6.0);
        let inner = (width - pad * 2.0).max(40.0);
        let chip_w = |ui: &Ui, t: &str| ui.text.width(t, font) + 12.0 + 22.0;

        // Flow layout, from the box's own top-left corner.
        let mut places = Vec::with_capacity(tags.len());
        let (mut x, mut line) = (0.0f32, 0usize);
        for t in tags.iter() {
            let w = chip_w(self, t).min(inner);
            if x > 0.0 && x + w > inner {
                x = 0.0;
                line += 1;
            }
            places.push(Rect::new(x, line as f32 * line_h + (line_h - chip_h) * 0.5, w, chip_h));
            x += w + gap;
        }
        let min_field = 110.0;
        if x > 0.0 && inner - x < min_field {
            x = 0.0;
            line += 1;
        }
        let field_local = Rect::new(x, line as f32 * line_h, inner - x, line_h);
        let height = (line + 1) as f32 * line_h + pad * 2.0;
        let target = self.alloc(Vec2::new(width, height));
        let (r, life) = self.enter(id, target, Smooth);
        let a = self.alpha * life;
        let focused = self.focus == Some(field);
        let (hover, _, on) = {
            let over = self.pointer_in(r);
            self.drive3(id, over, false, focused as u8 as f32, Smooth)
        };
        field_box(self, r, hover, on, a);
        let origin = Vec2::new(target.x + pad, target.y + pad);
        let field_rect = field_local.translate(origin);

        // The tags.
        let mut changed = false;
        let mut remove = None;
        let snappy = self.steps[Snappy as usize];
        for (i, t) in tags.iter().enumerate() {
            let cid = id.child("tag").child(t);
            let (cr, cl) = self.enter(cid, places[i].translate(origin), Smooth);
            let x_rect = Rect::new(cr.right() - 24.0, cr.y, 24.0, cr.h);
            let xid = cid.child("x");
            self.enter(xid, x_rect, Snappy);
            let hit = self.hit(xid, x_rect);
            if self.a11y_on {
                let name = Locale::fill(self.locale.remove, &[t]);
                self.describe(xid, Role::Button, &name, x_rect, None, None);
            }
            if hit.clicked {
                remove = Some(i);
            }
            let n = self.nodes.get_mut(&xid).unwrap();
            let fresh = n.fresh();
            self.animating |= n.hover.drive(hit.hovered as u8 as f32, fresh, &snappy);
            let xh = n.hover.get();
            let ca = a * cl.clamp(0.0, 1.0);
            let cr = cr.scaled(0.85 + 0.15 * cl.clamp(0.0, 1.0));
            self.draw.rect(cr, cr.h * 0.5, self.pal[ACCENT].with_alpha(0.18 * ca));
            let label = crate::navigate::ellipsize(self, t, cr.w - 30.0, font);
            self.draw.add_text(Vec2::new(cr.x + 12.0, cr.y + (cr.h - font * LINE_HEIGHT) * 0.5), font, self.pal[TEXT].fade(ca), &label);
            let xc = Vec2::new(cr.right() - 13.0, cr.center().y);
            self.draw.rect(Rect::new(xc.x - 9.0, xc.y - 9.0, 18.0, 18.0), 18.0, self.pal[TEXT].with_alpha(0.12 * xh * ca));
            draw_icon(&mut self.draw, Icon::Close, xc, 9.0, self.pal[MUTED].mix(self.pal[TEXT], xh).fade(ca));
        }

        // The field, without a box of its own, after the last tag.
        let text_id = id.child("texto");
        let mut text = self.strings.remove(&text_id).unwrap_or_default();
        let was_empty = text.is_empty();
        let mut edit = None;
        let shown_placeholder = if tags.is_empty() { placeholder } else { "" };
        self.group(id.child("fila"), Dir::Row, Vec2::ZERO, 0.0, Some(field_rect.min()), |ui| {
            edit = Some(ui.text_field_id(field, &mut text, shown_placeholder, field_rect.w, false));
        });
        let edit = edit.unwrap_or_default();
        let mut add = Vec::new();
        if text.contains(',') {
            let mut parts: Vec<&str> = text.split(',').collect();
            let rest = parts.pop().unwrap_or("").to_string();
            add.extend(parts.iter().map(|p| p.trim().to_string()));
            text = rest;
        }
        if edit.submitted {
            add.push(text.trim().to_string());
            text.clear();
        }
        for t in add.into_iter().filter(|t| !t.is_empty()) {
            if !tags.contains(&t) {
                // It leaves from the field and flies to its place.
                let from = Rect::new(field_rect.x, field_rect.y + (line_h - chip_h) * 0.5, chip_w(self, &t), chip_h);
                let frame = self.frame;
                self.nodes.entry(id.child("tag").child(&t)).or_insert_with(Node::new).launch(frame, from);
                tags.push(t);
                changed = true;
            }
        }
        if focused && was_empty && self.keys.contains(&Key::Backspace) && remove.is_none() {
            remove = tags.len().checked_sub(1);
        }
        if let Some(i) = remove {
            tags.remove(i);
            changed = true;
        }
        self.strings.insert(text_id, text);
        // A click anywhere in the box goes to the field.
        if self.hit_inert(id.child("caja"), r).clicked && self.focus != Some(field) {
            self.focus = Some(field);
            self.cursor = usize::MAX;
            self.anchor = None;
        }
        if changed {
            self.learn(id, key);
        }
        changed
    }

    /// A field for choosing any number of `options`. It shows the chosen
    /// ones as small tags; clicking opens a list with a search box on top
    /// (accents and case don't matter) where each option toggles. Arrows
    /// and Enter work in the list. Returns `true` when the choice changed;
    /// `selected` holds indices into `options`, in the order chosen.
    pub fn multi_select(&mut self, key: &str, options: &[&str], selected: &mut Vec<usize>, placeholder: &str, width: f32) -> bool {
        let id = self.declare(key);
        let (font, h) = (self.theme.font, self.theme.control);
        let small = font * 0.85;
        let target = self.alloc(Vec2::new(width, h));
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        let summary: Vec<&str> = selected.iter().filter_map(|&i| options.get(i).copied()).collect();
        self.describe(id, Role::ComboBox, key, r, Some(&summary.join(", ")), None);
        if hit.clicked {
            self.open = if self.open == Some(id) { None } else { Some(id) };
        }
        let shown = self.open == Some(id);
        let (hover, press, turn) = self.drive3(id, hit.hovered, hit.held, shown as u8 as f32, Smooth);
        let a = self.alpha * life;
        field_box(self, r.scaled(1.0 - 0.02 * press), hover, turn, a);

        // The chosen ones as small pills, as many as fit, then "+N".
        let room = r.w - 44.0;
        let (mut x, mut fitted) = (r.x + 8.0, 0);
        for (k, label) in summary.iter().enumerate() {
            let w = self.text.width(label, small) + 18.0;
            let rest = summary.len() - k - 1;
            let more = if rest > 0 { self.text.width("+00", small) + 12.0 } else { 0.0 };
            if x + w + more > r.x + room {
                break;
            }
            let pid = id.child("pill").child(label);
            let ph = (h * 0.62).round();
            let (pr, pl) = self.enter(pid, Rect::new(x, r.y + (h - ph) * 0.5, w, ph), Smooth);
            let pa = a * pl.clamp(0.0, 1.0);
            self.draw.rect(pr, ph * 0.5, self.pal[ACCENT].with_alpha(0.18 * pa));
            self.draw.add_text(Vec2::new(pr.x + 9.0, pr.y + (ph - small * LINE_HEIGHT) * 0.5), small, self.pal[TEXT].fade(pa), label);
            x += w + 6.0;
            fitted += 1;
        }
        if fitted < summary.len() {
            let more = format!("+{}", summary.len() - fitted);
            self.draw.add_text(Vec2::new(x + 2.0, r.y + (h - small * LINE_HEIGHT) * 0.5), small, self.pal[MUTED].fade(a), &more);
        } else if summary.is_empty() {
            self.draw.add_text(Vec2::new(r.x + 12.0, r.y + (h - font * LINE_HEIGHT) * 0.5), font, self.pal[MUTED].fade(a), placeholder);
        }
        let c = Vec2::new(r.right() - 20.0, r.center().y);
        let flip = turn.clamp(0.0, 1.0);
        let s = font * 0.8;
        let ink = self.pal[MUTED].fade(a);
        self.draw.line(c + Vec2::new(-0.3 * s, (-0.12 + 0.24 * flip) * s), c + Vec2::new(0.0, (0.18 - 0.36 * flip) * s), 2.0, ink);
        self.draw.line(c + Vec2::new(0.0, (0.18 - 0.36 * flip) * s), c + Vec2::new(0.3 * s, (-0.12 + 0.24 * flip) * s), 2.0, ink);

        // The list, with its search box.
        let (field, query_id) = (id.child("buscar"), id.child("consulta"));
        let n = self.nodes.get_mut(&id).unwrap();
        if shown && !n.open {
            n.index = 0;
            self.strings.insert(query_id, String::new());
            self.focus = Some(field);
            self.cursor = 0;
            self.focus_claimed = true;
        }
        let n = self.nodes.get_mut(&id).unwrap();
        n.open = shown;
        let mut index = n.index;
        let mut query = self.strings.remove(&query_id).unwrap_or_default();
        let q = fold(&query);
        let mut matches: Vec<(usize, i32)> = options.iter().enumerate().filter_map(|(i, o)| fuzzy(&fold(o), &q).map(|s| (i, s))).collect();
        matches.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let (up, down) = (self.keys.iter().filter(|k| **k == Key::Up).count(), self.keys.iter().filter(|k| **k == Key::Down).count());
        if shown {
            index = (index + down).saturating_sub(up).min(matches.len().saturating_sub(1));
        }
        let mut cfg = self.menu_cfg(r);
        cfg.width = Some(width);
        cfg.inset = 8.0;
        cfg.gap = 2.0;
        let row_w = width - 16.0;
        let mut toggled = None;
        let mut clear = false;
        let mut reset = false;
        let chosen = selected.clone();
        let dismissed = self.floating(id.child("panel"), shown, cfg, |ui| {
            let e = ui.text_field_id(field, &mut query, ui.locale.search, row_w, true);
            reset = e.changed;
            if e.submitted {
                toggled = matches.get(index).map(|m| m.0);
            }
            ui.space(4.0);
            for (k, &(i, _)) in matches.iter().take(8).enumerate() {
                if ui.option_row(id.child("op").with(i as u64), options[i], row_w, k == index, chosen.contains(&i)) {
                    toggled = Some(i);
                }
            }
            if matches.is_empty() {
                ui.muted(ui.locale.no_results);
            }
            if !chosen.is_empty() {
                ui.separator();
                let saved = ui.scope;
                ui.scope = id;
                clear = ui.button_with("quitar", ui.locale.clear_all, Variant::Ghost, Size::Sm).clicked;
                ui.scope = saved;
            }
        });
        if reset {
            index = 0;
        }
        let mut changed = false;
        if let Some(i) = toggled {
            match selected.iter().position(|&s| s == i) {
                Some(at) => {
                    selected.remove(at);
                }
                None => selected.push(i),
            }
            changed = true;
        }
        if clear {
            selected.clear();
            changed = true;
        }
        if changed {
            self.learn(id, key);
        }
        if dismissed && self.open == Some(id) {
            self.open = None;
            if self.focus == Some(field) {
                self.focus = Some(id);
            }
        }
        let n = self.nodes.get_mut(&id).unwrap();
        n.index = index;
        self.strings.insert(query_id, query);
        changed
    }

    /// A field that records a keyboard shortcut: click it (or reach it with
    /// Tab) and press a combination; it shows it as key caps. Backspace or
    /// Delete clear it, Escape leaves it as it was. Combinations with Ctrl
    /// need the host to pass them with [`chord`](Self::chord) while
    /// [`wants_keys`](Self::wants_keys) is true. Returns `true` when the
    /// shortcut changed.
    pub fn shortcut_input(&mut self, key: &str, value: &mut Option<Shortcut>, width: f32) -> bool {
        let id = self.declare(key);
        let (font, h) = (self.theme.font, self.theme.control);
        let target = self.alloc(Vec2::new(width, h));
        let (r, life) = self.enter(id, target, Smooth);
        let hit = self.hit(id, r);
        self.capture_ids.push(id);
        if self.a11y_on {
            let text = value.as_ref().map(|s| s.label(&self.locale));
            self.describe(id, Role::TextInput, key, r, text.as_deref(), None);
        }
        let mut focused = self.focus == Some(id);
        let mut changed = false;
        if focused && !self.focus_changed {
            let named = self.keys.iter().find_map(|k| match k {
                Key::Enter => Some("Enter"),
                Key::Space => Some("Space"),
                Key::Left => Some("Left"),
                Key::Right => Some("Right"),
                Key::Up => Some("Up"),
                Key::Down => Some("Down"),
                Key::Home => Some("Home"),
                Key::End => Some("End"),
                _ => None,
            });
            let pressed = self.chord.clone().or_else(|| named.map(str::to_string)).or_else(|| {
                let t = self.typed.trim();
                (!t.is_empty()).then(|| t.to_string())
            });
            if self.keys.contains(&Key::Escape) {
                self.focus = None;
                focused = false;
            } else if self.keys.iter().any(|k| matches!(k, Key::Backspace | Key::Delete)) {
                changed = value.is_some();
                *value = None;
            } else if let Some(k) = pressed {
                let key = if k.chars().count() == 1 { k.to_uppercase() } else { k };
                let shortcut = Shortcut { ctrl: self.mods.ctrl, shift: self.mods.shift && key != "Space", key };
                changed = value.as_ref() != Some(&shortcut);
                *value = Some(shortcut);
                // Recorded: let go of the keyboard.
                self.focus = None;
                focused = false;
            }
        }
        if changed {
            self.learn(id, key);
        }
        let (hover, _, on) = self.drive3(id, hit.hovered, hit.held, focused as u8 as f32, Smooth);
        let a = self.alpha * life;
        field_box(self, r, hover, on, a);
        let small = font * 0.85;
        if focused {
            self.draw.add_text(Vec2::new(r.x + 12.0, r.y + (h - font * LINE_HEIGHT) * 0.5), font, self.pal[MUTED].fade(a), self.locale.press_keys);
        } else if let Some(s) = value.as_ref() {
            let cap_h = (h * 0.62).round();
            let mut x = r.x + 10.0;
            let locale = self.locale;
            for (i, part) in s.parts_in(&locale).iter().enumerate() {
                if i > 0 {
                    self.draw.add_text(Vec2::new(x, r.y + (h - small * LINE_HEIGHT) * 0.5), small, self.pal[MUTED].fade(a), "+");
                    x += self.text.width("+", small) + 6.0;
                }
                let w = (self.text.width(part, small) + 16.0).max(cap_h);
                let kid = id.child("cap").child(part);
                let (kr, kl) = self.enter(kid, Rect::new(x, r.y + (h - cap_h) * 0.5 - 1.0, w, cap_h), Bouncy);
                let ka = a * kl.clamp(0.0, 1.0);
                // A key cap: a darker base under a lighter top.
                self.draw.rect(kr.translate(Vec2::new(0.0, 2.0)), 6.0, self.pal[BORDER].fade(ka));
                self.draw.rect_stroked(kr, 6.0, self.pal[RAISED].mix(self.pal[TEXT], 0.06).fade(ka), self.pal[BORDER].fade(ka), 1.0);
                let tw = self.text.width(part, small);
                self.draw.add_text(
                    Vec2::new(kr.center().x - tw * 0.5, kr.y + (cap_h - small * LINE_HEIGHT) * 0.5),
                    small,
                    self.pal[TEXT].fade(ka),
                    part,
                );
                x += w + 6.0;
            }
        } else {
            self.draw.add_text(Vec2::new(r.x + 12.0, r.y + (h - font * LINE_HEIGHT) * 0.5), font, self.pal[MUTED].fade(a), self.locale.no_shortcut);
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_arithmetic() {
        assert_eq!(Date::new(1970, 1, 1).to_days(), 0);
        assert_eq!(Date::from_days(0), Date::new(1970, 1, 1));
        let d = Date::new(2026, 11, 16);
        assert_eq!(Date::from_days(d.to_days()), d);
        assert_eq!(d.weekday(), 0, "16 Nov 2026 is a Monday");
        assert_eq!(Date::new(2000, 2, 29).add_days(1), Date::new(2000, 3, 1));
        assert_eq!(Date::new(2024, 1, 31).add_months(1), Date::new(2024, 2, 29));
        assert_eq!(Date::new(2026, 12, 15).add_months(1), Date::new(2027, 1, 15));
        assert_eq!(Date::new(2026, 1, 15).add_months(-1), Date::new(2025, 12, 15));
        assert_eq!(Date::days_in_month(1900, 2), 28);
        assert_eq!(Date::days_in_month(2000, 2), 29);
        assert_eq!(Date::new(2026, 2, 31), Date::new(2026, 2, 28));
        assert_eq!(Date::from_unix(1_763_251_200.0), Date::new(2025, 11, 16));
        assert_eq!(d.to_string(), "Nov 16, 2026");
        for days in [-800_000i64, -1, 59, 11_016, 20_000, 2_932_896] {
            assert_eq!(Date::from_days(days).to_days(), days);
        }
    }

    #[test]
    fn shortcuts_read_as_people_write_them() {
        let s = Shortcut { ctrl: true, shift: true, key: "K".into() };
        assert_eq!(s.to_string(), "Ctrl+Shift+K");
        assert_eq!(s.label(&Locale::SPANISH), "Ctrl+Mayús+K");
        let enter = Shortcut { ctrl: false, shift: false, key: "Enter".into() };
        assert_eq!(enter.parts(), ["Enter"]);
        assert_eq!(enter.parts_in(&Locale::SPANISH), ["Intro"]);
    }

    #[test]
    fn hsv_round_trips() {
        for hex in [0x7c6cff, 0x000000, 0xffffff, 0x22c3a6, 0xff0000, 0x808080, 0x0000ff] {
            let c = Color::hex(hex);
            assert_eq!(Color::from_hsv(c.to_hsv()).to_hex(), hex, "{hex:06x}");
        }
        let red = Color::hex(0xff0000).to_hsv();
        assert_eq!((red.h, red.s, red.v), (0.0, 1.0, 1.0));
    }
}
