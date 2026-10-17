//! The few x87 `Double` operations the hills' outlines use (10b0:375a),
//! done on integers so every target gets the same bits.
//!
//! The original works these out on the FPU (or `WIN87EM`'s emulator):
//! `FILD` of an `Integer` difference, a division (1108:055f: `FXCH`,
//! `FDIVP`), `FADD` of a `Double`, `FSTP` into a `Double` after each, and
//! `Round` (1108:0ea0: `FRNDINT`, `FISTP`). The control word is the one
//! `__FPMATH`'s initialisation (1108:0dc1) leaves: round to nearest even,
//! 64-bit precision. So each quotient and sum is rounded twice: to the
//! 64-bit mantissa of the register, then to the 53 bits of the `Double`
//! it's stored in, and `Round` rounds halves to even.
//!
//! A value here is an integer count of `2^-SCALE`. The operands are
//! differences of at most a few hundred and their quotients, and
//! rounding only coarsens, so every value the routine meets is exact at
//! that scale.

/// The fixed point's fraction bits.
const SCALE: u32 = 100;

/// A `Double` as it's stored in memory, in units of `2^-SCALE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Double(i128);

/// `x` rounded to `bits` significant bits, halves to even.
fn round_to(x: i128, bits: u32) -> i128 {
    let magnitude = x.unsigned_abs();
    let length = magnitude.bit_width();
    if length <= bits {
        return x;
    }
    let shift = length - bits;
    let mut kept = magnitude >> shift;
    let rest = magnitude & ((1 << shift) - 1);
    let half = 1 << (shift - 1);
    if rest > half || (rest == half && kept & 1 == 1) {
        kept += 1;
    }
    let rounded = i128::try_from(kept << shift).unwrap_or(i128::MAX);
    if x < 0 { -rounded } else { rounded }
}

/// What a result goes through: the register's 64 bits, then the
/// `Double`'s 53.
fn store(x: i128) -> Double {
    Double(round_to(round_to(x, 64), 53))
}

impl Double {
    /// `FILD`: exact.
    pub fn from_int(n: i32) -> Double {
        Double(i128::from(n) << SCALE)
    }

    /// `n / d` as `FDIVP` and `FSTP` leave it. `d` isn't 0 where the
    /// original divides.
    pub fn quotient(n: i32, d: i32) -> Double {
        if d == 0 {
            return Double(0);
        }
        let negative = (n < 0) != (d < 0);
        let n = u128::from(n.unsigned_abs()) << SCALE;
        let d = u128::from(d.unsigned_abs());
        // The quotient's length, then 64 bits of it rounded from the
        // remainder, scaled back.
        let whole = n / d;
        let length = whole.bit_width();
        let shift = length.saturating_sub(64);
        let divisor = d << shift;
        let mut kept = n / divisor;
        let rest = n % divisor;
        if 2 * rest > divisor || (2 * rest == divisor && kept & 1 == 1) {
            kept += 1;
        }
        let x = i128::try_from(kept << shift).unwrap_or(i128::MAX);
        let mut q = store(x);
        if negative {
            q.0 = -q.0;
        }
        q
    }

    /// `FADD` and `FSTP`.
    #[must_use]
    pub fn add(self, other: Double) -> Double {
        store(self.0 + other.0)
    }

    /// `Round`: `FRNDINT` with halves to even, `FISTP` into a `Longint`.
    pub fn round(self) -> i32 {
        let unit = 1i128 << SCALE;
        let floor = self.0.div_euclid(unit);
        let rest = self.0.rem_euclid(unit);
        let half = unit / 2;
        let n = if rest > half || (rest == half && floor & 1 == 1) {
            floor + 1
        } else {
            floor
        };
        i32::try_from(n).unwrap_or(i32::MIN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_halves_to_even() {
        let half = Double::quotient(1, 2);
        assert_eq!(Double::from_int(2).add(half).round(), 2);
        assert_eq!(Double::from_int(3).add(half).round(), 4);
        assert_eq!(Double::from_int(-3).add(half).round(), -2);
        assert_eq!(Double::from_int(-4).add(half).round(), -4);
        assert_eq!(Double::from_int(7).round(), 7);
    }

    /// An `f64` in units of `2^-SCALE`, from its bits.
    fn exact(f: f64) -> i128 {
        let bits = f.to_bits();
        let exponent = i32::try_from((bits >> 52) & 0x7ff).unwrap();
        let mantissa = i128::from((bits & ((1 << 52) - 1)) | (1 << 52));
        let shift = exponent - 1075 + i32::try_from(SCALE).unwrap();
        let magnitude = if shift >= 0 {
            mantissa << shift
        } else {
            mantissa >> -shift
        };
        if bits >> 63 == 1 { -magnitude } else { magnitude }
    }

    #[test]
    fn matches_ieee_where_one_rounding_does() {
        // Sums of thirds stepped as the outline does: the same as f64
        // here, where the double rounding changes nothing.
        let third = Double::quotient(-1, 3);
        assert_eq!(third.0, exact(-1.0 / 3.0));
        let mut x = Double::from_int(5);
        let mut f = 5.0f64;
        for _ in 0..14 {
            x = x.add(third);
            f += -1.0 / 3.0;
            assert_eq!(x.0, exact(f));
        }
        assert_eq!(Double::quotient(7, 97).0, exact(7.0 / 97.0));
        assert_eq!(Double::quotient(-45, 61).0, exact(-45.0 / 61.0));
    }

    #[test]
    fn rounds_twice() {
        // 1 + 2^-53 + 2^-65 rounds straight to 53 bits up to 1 + 2^-52;
        // the register's 64 bits drop the 2^-65 first, which leaves it
        // exactly halfway, and the store rounds that to even: 1.
        let one = 1i128 << SCALE;
        let x = one + (one >> 53) + (one >> 65);
        assert_eq!(store(x).0, one);
        assert_eq!(Double(round_to(x, 53)).0, one + (one >> 52));
    }
}
