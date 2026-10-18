//! The game's text output. It doesn't use Windows fonts: every string is
//! drawn glyph by glyph from `GRAFIK\FONT.BMP`, wrapped and aligned by
//! `draw_text`.
//!
//! `FONT.BMP` is a strip of 12×14 cells, 120 across. The top row of cells is
//! the mask (black glyph on white), and each further row is the glyphs in
//! one colour on black. `FONTDAT1.DAT` gives each character's cell and
//! `FONTDAT2.DAT` how far it advances the pen.

use formats::bmp::Bitmap;
use vcl::Rect;
use vcl::surface::{Rop, Surface};

/// A glyph cell in `FONT.BMP`.
pub const CELL_WIDTH: i32 = 12;
/// A glyph cell's height, which is also the line height.
pub const LINE_HEIGHT: i32 = 14;
/// `draw_text` keeps the line ends in an array of 10, the first of which is
/// the start. A tenth line would run past it, so we stop at nine.
const MAX_LINES: usize = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// One wrapped line: characters `start..end` of the string, drawn `offset`
/// pixels right of the text's left edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub start: usize,
    pub end: usize,
    pub offset: i32,
}

/// `FONT.BMP` with its two tables. Loaded by `text_init` // 1030:2b19
#[derive(Debug, Clone)]
pub struct Font {
    bitmap: Surface,
    /// `FONTDAT1.DAT`: each character's cell. // fontdat1
    cell: [u8; 256],
    /// `FONTDAT2.DAT`: each character's advance. // fontdat2
    advance: [u8; 256],
}

/// The 32 rows of 8 that `load_fontdat` reads, as one table. // 1030:2972
fn table(rows: &[[u8; 8]]) -> [u8; 256] {
    let mut t = [0; 256];
    for (to, from) in t.iter_mut().zip(rows.iter().flatten()) {
        *to = *from;
    }
    t
}

impl Font {
    /// The font from `FONT.BMP` and the rows of `FONTDAT1.DAT` and
    /// `FONTDAT2.DAT` (`formats::data::fontdat`).
    #[must_use]
    pub fn new(bitmap: &Bitmap, cells: &[[u8; 8]], advances: &[[u8; 8]]) -> Self {
        Font {
            bitmap: Surface::from_bitmap(bitmap),
            cell: table(cells),
            advance: table(advances),
        }
    }

    fn advance(&self, c: u8) -> i32 {
        i32::from(self.advance[usize::from(c)])
    }

    /// How wide `text` is on one line: its characters' advances. The port's
    /// own measure (the message boxes, [`crate::msgdlg`]); `draw_text`
    /// measures as [`Font::wrap`] says.
    #[must_use]
    pub fn width(&self, text: &[u8]) -> i32 {
        text.iter().map(|&c| self.advance(c)).sum()
    }

    /// How `draw_text` wraps `text` into `width` pixels. // 1030:23d3
    ///
    /// This keeps the original's quirks. A line broken at a space keeps the
    /// space, and the space is measured again at the start of the next line.
    /// The last character of a line isn't measured for alignment, except
    /// mostly when right-aligning. Until the string's first space, a line
    /// that doesn't fit takes the whole rest of the string. After it, a word
    /// longer than a line breaks at the space the line started with, over
    /// and over: the original then writes past its array of line ends, and
    /// we stop at nine lines.
    #[must_use]
    pub fn wrap(&self, text: &[u8], width: i32, align: Align) -> Vec<Line> {
        let n = text.len();
        if n == 0 {
            return Vec::new();
        }
        // The original counts from 1, like Pascal strings.
        let ch = |i: usize| text[i - 1];
        let w = |i: usize| self.advance(ch(i));
        let aligned = |center: i32, right: i32| match align {
            Align::Left => 0,
            Align::Center => center / 2,
            Align::Right => right,
        };
        // BUG(original): the breaking space stays on the line and is measured again on the next; the last character isn't measured for alignment; before the first space an overlong line takes the rest of the string.
        let mut ends: Vec<(usize, i32)> = Vec::new();
        if n < 2 {
            ends.push((1, aligned(width - w(1), width - w(1))));
        } else {
            let mut i = 1;
            let mut space = 1;
            let mut before_space = 0;
            let mut no_space = true;
            // BUG(original): a word longer than a line, after the first space, writes past the 10 line ends; the port stops at nine lines.
            while i < n && ends.len() < MAX_LINES {
                let mut used = 0;
                while w(i) + used <= width && i < n {
                    if ch(i) == b' ' {
                        space = i;
                        before_space = used;
                        no_space = false;
                    }
                    used += w(i);
                    i += 1;
                }
                let all_on_one_line = i >= n && ends.is_empty();
                if space < n && ch(i) != b' ' && i < n && width < w(i) + used {
                    // Back to the last space.
                    i = if no_space { n } else { space };
                    used = before_space;
                    ends.push((i, aligned(width - used, width - used)));
                } else {
                    if no_space {
                        i = n;
                    }
                    let offset = if no_space || all_on_one_line {
                        aligned(width - used - w(i), width - used - w(i))
                    } else {
                        aligned(width - used, width - used - w(i))
                    };
                    ends.push((i, offset));
                }
            }
        }
        let mut start = 0;
        ends.into_iter()
            .map(|(end, offset)| {
                let line = Line {
                    start: start.min(end),
                    end,
                    offset,
                };
                start = end;
                line
            })
            .collect()
    }

    /// `draw_text`: draws `text` into `dst` at `x`, `y` in colour row
    /// `color` (1–11), wrapped into `width` pixels. Each glyph is masked in
    /// with `SRCAND` and then its colour added with `SRCINVERT`. // 1030:23d3
    ///
    /// The original takes the `TImage` and, if asked, refreshes it after.
    /// That's up to the caller here.
    pub fn draw(&self, dst: &mut Surface, text: &[u8], (x, y): (i32, i32), width: i32, align: Align, color: i32) {
        let clip = dst.bounds();
        let mut top = y;
        for line in self.wrap(text, width, align) {
            let mut left = x + line.offset;
            for &c in &text[line.start..line.end] {
                let cell = i32::from(self.cell[usize::from(c)]) * CELL_WIDTH;
                let mask = Rect::new(cell, 0, CELL_WIDTH, LINE_HEIGHT);
                let glyph = Rect::new(cell, color * LINE_HEIGHT, CELL_WIDTH, LINE_HEIGHT);
                dst.blit(left, top, &self.bitmap, mask, Rop::And, clip);
                dst.blit(left, top, &self.bitmap, glyph, Rop::Invert, clip);
                left += self.advance(c);
            }
            top += LINE_HEIGHT;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every character 1 pixel wide but the space, which is 2.
    fn font() -> Font {
        let mut advance = [1; 256];
        advance[usize::from(b' ')] = 2;
        Font {
            bitmap: Surface::new(1, 1, 0),
            cell: [0; 256],
            advance,
        }
    }

    fn lines(text: &str, width: i32, align: Align) -> Vec<(&str, i32)> {
        font()
            .wrap(text.as_bytes(), width, align)
            .into_iter()
            .map(|l| (&text[l.start..l.end], l.offset))
            .collect()
    }

    #[test]
    fn fits() {
        assert_eq!(lines("x", 5, Align::Right), [("x", 4)]);
        assert_eq!(lines("abc", 10, Align::Left), [("abc", 0)]);
        assert_eq!(lines("abc", 10, Align::Center), [("abc", 3)]);
        assert_eq!(lines("abc", 10, Align::Right), [("abc", 7)]);
        assert_eq!(lines("", 10, Align::Right), []);
    }

    #[test]
    fn wraps_at_spaces() {
        assert_eq!(lines("ab cd ef", 6, Align::Left), [("ab cd ", 0), ("ef", 0)]);
        // Each line after the first starts by measuring the space that ended
        // the one before.
        assert_eq!(lines("ab cd ef", 5, Align::Left), [("ab ", 0), ("cd ", 0), ("ef", 0)]);
        assert_eq!(lines("ab cd ef", 5, Align::Right), [("ab ", 3), ("cd ", -1), ("ef", 1)]);
        // Before the first space nothing wraps.
        assert_eq!(lines("abcdefgh", 4, Align::Left), [("abcdefgh", 0)]);
        // After it, a long word makes empty lines until the array is full.
        let empty = lines("ab cdefgh", 4, Align::Left);
        assert_eq!(empty.len(), MAX_LINES);
        assert_eq!(empty[0], ("ab ", 0));
        assert_eq!(empty[1], ("", 0));
    }
}
