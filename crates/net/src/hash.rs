//! The world's hash for desync checks, the same on every peer and every
//! target. The world's `Hash` (derived) feeds a [`Hasher`]; the standard
//! one's output isn't promised to stay the same between Rust releases,
//! and `usize` (the lengths of `Vec`s) is 4 bytes on wasm32 and 8 on
//! 64-bit targets. This one writes every integer as little-endian bytes
//! and `usize` as 64 bits, and mixes them `FxHash`'s way, eight bytes at a
//! time.

use std::hash::{Hash, Hasher};

use game::world::World;

/// `FxHash`'s multiplier.
const K: u64 = 0x517c_c1b7_2722_0a95;

/// A [`Hasher`] whose result depends only on what's written, not on the
/// target or the Rust release.
#[derive(Debug, Clone, Default)]
pub struct StableHasher {
    hash: u64,
    /// Bytes not mixed in yet, at most 7, and how many.
    tail: [u8; 8],
    len: usize,
}

impl StableHasher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn mix(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(K);
    }
}

impl Hasher for StableHasher {
    fn write(&mut self, mut bytes: &[u8]) {
        if self.len > 0 {
            let take = (8 - self.len).min(bytes.len());
            self.tail[self.len..self.len + take].copy_from_slice(&bytes[..take]);
            self.len += take;
            bytes = &bytes[take..];
            if self.len < 8 {
                return;
            }
            let word = u64::from_le_bytes(self.tail);
            self.mix(word);
            self.len = 0;
        }
        let (words, rest) = bytes.as_chunks::<8>();
        for &word in words {
            self.mix(u64::from_le_bytes(word));
        }
        self.tail[..rest.len()].copy_from_slice(rest);
        self.len = rest.len();
    }

    fn write_u8(&mut self, i: u8) {
        self.write(&[i]);
    }

    fn write_u16(&mut self, i: u16) {
        self.write(&i.to_le_bytes());
    }

    fn write_u32(&mut self, i: u32) {
        self.write(&i.to_le_bytes());
    }

    fn write_u64(&mut self, i: u64) {
        self.write(&i.to_le_bytes());
    }

    fn write_u128(&mut self, i: u128) {
        self.write(&i.to_le_bytes());
    }

    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }

    fn write_i8(&mut self, i: i8) {
        self.write(&i.to_le_bytes());
    }

    fn write_i16(&mut self, i: i16) {
        self.write(&i.to_le_bytes());
    }

    fn write_i32(&mut self, i: i32) {
        self.write(&i.to_le_bytes());
    }

    fn write_i64(&mut self, i: i64) {
        self.write(&i.to_le_bytes());
    }

    fn write_i128(&mut self, i: i128) {
        self.write(&i.to_le_bytes());
    }

    fn write_isize(&mut self, i: isize) {
        self.write_i64(i as i64);
    }

    fn finish(&self) -> u64 {
        let mut h = self.clone();
        if h.len > 0 {
            let mut word = [0; 8];
            word[..h.len].copy_from_slice(&h.tail[..h.len]);
            // The count of the tail's bytes too, so "a" and "a\0" differ.
            word[7] ^= u8::try_from(h.len).unwrap_or(0) << 4;
            h.mix(u64::from_le_bytes(word));
        }
        h.hash
    }
}

/// `value`'s stable hash.
#[must_use]
pub fn stable_hash<T: Hash + ?Sized>(value: &T) -> u64 {
    let mut h = StableHasher::new();
    value.hash(&mut h);
    h.finish()
}

/// The world's hash, which the peers compare.
#[must_use]
pub fn world_hash(world: &World) -> u64 {
    stable_hash(world)
}

/// The hash of what a world takes from the game's files rather than from
/// a saved game: the tables and the folder's language. A client whose
/// game files aren't the host's (another release) can't follow the
/// host's world.
#[must_use]
pub fn data_hash(world: &World) -> u64 {
    let mut h = StableHasher::new();
    world.tables.hash(&mut h);
    world.folder_language.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_bytes_split_anyhow_hash_the_same() {
        let data: Vec<u8> = (0..=255).collect();
        let whole = {
            let mut h = StableHasher::new();
            h.write(&data);
            h.finish()
        };
        for cut in [1, 3, 7, 8, 9, 100] {
            let mut h = StableHasher::new();
            for c in data.chunks(cut) {
                h.write(c);
            }
            assert_eq!(h.finish(), whole, "{cut}");
        }
        assert_ne!(stable_hash(&[1u8][..]), stable_hash(&[1u8, 0][..]));
        // Pinned: the hash must not change between releases or targets.
        assert_eq!(stable_hash(&(1u16, -2i32, vec![3u8, 4], "x")), 0xa41a_90e8_0a69_6466);
    }
}
