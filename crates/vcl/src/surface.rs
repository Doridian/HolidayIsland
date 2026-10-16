//! An 8-bit indexed bitmap: the screen, and each `TImage`'s picture.

use formats::bmp::Bitmap;

use crate::Rect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surface {
    pub width: usize,
    pub height: usize,
    /// Palette indices, top row first.
    pub pixels: Vec<u8>,
}

/// `n` as a coordinate; surfaces are far smaller than `i32::MAX`.
fn coord(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// A clipped, non-negative coordinate as an index.
fn index(n: i32) -> usize {
    usize::try_from(n).unwrap_or(0)
}

impl Surface {
    #[must_use]
    pub fn new(width: usize, height: usize, fill: u8) -> Self {
        Surface {
            width,
            height,
            pixels: vec![fill; width * height],
        }
    }

    /// The bitmap's pixels, as `TBitmap.LoadFromFile` would hold them. The
    /// palette is the caller's business.
    #[must_use]
    pub fn from_bitmap(b: &Bitmap) -> Self {
        Surface {
            width: b.width,
            height: b.height,
            pixels: b.pixels.clone(),
        }
    }

    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(0, 0, coord(self.width), coord(self.height))
    }

    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> u8 {
        self.pixels[y * self.width + x]
    }

    /// Fills `r`, clipped to the surface and to `clip`.
    pub fn fill_rect(&mut self, r: Rect, clip: Rect, color: u8) {
        let r = r.intersect(&clip).intersect(&self.bounds());
        if r.is_empty() {
            return;
        }
        for y in r.top..r.bottom {
            let row = index(y) * self.width;
            self.pixels[row + index(r.left)..row + index(r.right)].fill(color);
        }
    }

    /// Copies all of `src` with its top left at `x`, `y`, clipped to the
    /// surface and to `clip`, like `BitBlt` with `SRCCOPY`.
    pub fn draw(&mut self, x: i32, y: i32, src: &Surface, clip: Rect) {
        let dst = Rect::new(x, y, coord(src.width), coord(src.height))
            .intersect(&clip)
            .intersect(&self.bounds());
        if dst.is_empty() {
            return;
        }
        let w = index(dst.width());
        for dy in dst.top..dst.bottom {
            let from = index(dy - y) * src.width + index(dst.left - x);
            let to = index(dy) * self.width + index(dst.left);
            self.pixels[to..to + w].copy_from_slice(&src.pixels[from..from + w]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_clips() {
        let mut dst = Surface::new(4, 3, 0);
        let src = Surface {
            width: 2,
            height: 2,
            pixels: vec![1, 2, 3, 4],
        };
        dst.draw(-1, 2, &src, dst.bounds());
        assert_eq!(dst.pixels, [0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0]);
        dst.draw(3, 0, &src, Rect::new(0, 0, 4, 1));
        assert_eq!(&dst.pixels[..4], [0, 0, 0, 1]);
        dst.fill_rect(Rect::new(1, 1, 10, 10), Rect::new(0, 0, 3, 3), 9);
        assert_eq!(dst.pixels, [0, 0, 0, 1, 0, 9, 9, 0, 2, 9, 9, 0]);
    }
}
