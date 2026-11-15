//! Stable identifiers. An [`Id`] is what gives an element its identity across
//! frames (and across sessions, for the adaptive [`Profile`](crate::Profile)):
//! declare the same id somewhere else, with another shape, and it morphs there.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(pub u64);

impl Id {
    pub const ROOT: Id = Id(0x9e37_79b9_7f4a_7c15);

    /// Deterministic FNV-1a hash, usable in `const` contexts.
    pub const fn new(key: &str) -> Id {
        Id::ROOT.child(key)
    }

    /// Id of `key` inside the namespace of `self`.
    pub const fn child(self, key: &str) -> Id {
        let bytes = key.as_bytes();
        let mut h = self.0 ^ 0xcbf2_9ce4_8422_2325;
        let mut i = 0;
        while i < bytes.len() {
            h ^= bytes[i] as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
            i += 1;
        }
        Id(h)
    }

    /// Id of the `n`-th anonymous child of `self`.
    pub const fn with(self, n: u64) -> Id {
        let mut z = self.0 ^ n.wrapping_add(0x9e37_79b9_7f4a_7c15).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Id(z ^ (z >> 31))
    }
}

/// Ids are already well-distributed hashes, so the map hasher is the identity.
#[derive(Default)]
pub struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 << 8) | b as u64;
        }
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = v;
    }
}

pub type IdMap<V> = HashMap<Id, V, BuildHasherDefault<IdHasher>>;
