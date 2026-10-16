//! Delphi's `Random`, which the game uses for everything random: a linear
//! congruential generator on the global `RandSeed` (`1110:4a06`).

/// `RandSeed` and the functions on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Random {
    /// `RandSeed`. // 1110:4a06
    pub seed: u32,
}

impl Random {
    #[must_use]
    pub const fn new(seed: u32) -> Self {
        Random { seed }
    }

    /// Steps the seed: `RandSeed * $08088405 + 1`, which the RTL works out
    /// with 16-bit multiplies (the multiplier's low word at `1108:1e1f`).
    /// // 1108:1de9
    fn step(&mut self) -> u32 {
        self.seed = self.seed.wrapping_mul(0x0808_8405).wrapping_add(1);
        self.seed
    }

    /// `Random(range)`: `0..range`, the top 16 bits of `RandSeed * range`.
    /// `Random(0)` is 0. // 1108:1d8c
    pub fn random(&mut self, range: u16) -> u16 {
        let product = u64::from(self.step()) * u64::from(range);
        // Always below `range`.
        u16::try_from(product >> 32).unwrap_or(u16::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence() {
        let mut r = Random::new(0);
        assert_eq!(r.random(100), 0);
        assert_eq!(r.seed, 1);
        assert_eq!(r.random(100), 3);
        assert_eq!(r.seed, 0x0808_8406);
        assert_eq!(r.random(65000), 55968);
        assert_eq!(r.seed, 0xdc6d_ac1f);
        assert_eq!(r.random(0), 0);
    }
}
