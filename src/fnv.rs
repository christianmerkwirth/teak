//! A fast hasher for the hash maps on hot paths (node ids, inode numbers,
//! name bytes). The std hasher resists hash flooding, which teak does not
//! need for keys it makes itself.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// FNV-1a, 64 bit.
pub struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
}

impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn write_u64(&mut self, n: u64) {
        // One round per word: much faster than byte by byte, and good enough
        // for ids and inode numbers.
        self.0 = (self.0 ^ n).wrapping_mul(0x0100_0000_01b3).rotate_left(29);
    }

    fn write_u32(&mut self, n: u32) {
        self.write_u64(u64::from(n));
    }

    fn write_usize(&mut self, n: usize) {
        self.write_u64(n as u64);
    }
}

pub type FnvMap<K, V> = HashMap<K, V, BuildHasherDefault<Fnv>>;
pub type FnvSet<K> = HashSet<K, BuildHasherDefault<Fnv>>;
