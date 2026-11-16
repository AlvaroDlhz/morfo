//! The state that travels from one build of the app to the next.
//!
//! Values are stored by name, as text. A field that the new build no longer
//! has is ignored and one it has just gained keeps its default, so the
//! state survives the very code changes that caused the reload.

use std::collections::BTreeMap;

const HEADER: &str = "morfo-live1";

/// Named values saved before a reload and read back after it.
///
/// ```
/// use morfo_live::Snapshot;
///
/// let mut snap = Snapshot::new();
/// snap.set("tab", &3usize);
/// snap.set("tags", &[true, false]);
///
/// let snap = Snapshot::decode(&snap.encode()).unwrap();
/// let mut tab = 0usize;
/// snap.load("tab", &mut tab);
/// assert_eq!(tab, 3);
/// assert_eq!(snap.get::<[bool; 2]>("tags"), Some([true, false]));
/// assert_eq!(snap.get::<u8>("gone"), None);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    map: BTreeMap<String, String>,
}

impl Snapshot {
    pub fn new() -> Snapshot {
        Snapshot::default()
    }

    pub fn set<T: Value>(&mut self, key: &str, value: &T) {
        self.map.insert(key.to_string(), value.write());
    }

    /// The value saved as `key`, if there is one and it still reads as a `T`.
    pub fn get<T: Value>(&self, key: &str) -> Option<T> {
        T::read(self.map.get(key)?)
    }

    /// Overwrites `into` with the value saved as `key`; leaves it alone (and
    /// returns `false`) when there is none.
    pub fn load<T: Value>(&self, key: &str, into: &mut T) -> bool {
        match self.get(key) {
            Some(value) => {
                *into = value;
                true
            }
            None => false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Text form: one `key<TAB>value` line per entry.
    pub fn encode(&self) -> String {
        let mut out = String::from(HEADER);
        out.push('\n');
        for (key, value) in &self.map {
            escape(key, &mut out);
            out.push('\t');
            escape(value, &mut out);
            out.push('\n');
        }
        out
    }

    /// Inverse of [`encode`](Self::encode). `None` on malformed input.
    pub fn decode(text: &str) -> Option<Snapshot> {
        let mut lines = text.lines();
        if lines.next()? != HEADER {
            return None;
        }
        let mut map = BTreeMap::new();
        for line in lines.filter(|l| !l.is_empty()) {
            let (key, value) = line.split_once('\t')?;
            map.insert(unescape(key)?, unescape(value)?);
        }
        Some(Snapshot { map })
    }
}

fn escape(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
}

fn unescape(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        out.push(match c {
            '\\' => match chars.next()? {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '\\' => '\\',
                _ => return None,
            },
            c => c,
        });
    }
    Some(out)
}

/// Something a [`Snapshot`] can hold: numbers, `bool`, `char`, `String`, and
/// `Option`s, `Vec`s, arrays and pairs of those.
pub trait Value: Sized {
    fn write(&self) -> String;
    fn read(text: &str) -> Option<Self>;
}

macro_rules! plain {
    ($($t:ty)*) => {$(
        impl Value for $t {
            fn write(&self) -> String {
                self.to_string()
            }
            fn read(text: &str) -> Option<Self> {
                text.parse().ok()
            }
        }
    )*};
}
plain!(bool char u8 u16 u32 u64 usize i8 i16 i32 i64 isize f32 f64 String);

/// Items one after another, each closed by a comma (so no items and one
/// empty item are told apart).
fn join(items: impl Iterator<Item = String>) -> String {
    let mut out = String::new();
    for item in items {
        for c in item.chars() {
            if c == '\\' || c == ',' {
                out.push('\\');
            }
            out.push(c);
        }
        out.push(',');
    }
    out
}

fn split(text: &str) -> Option<Vec<String>> {
    let (mut items, mut item) = (Vec::new(), String::new());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => item.push(chars.next()?),
            ',' => items.push(std::mem::take(&mut item)),
            c => item.push(c),
        }
    }
    item.is_empty().then_some(items)
}

impl<T: Value> Value for Vec<T> {
    fn write(&self) -> String {
        join(self.iter().map(Value::write))
    }
    fn read(text: &str) -> Option<Self> {
        split(text)?.iter().map(|item| T::read(item)).collect()
    }
}

impl<T: Value> Value for Option<T> {
    fn write(&self) -> String {
        join(self.iter().map(Value::write))
    }
    fn read(text: &str) -> Option<Self> {
        match split(text)?.as_slice() {
            [] => Some(None),
            [item] => T::read(item).map(Some),
            _ => None,
        }
    }
}

impl<T: Value, const N: usize> Value for [T; N] {
    fn write(&self) -> String {
        join(self.iter().map(Value::write))
    }
    fn read(text: &str) -> Option<Self> {
        Vec::<T>::read(text)?.try_into().ok()
    }
}

impl<A: Value, B: Value> Value for (A, B) {
    fn write(&self) -> String {
        join([self.0.write(), self.1.write()].into_iter())
    }
    fn read(text: &str) -> Option<Self> {
        match split(text)?.as_slice() {
            [a, b] => Some((A::read(a)?, B::read(b)?)),
            _ => None,
        }
    }
}

/// Gives a struct `save(&self, &mut Snapshot)` and
/// `restore(&mut self, &Snapshot)` for the fields listed, each stored under
/// its own name.
///
/// ```
/// #[derive(Default)]
/// struct State {
///     tab: usize,
///     name: String,
///     scratch: Vec<u8>, // not listed: starts over after a reload
/// }
/// morfo_live::persist!(State { tab, name });
///
/// let before = State { tab: 2, name: "Ada".into(), scratch: vec![1] };
/// let mut snap = morfo_live::Snapshot::new();
/// before.save(&mut snap);
///
/// let mut after = State::default();
/// after.restore(&snap);
/// assert_eq!((after.tab, after.name.as_str()), (2, "Ada"));
/// ```
#[macro_export]
macro_rules! persist {
    ($ty:ty { $($field:ident),* $(,)? }) => {
        impl $ty {
            /// Writes the persistent fields into `snap`.
            pub fn save(&self, snap: &mut $crate::Snapshot) {
                $(snap.set(stringify!($field), &self.$field);)*
            }

            /// Reads back the fields `snap` has; the rest keep their value.
            pub fn restore(&mut self, snap: &$crate::Snapshot) {
                $(snap.load(stringify!($field), &mut self.$field);)*
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round<T: Value + PartialEq + std::fmt::Debug>(value: T) {
        let mut snap = Snapshot::new();
        snap.set("v", &value);
        let back = Snapshot::decode(&snap.encode()).expect("decodes");
        assert_eq!(back.get::<T>("v"), Some(value));
    }

    #[test]
    fn values_round_trip() {
        round(true);
        round(-7i32);
        round(0.1f32);
        round(f64::MAX);
        round('ñ');
        round(String::new());
        round("tabs\tand\nlines, commas \\ and = signs\r".to_string());
        round(Some(3usize));
        round(None::<usize>);
        round(Some(String::new()));
        round(Vec::<String>::new());
        round(vec![String::new()]);
        round(vec!["a,b".to_string(), "c\\".to_string(), String::new()]);
        round([true, false, true]);
        round((0.25f32, 0.7f32));
        round(Some((2usize, false)));
        round(vec![Some(vec!["x,".to_string()]), None]);
    }

    #[test]
    fn tolerates_a_changed_shape() {
        let mut snap = Snapshot::new();
        snap.set("count", &"many".to_string());
        snap.set("pair", &[1u8, 2, 3]);
        let mut count = 5u32;
        // Saved as text, now a number: the default stays.
        assert!(!snap.load("count", &mut count));
        assert_eq!(count, 5);
        assert_eq!(snap.get::<[u8; 2]>("pair"), None);
        assert_eq!(snap.get::<u8>("missing"), None);
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(Snapshot::decode(""), None);
        assert_eq!(Snapshot::decode("something else\n"), None);
        assert_eq!(Snapshot::decode("morfo-live1\nno-tab-here\n"), None);
        assert_eq!(Snapshot::decode("morfo-live1\nkey\tbad \\x escape\n"), None);
        assert!(Snapshot::decode("morfo-live1\n").unwrap().is_empty());
    }
}
