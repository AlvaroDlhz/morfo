//! The words and formats the components themselves show: month and day
//! names, key names, "More…", "No results", how numbers and dates are
//! written, and what screen readers are told. English by default; Spanish
//! included; any other is a `const` away.

use crate::forms::Date;

/// How a date is written in full.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DateOrder {
    /// `16 Nov 2026`
    DayMonthYear,
    /// `Nov 16, 2026`
    MonthDayYear,
    /// `2026-11-16`
    YearMonthDay,
}

/// Names of the keys a [`Shortcut`](crate::Shortcut) can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyNames {
    pub ctrl: &'static str,
    pub shift: &'static str,
    pub enter: &'static str,
    pub space: &'static str,
    pub left: &'static str,
    pub right: &'static str,
    pub up: &'static str,
    pub down: &'static str,
    pub home: &'static str,
    pub end: &'static str,
}

/// Every word and format the components show by themselves. A translation
/// is a `const` like [`Locale::ENGLISH`]; give it to
/// [`Ui::set_locale`](crate::Ui::set_locale). Text that comes from your
/// own code (labels, titles) is yours to translate.
///
/// Templates hold `{}` where a number goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Locale {
    /// Month names, January first, as written inside a sentence.
    pub months: [&'static str; 12],
    /// Short month names, January first.
    pub months_short: [&'static str; 12],
    /// One- or two-letter day names, Monday first, for calendar headers.
    pub weekdays: [&'static str; 7],
    /// The day weeks start on: `0` Monday … `6` Sunday.
    pub first_weekday: u8,
    pub date_order: DateOrder,
    pub decimal: char,
    /// Thousands separator, used from [`group_from`](Self::group_from)
    /// digits on (`None`: never).
    pub group: Option<char>,
    pub group_from: u8,
    pub keys: KeyNames,
    /// The entry under which an adaptive menu folds what is rarely used.
    pub more: &'static str,
    /// Placeholder of a search box inside a list.
    pub search: &'static str,
    pub no_results: &'static str,
    pub clear_all: &'static str,
    /// What a tag's close button removes: `Remove {}`.
    pub remove: &'static str,
    pub press_keys: &'static str,
    pub no_shortcut: &'static str,
    pub type_command: &'static str,
    pub no_commands: &'static str,
    pub previous: &'static str,
    pub next: &'static str,
    pub page: &'static str,
    pub previous_month: &'static str,
    pub next_month: &'static str,
    pub minimize: &'static str,
    pub maximize: &'static str,
    pub close: &'static str,
    /// The state of each step of a process.
    pub step_current: &'static str,
    pub step_done: &'static str,
    pub step_pending: &'static str,
    /// `{} values` (a chart series, for screen readers).
    pub values: &'static str,
    /// `{} modules · {} lessons` (a course index).
    pub course_summary: &'static str,
    /// `MODULE {}`
    pub module: &'static str,
    /// Written right to left (Hebrew, Arabic, Persian…): the whole
    /// interface is laid out as in a mirror, and lines read from the right.
    pub rtl: bool,
}

impl Locale {
    pub const ENGLISH: Locale = Locale {
        months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
        months_short: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
        weekdays: ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
        first_weekday: 6,
        date_order: DateOrder::MonthDayYear,
        decimal: '.',
        group: Some(','),
        group_from: 4,
        keys: KeyNames {
            ctrl: "Ctrl",
            shift: "Shift",
            enter: "Enter",
            space: "Space",
            left: "Left",
            right: "Right",
            up: "Up",
            down: "Down",
            home: "Home",
            end: "End",
        },
        more: "More…",
        search: "Search…",
        no_results: "No results",
        clear_all: "Clear all",
        remove: "Remove {}",
        press_keys: "Press a key combination…",
        no_shortcut: "No shortcut",
        type_command: "Type a command…",
        no_commands: "No command matches.",
        previous: "Previous",
        next: "Next",
        page: "page",
        previous_month: "Previous month",
        next_month: "Next month",
        minimize: "Minimize",
        maximize: "Maximize",
        close: "Close",
        step_current: "current",
        step_done: "done",
        step_pending: "pending",
        values: "{} values",
        course_summary: "{} modules · {} lessons",
        module: "MODULE {}",
        rtl: false,
    };

    pub const SPANISH: Locale = Locale {
        months: ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
        months_short: ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"],
        weekdays: ["L", "M", "X", "J", "V", "S", "D"],
        first_weekday: 0,
        date_order: DateOrder::DayMonthYear,
        decimal: ',',
        group: Some('.'),
        group_from: 5,
        keys: KeyNames {
            ctrl: "Ctrl",
            shift: "Mayús",
            enter: "Intro",
            space: "Espacio",
            left: "Izquierda",
            right: "Derecha",
            up: "Arriba",
            down: "Abajo",
            home: "Inicio",
            end: "Fin",
        },
        more: "Más…",
        search: "Buscar…",
        no_results: "Sin resultados",
        clear_all: "Quitar todas",
        remove: "Quitar {}",
        press_keys: "Pulsa una combinación…",
        no_shortcut: "Sin atajo",
        type_command: "Escribe un comando…",
        no_commands: "Ningún comando coincide.",
        previous: "Anterior",
        next: "Siguiente",
        page: "página",
        previous_month: "Mes anterior",
        next_month: "Mes siguiente",
        minimize: "Minimizar",
        maximize: "Maximizar",
        close: "Cerrar",
        step_current: "actual",
        step_done: "hecho",
        step_pending: "pendiente",
        values: "{} valores",
        course_summary: "{} módulos · {} lecciones",
        module: "MÓDULO {}",
        rtl: false,
    };

    /// This locale, written right to left (or not): for a translation into
    /// Hebrew or Arabic built on top of another, `Locale { months: …,
    /// ..Locale::ENGLISH.with_rtl(true) }`.
    pub const fn with_rtl(mut self, rtl: bool) -> Locale {
        self.rtl = rtl;
        self
    }

    /// `template` with each `{}` replaced by the next of `args`.
    pub fn fill(template: &str, args: &[&dyn std::fmt::Display]) -> String {
        let mut out = String::with_capacity(template.len() + 8);
        let mut rest = template;
        let mut args = args.iter();
        while let Some(at) = rest.find("{}") {
            out.push_str(&rest[..at]);
            if let Some(a) = args.next() {
                use std::fmt::Write;
                let _ = write!(out, "{a}");
            }
            rest = &rest[at + 2..];
        }
        out.push_str(rest);
        out
    }

    /// `v` with `decimals` decimals, this locale's decimal mark and, from
    /// [`group_from`](Self::group_from) digits on, its thousands separator.
    pub fn number(&self, v: f64, decimals: usize) -> String {
        let mut out = String::new();
        self.write_number(v, decimals, &mut out);
        out
    }

    /// Like [`number`](Self::number), into `out` (cleared first).
    pub fn write_number(&self, v: f64, decimals: usize, out: &mut String) {
        use std::fmt::Write;
        out.clear();
        let mut digits = String::new();
        let _ = write!(digits, "{:.*}", decimals, v.abs());
        let (int, frac) = digits.split_once('.').unwrap_or((&digits, ""));
        if v < 0.0 && digits.bytes().any(|b| b.is_ascii_digit() && b != b'0') {
            out.push('-');
        }
        let n = int.len();
        let grouped = self.group.filter(|_| n >= self.group_from as usize);
        for (i, c) in int.chars().enumerate() {
            if let Some(g) = grouped {
                if i > 0 && (n - i).is_multiple_of(3) {
                    out.push(g);
                }
            }
            out.push(c);
        }
        if !frac.is_empty() {
            out.push(self.decimal);
            out.push_str(frac);
        }
    }

    /// `date` written in full, in this locale's order.
    pub fn date(&self, date: Date) -> String {
        let month = self.months_short[(date.month.clamp(1, 12) - 1) as usize];
        match self.date_order {
            DateOrder::DayMonthYear => format!("{} {month} {}", date.day, date.year),
            DateOrder::MonthDayYear => format!("{month} {}, {}", date.day, date.year),
            DateOrder::YearMonthDay => format!("{:04}-{:02}-{:02}", date.year, date.month, date.day),
        }
    }

    /// The name this locale gives to `key`, as [`Shortcut`](crate::Shortcut)
    /// stores it (`"Enter"`, `"Left"`… or a letter or digit, kept as is).
    pub fn key_name<'a>(&self, key: &'a str) -> &'a str {
        let k = &self.keys;
        match key {
            "Enter" => k.enter,
            "Space" => k.space,
            "Left" => k.left,
            "Right" => k.right,
            "Up" => k.up,
            "Down" => k.down,
            "Home" => k.home,
            "End" => k.end,
            other => other,
        }
    }
}

impl Default for Locale {
    fn default() -> Self {
        Locale::ENGLISH
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_dates_in_each_locale() {
        let (en, es) = (Locale::ENGLISH, Locale::SPANISH);
        assert_eq!(en.number(12480.0, 0), "12,480");
        assert_eq!(es.number(12480.0, 0), "12.480");
        assert_eq!(es.number(1250.0, 0), "1250");
        assert_eq!(en.number(1250.0, 0), "1,250");
        assert_eq!(en.number(98.64, 1), "98.6");
        assert_eq!(es.number(98.64, 1), "98,6");
        assert_eq!(es.number(-3.5, 1), "-3,5");
        assert_eq!(es.number(-0.01, 1), "0,0");
        assert_eq!(en.number(1234567.0, 2), "1,234,567.00");
        let d = Date::new(2026, 11, 16);
        assert_eq!(en.date(d), "Nov 16, 2026");
        assert_eq!(es.date(d), "16 nov 2026");
        assert_eq!(Locale { date_order: DateOrder::YearMonthDay, ..en }.date(d), "2026-11-16");
        assert_eq!(Locale::fill(en.course_summary, &[&5, &18]), "5 modules · 18 lessons");
        assert_eq!(Locale::fill("{} of {}", &[&1]), "1 of ");
        assert_eq!(es.key_name("Enter"), "Intro");
        assert_eq!(es.key_name("K"), "K");
    }
}
