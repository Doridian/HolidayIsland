//! The high-score table, `SOUND\HIGHSCOR.WAV` (and the installer's
//! `HH_HIGH.WAV`): not a sound, despite the name. The game reads and writes
//! it as one block (1040:24a8, 1040:2517) into the memory at `*1110:6ee4`.
//! `SOUND\HIGHSCOR.WAV` on the CD is a cleared table; the copy in `SOUNDLO`
//! still has the developers' test entries.

use crate::pascal::PString;

/// Ranks per table.
pub const RANKS: usize = 10;
/// Tables: index 0 and the six scenarios (1 to 6, as in `1110:006e`).
pub const GAMES: usize = 7;
const RECORD: usize = 0x43;
/// The file's size.
pub const SIZE: usize = RANKS * GAMES * RECORD;

/// One entry. Resetting the table (1040:1ca7) zeroes the lengths and the
/// score but leaves the old characters behind them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// // +0x00
    pub name: PString<21>,
    /// // +0x15
    pub date: PString<21>,
    /// What the table is sorted by, highest first; 0 to 100 in the tables
    /// on the CD. // +0x2a
    pub score: i32,
    /// Text that looks like an amount of money (`5.893.223.530`). // +0x2e
    pub money_text: PString<21>,
}

/// `entries[rank][game]`, as laid out in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighScores {
    pub entries: [[Entry; GAMES]; RANKS],
}

fn pstring(b: &[u8]) -> PString<21> {
    let mut raw = [0; 21];
    raw.copy_from_slice(b);
    PString(raw)
}

impl HighScores {
    /// Parses the table. `None` if the file isn't exactly [`SIZE`] bytes
    /// (the original reads that many and doesn't check).
    #[must_use]
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() != SIZE {
            return None;
        }
        let entries = std::array::from_fn(|rank| {
            std::array::from_fn(|game| {
                let r = &data[(rank * GAMES + game) * RECORD..][..RECORD];
                Entry {
                    name: pstring(&r[0x00..0x15]),
                    date: pstring(&r[0x15..0x2a]),
                    score: i32::from_le_bytes([r[0x2a], r[0x2b], r[0x2c], r[0x2d]]),
                    money_text: pstring(&r[0x2e..0x43]),
                }
            })
        });
        Some(Self { entries })
    }

    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(SIZE);
        for e in self.entries.iter().flatten() {
            out.extend_from_slice(&e.name.0);
            out.extend_from_slice(&e.date.0);
            out.extend_from_slice(&e.score.to_le_bytes());
            out.extend_from_slice(&e.money_text.0);
        }
        out
    }
}
