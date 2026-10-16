//! `TColor`: `$00BBGGRR`, or `$80000000` plus a `COLOR_*` index for the
//! system colours.

use crate::{Palette, Rgb};

/// A Delphi `TColor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u32);

const SYSTEM: u32 = 0x8000_0000;

/// The standard colour names and their values, as in `Graphics`.
const NAMES: [(&[u8], u32); 24] = [
    (b"clBlack", 0x00_0000),
    (b"clMaroon", 0x00_0080),
    (b"clGreen", 0x00_8000),
    (b"clOlive", 0x00_8080),
    (b"clNavy", 0x80_0000),
    (b"clPurple", 0x80_0080),
    (b"clTeal", 0x80_8000),
    (b"clGray", 0x80_8080),
    (b"clSilver", 0xc0_c0c0),
    (b"clRed", 0x00_00ff),
    (b"clLime", 0x00_ff00),
    (b"clYellow", 0x00_ffff),
    (b"clBlue", 0xff_0000),
    (b"clFuchsia", 0xff_00ff),
    (b"clAqua", 0xff_ff00),
    (b"clWhite", 0xff_ffff),
    (b"clWindow", SYSTEM | 0x05),
    (b"clWindowText", SYSTEM | 0x08),
    (b"clBtnFace", SYSTEM | 0x0f),
    (b"clBtnShadow", SYSTEM | 0x10),
    (b"clGrayText", SYSTEM | 0x11),
    (b"clBtnText", SYSTEM | 0x12),
    (b"clBtnHighlight", SYSTEM | 0x14),
    (b"clHighlight", SYSTEM | 0x0d),
];

/// Windows 3.1's default system colours by `COLOR_*` index; the port has no
/// user colour scheme.
fn system_rgb(index: u32) -> u32 {
    match index {
        5 | 20 => 0xff_ffff,
        13 => 0x80_0000,
        15 => 0xc0_c0c0,
        16 | 17 => 0x80_8080,
        _ => 0,
    }
}

impl Color {
    pub const BLACK: Color = Color(0);
    pub const WHITE: Color = Color(0xff_ffff);
    pub const BTN_FACE: Color = Color(SYSTEM | 0x0f);
    pub const BTN_SHADOW: Color = Color(SYSTEM | 0x10);
    pub const BTN_HIGHLIGHT: Color = Color(SYSTEM | 0x14);

    /// The colour for a DFM identifier such as `clSilver`.
    #[must_use]
    pub fn from_ident(name: &[u8]) -> Option<Color> {
        NAMES.iter().find(|(n, _)| *n == name).map(|&(_, v)| Color(v))
    }

    /// The colour as RGB, with system colours resolved.
    #[must_use]
    pub fn rgb(self) -> Rgb {
        let v = if self.0 & SYSTEM != 0 {
            system_rgb(self.0 & 0xff)
        } else {
            self.0
        };
        let [r, g, b, _] = v.to_le_bytes();
        Rgb { r, g, b }
    }

    /// The palette index GDI draws the colour with on a 256-colour screen:
    /// the nearest palette entry, like `GetNearestPaletteIndex`.
    #[must_use]
    pub fn index(self, palette: &Palette) -> u8 {
        nearest(palette, self.rgb())
    }
}

/// The index of the palette entry closest to `c` (squared RGB distance; the
/// first one on a tie).
#[must_use]
pub fn nearest(palette: &Palette, c: Rgb) -> u8 {
    let dist = |p: &Rgb| {
        let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
        d(p.r, c.r) + d(p.g, c.g) + d(p.b, c.b)
    };
    let mut best = (0, dist(&palette[0]));
    for (i, p) in (0..=u8::MAX).zip(palette) {
        if dist(p) < best.1 {
            best = (i, dist(p));
        }
    }
    best.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors() {
        assert_eq!(
            Color::from_ident(b"clSilver").unwrap().rgb(),
            Rgb {
                r: 0xc0,
                g: 0xc0,
                b: 0xc0
            }
        );
        assert_eq!(Color::from_ident(b"clRed").unwrap().rgb(), Rgb { r: 0xff, g: 0, b: 0 });
        assert_eq!(Color::from_ident(b"clBtnFace").unwrap().rgb(), Color(0xc0_c0c0).rgb());
        assert_eq!(Color::from_ident(b"clNope"), None);
        let mut palette = [Rgb::default(); 256];
        palette[7] = Rgb {
            r: 0xc0,
            g: 0xc0,
            b: 0xb0,
        };
        palette[9] = Rgb {
            r: 0xc0,
            g: 0xc0,
            b: 0xb0,
        };
        assert_eq!(Color::BTN_FACE.index(&palette), 7);
        assert_eq!(Color::BLACK.index(&palette), 0);
    }
}
