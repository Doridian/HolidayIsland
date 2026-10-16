//! Delphi 1 text files and string conversions, as the game's loaders use them.
//!
//! The data files are read with `Read`, `Readln`, `Eof` and `StrToInt` on a
//! `Text` variable, so their exact behaviour defines the file formats. Strings
//! stay as Windows-1252 bytes.

use std::fmt;

/// Ctrl-Z, which ends a text file in Delphi 1's text mode.
const EOF_MARK: u8 = 0x1a;
const CR: u8 = b'\r';
const LF: u8 = b'\n';

/// The longest `string` (`ShortString`) Delphi 1 can hold.
pub const MAX_SHORT_STRING: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// `Read` of a number found something that isn't one (runtime error 106).
    InvalidNumericFormat { line: usize },
    /// `StrToInt` failed (`EConvertError`).
    NotAnInteger(Vec<u8>),
    /// The loader expected more lines than the file has.
    UnexpectedEof { line: usize },
    /// A section the loader searches for wasn't found.
    Missing(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidNumericFormat { line } => write!(f, "invalid number in line {line}"),
            Error::NotAnInteger(s) => write!(f, "'{}' is not a valid integer value", String::from_utf8_lossy(s)),
            Error::UnexpectedEof { line } => write!(f, "unexpected end of file in line {line}"),
            Error::Missing(what) => write!(f, "{what} not found"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// A `Text` file opened with `Reset`.
pub struct TextReader<'a> {
    data: &'a [u8],
    pos: usize,
    line: usize,
}

impl<'a> TextReader<'a> {
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0, line: 1 }
    }

    /// The 1-based line the reader is in, for error messages.
    #[must_use]
    pub fn line(&self) -> usize {
        self.line
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied().filter(|&c| c != EOF_MARK)
    }

    fn advance(&mut self) {
        if self.data[self.pos] == LF {
            self.line += 1;
        }
        self.pos += 1;
    }

    /// `Eof(f)`: at the end of the data or at a Ctrl-Z.
    #[must_use]
    pub fn eof(&self) -> bool {
        self.peek().is_none()
    }

    /// `Read(f, s)` into a `string[max]`: the rest of the line, without the
    /// line end. Characters past `max` are skipped like the RTL does.
    pub fn read_str(&mut self, max: usize) -> Vec<u8> {
        let mut s = Vec::new();
        while let Some(c) = self.peek() {
            if c == CR || c == LF {
                break;
            }
            if s.len() < max {
                s.push(c);
            }
            self.advance();
        }
        s
    }

    /// `Read(f, c)` with `c: Char`: the next character, which may be part of a
    /// line end; Ctrl-Z at the end of the file.
    pub fn read_char(&mut self) -> u8 {
        match self.peek() {
            Some(c) => {
                self.advance();
                c
            }
            None => EOF_MARK,
        }
    }

    /// `Readln(f)`: skips to the start of the next line.
    pub fn readln(&mut self) {
        while let Some(c) = self.peek() {
            self.advance();
            if c == LF {
                return;
            }
            if c == CR {
                if self.peek() == Some(LF) {
                    self.advance();
                }
                return;
            }
        }
    }

    /// `Readln(f, s)` with `s: string`.
    pub fn read_line(&mut self) -> Vec<u8> {
        self.read_line_max(MAX_SHORT_STRING)
    }

    /// `Readln(f, s)` with `s: string[max]`.
    pub fn read_line_max(&mut self, max: usize) -> Vec<u8> {
        let s = self.read_str(max);
        self.readln();
        s
    }

    /// `Read(f, n)` with `n: Longint`: skips blanks and line ends, then reads
    /// one blank-delimited word and converts it with `Val`. At the end of the
    /// file it yields 0, like the RTL.
    ///
    /// # Errors
    /// [`Error::InvalidNumericFormat`] if the word isn't a number.
    pub fn read_longint(&mut self) -> Result<i32> {
        while self.peek().is_some_and(|c| c <= b' ') {
            self.advance();
        }
        let line = self.line;
        let mut word = Vec::new();
        while let Some(c) = self.peek().filter(|&c| c > b' ') {
            word.push(c);
            self.advance();
        }
        if word.is_empty() {
            return Ok(0);
        }
        val_longint(&word).ok_or(Error::InvalidNumericFormat { line })
    }
}

/// `Val(s, n, code)` for a `Longint`: leading blanks, an optional sign, then
/// decimal digits or `$` and hex digits, and nothing after them. `None` where
/// `code` would be non-zero.
#[must_use]
pub fn val_longint(s: &[u8]) -> Option<i32> {
    let s = &s[s.iter().take_while(|&&c| c == b' ').count()..];
    let (negative, s) = match s.split_first() {
        Some((b'-', rest)) => (true, rest),
        Some((b'+', rest)) => (false, rest),
        _ => (false, s),
    };
    let (radix, digits) = match s.split_first() {
        Some((b'$', rest)) => (16, rest),
        _ => (10, s),
    };
    if digits.is_empty() {
        return None;
    }
    let mut n: u32 = 0;
    for &c in digits {
        let d = char::from(c).to_digit(radix)?;
        n = n.checked_mul(radix)?.checked_add(d)?;
    }
    // Hex literals may set the sign bit ($FFFFFFFF is -1); decimal ones may not.
    let n = if radix == 16 {
        n.cast_signed()
    } else {
        i32::try_from(n).ok()?
    };
    Some(if negative { n.wrapping_neg() } else { n })
}

/// `StrToInt(s)`.
///
/// # Errors
/// [`Error::NotAnInteger`] (the original raises `EConvertError`).
pub fn str_to_int(s: &[u8]) -> Result<i32> {
    val_longint(s).ok_or_else(|| Error::NotAnInteger(s.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn val() {
        assert_eq!(val_longint(b"   42"), Some(42));
        assert_eq!(val_longint(b"-7"), Some(-7));
        assert_eq!(val_longint(b"$ff"), Some(255));
        assert_eq!(val_longint(b"$FFFFFFFF"), Some(-1));
        assert_eq!(val_longint(b"2147483647"), Some(i32::MAX));
        assert_eq!(val_longint(b"2147483648"), None);
        assert_eq!(val_longint(b"12 "), None);
        assert_eq!(val_longint(b""), None);
        assert_eq!(val_longint(b"     "), None);
        assert_eq!(val_longint(b"-"), None);
    }

    #[test]
    fn reading() {
        let mut r = TextReader::new(b"  1   -2 name\r\n3\r\nline two\r\n\x1agarbage");
        assert_eq!(r.read_longint(), Ok(1));
        assert_eq!(r.read_longint(), Ok(-2));
        r.readln();
        assert_eq!(r.read_longint(), Ok(3));
        r.readln();
        assert!(!r.eof());
        assert_eq!(r.read_line(), b"line two");
        assert!(r.eof());
        assert_eq!(r.read_longint(), Ok(0));
        assert_eq!(r.read_line(), b"");
    }

    #[test]
    fn bad_number() {
        let mut r = TextReader::new(b"1\r\nx\r\n");
        assert_eq!(r.read_longint(), Ok(1));
        assert_eq!(r.read_longint(), Err(Error::InvalidNumericFormat { line: 2 }));
    }

    #[test]
    fn long_line() {
        let long = vec![b'a'; 300];
        let mut data = long.clone();
        data.extend_from_slice(b"\r\nb");
        let mut r = TextReader::new(&data);
        assert_eq!(r.read_line(), &long[..255]);
        assert_eq!(r.read_line(), b"b");
    }
}
