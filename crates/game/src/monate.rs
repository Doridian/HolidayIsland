//! The game's date: the `TMonate` object at `1110:65d6` (segment `1030`),
//! which `monate_create` (1030:000d) builds with the month names from
//! `MONATE.TXT` and the 1st of January 1996. The simulation's clock moves
//! it on by a day at a time (`docs/simulation.md`).

use crate::lang::Language;

/// The first day of a new game. // 1030:000d
pub const START: Date = Date {
    day: 1,
    month: 1,
    year: 1996,
};

/// Each month's length by `year mod 4`: February has 29 days in years
/// divisible by four, with no century rule. // 1110:174c
/// (`array[0..3, 1..12] of Byte`, read through `1110:174b + 12 * (year mod
/// 4) + month`)
pub const MONTH_LENGTHS: [[u8; 12]; 4] = [
    [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31],
    [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31],
    [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31],
    [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31],
];

/// The date fields of `TMonate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Date {
    /// 1 to the month's length. // +4, read by 1030:0382
    pub day: u8,
    /// 1 to 12. // +5, read by 1030:0397
    pub month: u8,
    /// // +6
    pub year: u16,
}

impl Default for Date {
    fn default() -> Self {
        START
    }
}

impl Date {
    /// The next day. // 1030:00eb
    pub fn advance(&mut self) {
        self.day = self.day.wrapping_add(1);
        if self.day > self.month_length() {
            self.day = 1;
            self.month = self.month.wrapping_add(1);
            if self.month > 12 {
                self.year = self.year.wrapping_add(1);
                self.month = 1;
            }
        }
    }

    /// The current month's length. The original reads past the table for
    /// a month outside 1..12; the port says 31 there.
    #[must_use]
    pub fn month_length(&self) -> u8 {
        // BUG(original): a month outside 1..12 reads past the table; the port says 31.
        let row = &MONTH_LENGTHS[usize::from(self.year % 4)];
        usize::from(self.month)
            .checked_sub(1)
            .and_then(|m| row.get(m))
            .copied()
            .unwrap_or(31)
    }

    /// The date as the info bar shows it: day, suffix, month name, year,
    /// `1. Januar 1996`, `1st January 1996`. The original's suffix is by
    /// `land` (`MODATYP.DAT`): `A` and `E` `st`, `nd`, `rd`, `th`
    /// (1030:0138..1030:0141), `D` `.` (1030:0144), `F` none; the port's
    /// pattern and suffixes are `language`'s (`format.date`,
    /// `format.day_suffixes`). `months` are `MONATE.TXT`'s twelve.
    /// // 1030:0148
    #[must_use]
    pub fn text(&self, months: &[Vec<u8>], language: Language) -> Vec<u8> {
        let month = usize::from(self.month).checked_sub(1).and_then(|m| months.get(m));
        let mut s = language.format().date_text(
            self.day,
            month.map_or(&[][..], |m| &m[..]),
            self.year.to_string().as_bytes(),
        );
        s.truncate(255);
        s
    }

    /// The date that `text` is, as [`Date::text`] writes it in one of the
    /// languages (`first` tried first) with that language's months: what
    /// the news archive keeps of a message's date (`crate::news::Entry`),
    /// read back so a view can write it in its own language. None for a
    /// text no language writes for any day (`kein Datum`).
    #[must_use]
    pub fn parse(text: &[u8], first: Language) -> Option<Date> {
        // The year is one of the text's runs of digits.
        let years: Vec<u16> = text
            .split(|b| !b.is_ascii_digit())
            .filter_map(|run| std::str::from_utf8(run).ok()?.parse().ok())
            .collect();
        let languages = std::iter::once(first).chain(Language::ALL.into_iter().filter(|&l| l != first));
        for language in languages {
            for &year in &years {
                for month in 1..=12 {
                    for day in 1..=31 {
                        let date = Date { day, month, year };
                        if date.text(language.months(), language) == text {
                            return Some(date);
                        }
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A date reads back from its text in either language.
    #[test]
    fn parse() {
        let date = Date {
            day: 23,
            month: 11,
            year: 1998,
        };
        for language in Language::ALL {
            let text = date.text(language.months(), language);
            for first in Language::ALL {
                assert_eq!(Date::parse(&text, first), Some(date), "{text:?}");
            }
        }
        assert_eq!(Date::parse(b"1. Januar 1996", Language::En), Some(START));
        assert_eq!(Date::parse(b"1st January 1996", Language::De), Some(START));
        assert_eq!(Date::parse(b"kein Datum", Language::De), None);
        assert_eq!(Date::parse(b"32. Januar 1996", Language::De), None);
    }

    #[test]
    fn advance() {
        let mut d = START;
        for _ in 0..30 {
            d.advance();
        }
        assert_eq!(d, Date { day: 31, ..START });
        d.advance();
        assert_eq!((d.day, d.month), (1, 2));
        // 1996 is a leap year, 1997 isn't.
        let mut d = Date {
            day: 28,
            month: 2,
            year: 1996,
        };
        d.advance();
        assert_eq!((d.day, d.month), (29, 2));
        d.advance();
        assert_eq!((d.day, d.month), (1, 3));
        let mut d = Date {
            day: 28,
            month: 2,
            year: 1997,
        };
        d.advance();
        assert_eq!((d.day, d.month), (1, 3));
        let mut d = Date {
            day: 31,
            month: 12,
            year: 1996,
        };
        d.advance();
        assert_eq!(d, Date { year: 1997, ..START });
        // A whole leap year and a normal one.
        let mut d = START;
        let days = (0..).take_while(|_| {
            d.advance();
            d.year == 1996
        });
        assert_eq!(days.count(), 365);
    }

    #[test]
    fn text() {
        let months: Vec<Vec<u8>> = ["Januar", "Februar", "M\u{e4}rz"]
            .iter()
            .map(|m| m.as_bytes().to_vec())
            .collect();
        assert_eq!(START.text(&months, Language::De), b"1. Januar 1996");
        let d = Date {
            day: 22,
            month: 2,
            year: 1997,
        };
        assert_eq!(d.text(&months, Language::En), b"22nd Februar 1997");
        assert_eq!(Date { day: 13, ..d }.text(&months, Language::En), b"13th Februar 1997");
        assert_eq!(Date { day: 31, ..d }.text(&months, Language::En), b"31st Februar 1997");
        assert_eq!(d.text(&months, Language::De), b"22. Februar 1997");
    }
}
