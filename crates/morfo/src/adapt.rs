//! Usage learning: the part that makes two users see two different interfaces.
//!
//! A [`Profile`] keeps one small record per element: an interaction score that
//! decays exponentially with time (so habits fade if they change). From it the
//! interface derives *where* things go ([`Profile::arrange`]) and *how much
//! room* they deserve ([`Profile::prominence`]). Both have hysteresis, so the
//! layout only moves when a preference is clear — never back and forth.
//!
//! It also learns *when* things are used (time of day, once
//! [`Profile::time_of_day`] is set) and *what follows what*
//! ([`Profile::predict`]), and can say why it ranks something
//! ([`Profile::explain`]). The person stays in control: learning can be
//! paused ([`Profile::frozen`]), single habits forgotten
//! ([`Profile::forget`]) and the whole profile inspected
//! ([`Profile::entries`]), exported or erased.
//!
//! The profile knows nothing about rendering or clocks: pass any time in
//! seconds (Unix time if you persist it between sessions, which the time of
//! day needs anyway).

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

/// Why the profile ranks an element where it does (see [`Profile::explain`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// It usually comes right after this other element.
    AfterPrevious(Id),
    /// It is used mostly at this time of day: 0 night, 1 morning,
    /// 2 afternoon, 3 evening.
    TimeOfDay(u8),
    /// It is used often (this many times in total).
    Frequent(u32),
    /// It has been available for a while but is hardly used.
    Rare,
    /// It was used recently, without a stronger pattern yet.
    Recent,
}

/// Parts of the day, for time-of-day habits.
const BUCKETS: usize = 4;
/// Two actions closer than this (seconds) form a sequence.
const FOLLOW_WINDOW: f64 = 600.0;
/// How much the time of day weighs against overall use.
const TIME_WEIGHT: f32 = 0.5;

#[derive(Clone, Copy, Debug, Default)]
struct Usage {
    score: f32,
    last: f64,
    count: u32,
    level: Prominence,
    /// The score split by part of the day.
    by_time: [f32; BUCKETS],
}

/// "After `prev`, the person went on to `next`", decaying like scores.
#[derive(Clone, Copy, Debug)]
struct Follow {
    next: Id,
    score: f32,
    last: f64,
}

/// Per-user usage model. Cheap to query: every operation is O(1) per element.
#[derive(Clone, Debug)]
pub struct Profile {
    map: IdMap<Usage>,
    follows: IdMap<Vec<Follow>>,
    last_action: Option<(Id, f64)>,
    /// Seconds for an unused element's score to halve.
    pub half_life: f64,
    /// Score at which [`affinity`](Self::affinity) reaches 0.5.
    pub pivot: f32,
    /// Offset from UTC in seconds, when time-of-day habits are on.
    pub time_of_day: Option<f64>,
    /// While true nothing new is learned; what was learned still applies.
    pub frozen: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            map: IdMap::default(),
            follows: IdMap::default(),
            last_action: None,
            half_life: 3.0 * 24.0 * 3600.0,
            pivot: 3.0,
            time_of_day: None,
            frozen: false,
        }
    }
}

fn decay(dt: f64, half_life: f64) -> f32 {
    0.5f64.powf(dt.max(0.0) / half_life) as f32
}

impl Profile {
    pub fn new() -> Self {
        Self::default()
    }

    /// Part of the day `now` falls in, if time-of-day habits are on.
    fn bucket(&self, now: f64) -> Option<usize> {
        self.time_of_day.map(|offset| (((now + offset).rem_euclid(86_400.0)) / 21_600.0) as usize % BUCKETS)
    }

    /// Registers one interaction with `id` at time `now`.
    pub fn record(&mut self, id: Id, now: f64) {
        self.record_weighted(id, now, 1.0);
    }

    pub fn record_weighted(&mut self, id: Id, now: f64, weight: f32) {
        if self.frozen {
            return;
        }
        let (half_life, bucket) = (self.half_life, self.bucket(now));
        let u = self.map.entry(id).or_default();
        let k = decay(now - u.last, half_life);
        u.score = u.score * k + weight;
        for b in &mut u.by_time {
            *b *= k;
        }
        if let Some(b) = bucket {
            u.by_time[b] += weight;
        }
        u.last = now;
        u.count += 1;
        // Sequences: what tends to come next.
        if let Some((prev, at)) = self.last_action {
            // Uses at the same instant are one action seen twice (a button
            // inside a tile), not a sequence.
            if prev != id && now > at && now - at <= FOLLOW_WINDOW {
                let list = self.follows.entry(prev).or_default();
                match list.iter_mut().find(|f| f.next == id) {
                    Some(f) => {
                        f.score = f.score * decay(now - f.last, half_life) + weight;
                        f.last = now;
                    }
                    None => list.push(Follow { next: id, score: weight, last: now }),
                }
            }
        }
        self.last_action = Some((id, now));
    }

    /// Current (decayed) score of `id`.
    pub fn score(&self, id: Id, now: f64) -> f32 {
        self.map.get(&id).map_or(0.0, |u| u.score * decay(now - u.last, self.half_life))
    }

    /// Score adjusted for the time of day: what is usually used at this
    /// hour counts for more now. Same as [`score`](Self::score) while
    /// time-of-day habits are off.
    pub fn score_now(&self, id: Id, now: f64) -> f32 {
        let Some(u) = self.map.get(&id) else { return 0.0 };
        let k = decay(now - u.last, self.half_life);
        match self.bucket(now) {
            Some(b) => (u.score * (1.0 - TIME_WEIGHT) + TIME_WEIGHT * BUCKETS as f32 * u.by_time[b]) * k,
            None => u.score * k,
        }
    }

    /// Lifetime interaction count.
    pub fn count(&self, id: Id) -> u32 {
        self.map.get(&id).map_or(0, |u| u.count)
    }

    /// How much the user relies on `id` right now, saturating in `0..1`.
    pub fn affinity(&self, id: Id, now: f64) -> f32 {
        let s = self.score_now(id, now);
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
            let s = self.score_now(id(order[i]), now);
            let mut j = i;
            while j > 0 && s > self.score_now(id(order[j - 1]), now) * (1.0 + margin) + 0.5 {
                order.swap(j, j - 1);
                j -= 1;
            }
        }
    }

    /// What the person will probably do next, given what they just did, and
    /// how sure that is (`0..1`). `None` without a clear pattern.
    pub fn predict(&self, now: f64) -> Option<(Id, f32)> {
        let (prev, at) = self.last_action?;
        if now - at > FOLLOW_WINDOW {
            return None;
        }
        let list = self.follows.get(&prev)?;
        let scored = list.iter().map(|f| (f.next, f.score * decay(now - f.last, self.half_life)));
        let total: f32 = scored.clone().map(|(_, s)| s).sum();
        let (next, best) = scored.max_by(|a, b| a.1.total_cmp(&b.1))?;
        let share = best / total.max(1e-6);
        (best >= 2.0 && share >= 0.4).then_some((next, share))
    }

    /// The element used last, and when.
    pub fn last_action(&self) -> Option<(Id, f64)> {
        self.last_action
    }

    /// Why `id` ranks where it does. `None` if it was never used.
    pub fn explain(&self, id: Id, now: f64) -> Option<Reason> {
        let u = self.map.get(&id)?;
        if let (Some((next, _)), Some((prev, _))) = (self.predict(now), self.last_action) {
            if next == id {
                return Some(Reason::AfterPrevious(prev));
            }
        }
        if let Some(b) = self.bucket(now) {
            let total: f32 = u.by_time.iter().sum();
            if u.count >= 3 && total > 0.0 && u.by_time[b] / total >= 0.6 {
                return Some(Reason::TimeOfDay(b as u8));
            }
        }
        let a = self.affinity(id, now);
        Some(if a >= 0.5 {
            Reason::Frequent(u.count)
        } else if u.count >= 3 && a < 0.06 {
            Reason::Rare
        } else {
            Reason::Recent
        })
    }

    /// Everything learned, most-used first: `(element, score, times used)`.
    pub fn entries(&self, now: f64) -> Vec<(Id, f32, u32)> {
        let mut v: Vec<_> = self.map.keys().map(|&id| (id, self.score(id, now), self.count(id))).collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        v
    }

    /// Sum of every score: how much this person uses the interface lately.
    pub fn experience(&self, now: f64) -> f32 {
        self.map.keys().map(|&id| self.score(id, now)).sum()
    }

    /// Forgets everything learned about `id`.
    pub fn forget(&mut self, id: Id) {
        self.map.remove(&id);
        self.follows.remove(&id);
        for list in self.follows.values_mut() {
            list.retain(|f| f.next != id);
        }
        if self.last_action.is_some_and(|(a, _)| a == id) {
            self.last_action = None;
        }
    }

    /// Forgets everything (keeps the settings).
    pub fn clear(&mut self) {
        self.map.clear();
        self.follows.clear();
        self.last_action = None;
    }

    /// Compact text form, for saving the profile between sessions.
    pub fn encode(&self) -> String {
        let tod = self.time_of_day.map_or("-".to_string(), |o| o.to_string());
        let mut out = format!("morfo2 {} {} {} {}\n", self.half_life, self.pivot, tod, self.frozen as u8);
        let mut rows: Vec<_> = self.map.iter().collect();
        rows.sort_by_key(|(id, _)| **id);
        for (id, u) in rows {
            let t = u.by_time;
            out.push_str(&format!("u {:016x} {} {} {} {} {} {} {} {}\n", id.0, u.score, u.last, u.count, u.level as u8, t[0], t[1], t[2], t[3]));
        }
        let mut follows: Vec<_> = self.follows.iter().collect();
        follows.sort_by_key(|(id, _)| **id);
        for (prev, list) in follows {
            for f in list {
                out.push_str(&format!("f {:016x} {:016x} {} {}\n", prev.0, f.next.0, f.score, f.last));
            }
        }
        out
    }

    /// Inverse of [`encode`](Self::encode); also reads the older format.
    /// Returns `None` on malformed input.
    pub fn decode(text: &str) -> Option<Profile> {
        let mut lines = text.lines();
        let mut head = lines.next()?.split(' ');
        let version = head.next()?;
        if version != "morfo1" && version != "morfo2" {
            return None;
        }
        let mut p = Profile { half_life: head.next()?.parse().ok()?, pivot: head.next()?.parse().ok()?, ..Profile::default() };
        if version == "morfo2" {
            p.time_of_day = match head.next()? {
                "-" => None,
                o => Some(o.parse().ok()?),
            };
            p.frozen = head.next()? == "1";
        }
        let hex = |s: &str| u64::from_str_radix(s, 16).ok().map(Id);
        for line in lines.filter(|l| !l.is_empty()) {
            let mut f = line.split(' ');
            let kind = if version == "morfo1" { "u" } else { f.next()? };
            match kind {
                "u" => {
                    let id = hex(f.next()?)?;
                    let mut u = Usage {
                        score: f.next()?.parse().ok()?,
                        last: f.next()?.parse().ok()?,
                        count: f.next()?.parse().ok()?,
                        level: match f.next()? {
                            "0" => Prominence::Compact,
                            "1" => Prominence::Regular,
                            "2" => Prominence::Featured,
                            _ => return None,
                        },
                        by_time: [0.0; BUCKETS],
                    };
                    for b in &mut u.by_time {
                        if let Some(v) = f.next() {
                            *b = v.parse().ok()?;
                        }
                    }
                    p.map.insert(id, u);
                }
                "f" => {
                    let prev = hex(f.next()?)?;
                    let follow = Follow { next: hex(f.next()?)?, score: f.next()?.parse().ok()?, last: f.next()?.parse().ok()? };
                    p.follows.entry(prev).or_default().push(follow);
                }
                _ => return None,
            }
        }
        Some(p)
    }
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
        let mut p = Profile { time_of_day: Some(3600.0), ..Profile::default() };
        p.record(A, 5.0);
        p.record(B, 7.5);
        p.record(B, 9.0);
        let q = Profile::decode(&p.encode()).unwrap();
        assert_eq!(q.count(B), 2);
        assert_eq!(q.score(A, 20.0), p.score(A, 20.0));
        assert_eq!(q.score_now(B, 20.0), p.score_now(B, 20.0));
        assert_eq!(q.time_of_day, Some(3600.0));
        assert!(Profile::decode("garbage").is_none());
        // Profiles saved by the first version still load.
        let old = Profile::decode("morfo1 100 3\n0000000000000001 2.5 10 4 1\n").unwrap();
        assert_eq!(old.count(Id(1)), 4);
    }

    #[test]
    fn time_of_day_changes_the_order() {
        const HOUR: f64 = 3600.0;
        let mut p = Profile { time_of_day: Some(0.0), half_life: 1e9, ..Profile::default() };
        // A every morning, B every evening; overall B slightly more.
        for day in 0..5 {
            let base = day as f64 * 24.0 * HOUR;
            p.record(A, base + 8.0 * HOUR);
            p.record(B, base + 20.0 * HOUR);
            p.record(B, base + 21.0 * HOUR);
        }
        let mut morning = [B, A];
        p.arrange(&mut morning, 5.0 * 24.0 * HOUR + 9.0 * HOUR, 0.2, |i| i);
        let mut evening = [A, B];
        p.arrange(&mut evening, 5.0 * 24.0 * HOUR + 20.0 * HOUR, 0.2, |i| i);
        assert_eq!(morning, [A, B]);
        assert_eq!(evening, [B, A]);
        assert_eq!(p.explain(A, 5.0 * 24.0 * HOUR + 9.0 * HOUR), Some(Reason::TimeOfDay(1)));
    }

    #[test]
    fn sequences_are_predicted_and_explained() {
        let mut p = Profile::new();
        let mut t = 0.0;
        for _ in 0..3 {
            p.record(A, t);
            p.record(B, t + 5.0);
            t += 1000.0;
        }
        p.record(A, t);
        assert_eq!(p.predict(t + 1.0).map(|(id, _)| id), Some(B));
        assert_eq!(p.explain(B, t + 1.0), Some(Reason::AfterPrevious(A)));
        // Too long after the last action: no guess.
        assert!(p.predict(t + 5000.0).is_none());
    }

    #[test]
    fn the_person_stays_in_control() {
        let mut p = Profile::new();
        p.record(A, 0.0);
        p.record(B, 1.0);
        p.frozen = true;
        p.record(C, 2.0);
        assert_eq!(p.count(C), 0, "nothing is learned while frozen");
        p.frozen = false;
        p.forget(A);
        assert_eq!(p.count(A), 0);
        let entries = p.entries(2.0);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, B);
        assert!(p.experience(2.0) > 0.0);
    }
}
