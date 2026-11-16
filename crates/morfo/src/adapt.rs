//! Usage learning: the part that makes two users see two different interfaces.
//!
//! A [`Profile`] keeps one small record per element: an interaction score that
//! decays exponentially with time (so habits fade if they change). From it the
//! interface derives *where* things go ([`Profile::arrange`]) and *how much
//! room* they deserve ([`Profile::prominence`]). Both have hysteresis, so the
//! layout only moves when a preference is clear — never back and forth.
//!
//! The profile knows nothing about rendering or clocks: pass any time in
//! seconds (use Unix time if you persist it between sessions).

use crate::id::{Id, IdMap};

/// How much visual weight an element has earned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Prominence {
    /// Rarely used: smallest footprint.
    Compact,
    #[default]
    Regular,
    /// Used all the time: large and emphasized.
    Featured,
}

#[derive(Clone, Copy, Debug, Default)]
struct Usage {
    score: f32,
    last: f64,
    count: u32,
    level: Prominence,
}

/// Per-user usage model. Cheap to query: every operation is O(1) per element.
#[derive(Clone, Debug)]
pub struct Profile {
    map: IdMap<Usage>,
    /// Seconds for an unused element's score to halve.
    pub half_life: f64,
    /// Score at which [`affinity`](Self::affinity) reaches 0.5.
    pub pivot: f32,
}

impl Default for Profile {
    fn default() -> Self {
        Profile { map: IdMap::default(), half_life: 3.0 * 24.0 * 3600.0, pivot: 3.0 }
    }
}

impl Profile {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one interaction with `id` at time `now`.
    pub fn record(&mut self, id: Id, now: f64) {
        self.record_weighted(id, now, 1.0);
    }

    pub fn record_weighted(&mut self, id: Id, now: f64, weight: f32) {
        let half_life = self.half_life;
        let u = self.map.entry(id).or_default();
        u.score = decayed(u, now, half_life) + weight;
        u.last = now;
        u.count += 1;
    }

    /// Current (decayed) score of `id`.
    pub fn score(&self, id: Id, now: f64) -> f32 {
        self.map.get(&id).map_or(0.0, |u| decayed(u, now, self.half_life))
    }

    /// Lifetime interaction count.
    pub fn count(&self, id: Id) -> u32 {
        self.map.get(&id).map_or(0, |u| u.count)
    }

    /// How much the user relies on `id`, saturating in `0..1`.
    pub fn affinity(&self, id: Id, now: f64) -> f32 {
        let s = self.score(id, now);
        s / (s + self.pivot)
    }

    /// Visual weight for `id`, with hysteresis between levels.
    pub fn prominence(&mut self, id: Id, now: f64) -> Prominence {
        let a = self.affinity(id, now);
        let Some(u) = self.map.get_mut(&id) else { return Prominence::Regular };
        u.level = match u.level {
            Prominence::Featured if a < 0.42 => Prominence::Regular,
            Prominence::Regular if a >= 0.55 => Prominence::Featured,
            Prominence::Regular if u.count >= 3 && a < 0.06 => Prominence::Compact,
            Prominence::Compact if a >= 0.14 => Prominence::Regular,
            level => level,
        };
        u.level
    }

    /// Reorders `order` in place so more-used elements come first.
    ///
    /// The existing order is the starting point and an element only overtakes
    /// its neighbour when its score is clearly higher (`margin`, e.g. `0.2` =
    /// 20 %). Stable, allocation-free, O(n) when nothing changes.
    pub fn arrange<T: Copy>(&self, order: &mut [T], now: f64, margin: f32, id: impl Fn(T) -> Id) {
        for i in 1..order.len() {
            let s = self.score(id(order[i]), now);
            let mut j = i;
            while j > 0 && s > self.score(id(order[j - 1]), now) * (1.0 + margin) + 0.5 {
                order.swap(j, j - 1);
                j -= 1;
            }
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Compact text form, for saving the profile between sessions.
    pub fn encode(&self) -> String {
        let mut out = format!("morfo1 {} {}\n", self.half_life, self.pivot);
        let mut rows: Vec<_> = self.map.iter().collect();
        rows.sort_by_key(|(id, _)| **id);
        for (id, u) in rows {
            out.push_str(&format!("{:016x} {} {} {} {}\n", id.0, u.score, u.last, u.count, u.level as u8));
        }
        out
    }

    /// Inverse of [`encode`](Self::encode). Returns `None` on malformed input.
    pub fn decode(text: &str) -> Option<Profile> {
        let mut lines = text.lines();
        let mut head = lines.next()?.split(' ');
        if head.next()? != "morfo1" {
            return None;
        }
        let mut p = Profile { half_life: head.next()?.parse().ok()?, pivot: head.next()?.parse().ok()?, ..Profile::default() };
        for line in lines.filter(|l| !l.is_empty()) {
            let mut f = line.split(' ');
            let id = Id(u64::from_str_radix(f.next()?, 16).ok()?);
            let u = Usage {
                score: f.next()?.parse().ok()?,
                last: f.next()?.parse().ok()?,
                count: f.next()?.parse().ok()?,
                level: match f.next()? {
                    "0" => Prominence::Compact,
                    "1" => Prominence::Regular,
                    "2" => Prominence::Featured,
                    _ => return None,
                },
            };
            p.map.insert(id, u);
        }
        Some(p)
    }
}

fn decayed(u: &Usage, now: f64, half_life: f64) -> f32 {
    if u.score == 0.0 {
        return 0.0;
    }
    let dt = (now - u.last).max(0.0);
    u.score * (0.5f64.powf(dt / half_life)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Id = Id::new("a");
    const B: Id = Id::new("b");
    const C: Id = Id::new("c");

    #[test]
    fn decay_halves() {
        let mut p = Profile { half_life: 10.0, ..Profile::default() };
        p.record(A, 0.0);
        assert!((p.score(A, 10.0) - 0.5).abs() < 1e-5);
        assert!(p.affinity(A, 0.0) > p.affinity(A, 100.0));
    }

    #[test]
    fn arrange_has_hysteresis() {
        let mut p = Profile::new();
        let mut order = [A, B, C];
        p.record(C, 0.0);
        p.arrange(&mut order, 0.0, 0.2, |i| i);
        assert_eq!(order, [C, A, B]);
        // B barely ahead of C is not enough to overtake it...
        p.record(B, 0.0);
        p.arrange(&mut order, 0.0, 0.2, |i| i);
        assert_eq!(order, [C, B, A]);
        // ...a clear lead is.
        p.record(B, 0.0);
        p.arrange(&mut order, 0.0, 0.2, |i| i);
        assert_eq!(order, [B, C, A]);
    }

    #[test]
    fn prominence_levels() {
        let mut p = Profile::new();
        assert_eq!(p.prominence(A, 0.0), Prominence::Regular);
        for _ in 0..6 {
            p.record(A, 0.0);
        }
        assert_eq!(p.prominence(A, 0.0), Prominence::Featured);
        // Long unused: falls back, eventually to compact.
        let later = p.half_life * 12.0;
        assert_eq!(p.prominence(A, later), Prominence::Regular);
        assert_eq!(p.prominence(A, later), Prominence::Compact);
    }

    #[test]
    fn roundtrip() {
        let mut p = Profile::new();
        p.record(A, 5.0);
        p.record(B, 7.5);
        p.record(B, 9.0);
        let q = Profile::decode(&p.encode()).unwrap();
        assert_eq!(q.count(B), 2);
        assert_eq!(q.score(A, 20.0), p.score(A, 20.0));
        assert!(Profile::decode("garbage").is_none());
    }
}
