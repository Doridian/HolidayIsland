//! The players' money: `1110:6f30`, `array[1..8] of Longint`, with each
//! player's loan and its interest next to it (segment `1078`), and how the
//! info bar shows the amount (segment `1030`).

use formats::data::Land;

use crate::world::World;

/// The money side of the players. Player `n` is index `n - 1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Finances {
    /// What each player has, in the game's internal unit (the info bar
    /// shows it times `Waehrungsfaktor`). // 1110:6f30
    pub money: [i32; 8],
    /// Each player's loan. // 1110:6f50
    pub loan: [i32; 8],
    /// The monthly interest on it. // 1110:6f70
    pub interest: [i32; 8],
    /// `MaxSchulden`: how far a player may go into debt. // 1110:64ce
    pub max_debt: i32,
    /// `Zinssatz`: the yearly interest rate in percent. // 1110:64d2
    pub interest_rate: i32,
    /// `Startgeld`. // 1110:64d6
    pub start_money: i32,
    /// Freezes everyone's money: `add_money` does nothing while it's set.
    /// Only `init_game_defaults` writes it (false). // 1110:0070
    pub frozen: bool,
}

impl World {
    /// Adds `amount` to `player`'s money (1 to the number of players),
    /// saturating at the `Longint` limits. A computer player paying gets
    /// some of it back on the harder levels: a sixth at difficulty 1, a
    /// third at 2. // 1078:01e7
    ///
    /// For player 1 the original redraws the money on the info bar; the
    /// port's view does that when it sees the amount change.
    pub fn add_money(&mut self, player: u8, amount: i32) {
        let f = &mut self.finances;
        if f.frozen || player == 0 || player > self.player_count {
            return;
        }
        let i = usize::from(player - 1);
        f.money[i] = f.money[i].saturating_add(amount);
        if player > 1 && amount < 0 {
            let back = match self.difficulty {
                1 => amount / 6,
                2 => amount / 3,
                _ => 0,
            };
            f.money[i] = f.money[i].wrapping_sub(back);
        }
    }

    /// Gives `player` a loan of `amount` and works out its monthly
    /// interest at the current rate. // 1078:0466
    pub fn set_loan(&mut self, player: u8, amount: i32) {
        let f = &mut self.finances;
        let Some(i) = usize::from(player).checked_sub(1).filter(|&i| i < 8) else {
            return;
        };
        f.loan[i] = amount;
        f.interest[i] = (amount / 24).wrapping_mul(f.interest_rate) / 50;
    }
}

/// `amount` as the game shows money: times the currency factor
/// (`Waehrungsfaktor`, `SONSTIGE.TXT`), with a `-` in front of negative
/// amounts and the digits grouped by threes for the `land`. Amounts whose
/// product wouldn't fit a `Longint` are worked out in two halves.
/// // 1030:11a0
#[must_use]
pub fn format_money(amount: i32, factor: i32, land: Land) -> Vec<u8> {
    let sign: &[u8] = if amount < 0 { b"-" } else { b"" }; // 1030:119c
    let n = amount.wrapping_abs();
    let digits = if factor != 0 && i32::MAX / factor > n {
        n.wrapping_mul(factor).to_string()
    } else {
        // The product's last two digits, and the rest. `factor` 0 would
        // divide by zero in the original.
        let low = (n % 100).wrapping_mul(factor);
        let high = (low / 100).wrapping_add((n / 100).wrapping_mul(factor));
        let low = low % 100;
        if high == 0 {
            low.to_string()
        } else if low < 10 {
            format!("{high}0{low}") // 1030:119e
        } else {
            format!("{high}{low}")
        }
    };
    let mut s = sign.to_vec();
    s.extend_from_slice(digits.as_bytes());
    group_digits(&s, land)
}

/// Puts a separator before every third digit from the right: `.` in
/// Germany, `_` in France, `,` otherwise. A leading `-` doesn't count as
/// a digit. // 1030:0fdd
#[must_use]
pub fn group_digits(s: &[u8], land: Land) -> Vec<u8> {
    let separator = match land {
        Land::Deutschland => b'.',
        Land::Frankreich => b'_',
        Land::Amerika | Land::England => b',',
    };
    let first = usize::from(s.first() == Some(&b'-'));
    let mut out = s.to_vec();
    // Pascal positions: the separator goes in before `i`, for `i` = length
    // - 2, length - 5, ... down to the third digit.
    let mut i = s.len().wrapping_sub(2);
    while i >= first + 2 && i <= s.len() {
        out.insert(i - 1, separator);
        i = i.wrapping_sub(3);
    }
    out.truncate(255);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_money() {
        let mut w = World {
            player_count: 3,
            difficulty: 2,
            ..World::default()
        };
        w.finances.money = [100, 100, 100, 0, 0, 0, 0, 0];
        w.add_money(1, -50);
        assert_eq!(w.finances.money[0], 50);
        // Computers pay a third less on hard.
        w.add_money(2, -60);
        assert_eq!(w.finances.money[1], 60);
        w.add_money(2, 60);
        assert_eq!(w.finances.money[1], 120);
        w.difficulty = 1;
        w.add_money(3, -60);
        assert_eq!(w.finances.money[2], 50);
        // Not a player.
        w.add_money(4, 10);
        w.add_money(0, 10);
        assert_eq!(w.finances.money[3], 0);
        // Saturation.
        w.finances.money[0] = i32::MAX - 5;
        w.add_money(1, 10);
        assert_eq!(w.finances.money[0], i32::MAX);
        w.finances.money[0] = i32::MIN + 5;
        w.add_money(1, -10);
        assert_eq!(w.finances.money[0], i32::MIN);
        w.finances.frozen = true;
        w.add_money(1, 10);
        assert_eq!(w.finances.money[0], i32::MIN);
    }

    #[test]
    fn set_loan() {
        let mut w = World::default();
        w.finances.interest_rate = 6;
        w.set_loan(1, 100_000);
        assert_eq!(w.finances.loan[0], 100_000);
        assert_eq!(w.finances.interest[0], 4166 * 6 / 50);
    }

    #[test]
    fn format() {
        let de = Land::Deutschland;
        assert_eq!(format_money(0, 10, de), b"0");
        assert_eq!(format_money(12, 10, de), b"120");
        assert_eq!(format_money(1234, 10, de), b"12.340");
        assert_eq!(format_money(-123_456, 10, de), b"-1.234.560");
        assert_eq!(format_money(-12_345, 10, de), b"-123.450");
        assert_eq!(format_money(1234, 10, Land::England), b"12,340");
        assert_eq!(format_money(1234, 10, Land::Frankreich), b"12_340");
        // Too big for a Longint once multiplied.
        assert_eq!(format_money(i32::MAX, 10, de), b"21.474.836.470");
        assert_eq!(format_money(3_000_000, 1000, de), b"3.000.000.000");
        assert_eq!(format_money(3_000_001, 1000, de), b"3.000.001.000");
    }

    #[test]
    fn grouping() {
        let de = Land::Deutschland;
        assert_eq!(group_digits(b"", de), b"");
        assert_eq!(group_digits(b"1", de), b"1");
        assert_eq!(group_digits(b"123", de), b"123");
        assert_eq!(group_digits(b"1234", de), b"1.234");
        assert_eq!(group_digits(b"-123", de), b"-123");
        assert_eq!(group_digits(b"-1234", de), b"-1.234");
        assert_eq!(group_digits(b"1234567", de), b"1.234.567");
    }
}
