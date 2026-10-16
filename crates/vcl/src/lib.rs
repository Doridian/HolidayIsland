//! The subset of Delphi 1's VCL that Holiday Island uses, drawn into an 8-bit
//! indexed framebuffer like the original's 256-colour screen.
//!
//! The forms only use `TForm`, `TImage`, `TPanel`, `TTimer` and
//! `TMediaPlayer`. The game draws almost everything itself onto the images'
//! bitmaps, so this crate is about putting those in the right place.

pub mod color;
pub mod form;
pub mod random;
pub mod surface;

pub use formats::bmp::Rgb;

/// A 256-colour palette.
pub type Palette = [Rgb; 256];

/// `TRect`: right and bottom are exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    #[must_use]
    pub const fn new(left: i32, top: i32, width: i32, height: i32) -> Self {
        Rect {
            left,
            top,
            right: left + width,
            bottom: top + height,
        }
    }

    #[must_use]
    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    #[must_use]
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    #[must_use]
    pub fn offset(&self, dx: i32, dy: i32) -> Self {
        Rect {
            left: self.left + dx,
            top: self.top + dy,
            right: self.right + dx,
            bottom: self.bottom + dy,
        }
    }

    #[must_use]
    pub fn intersect(&self, other: &Rect) -> Rect {
        Rect {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }
}
