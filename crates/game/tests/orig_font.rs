//! Draws with the original font. Skipped when `orig/` isn't there.

use std::path::PathBuf;

use formats::bmp::Bitmap;
use formats::data::fontdat;
use game::text::{Align, Font, LINE_HEIGHT};
use vcl::surface::Surface;

fn read(name: &str) -> Option<Vec<u8>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../orig").join(name);
    let data = std::fs::read(&path).ok();
    if data.is_none() {
        eprintln!("skipping: {} not found", path.display());
    }
    data
}

#[test]
fn draws_text() {
    let (Some(bmp), Some(cells), Some(advances)) = (
        read("GRAFIK/FONT.BMP"),
        read("DATA/FONTDAT1.DAT"),
        read("DATA/FONTDAT2.DAT"),
    ) else {
        return;
    };
    let bitmap = Bitmap::parse(&bmp).unwrap();
    assert_eq!((bitmap.width, bitmap.height), (1448, 12 * 14));
    let font = Font::new(&bitmap, &fontdat(&cells).unwrap(), &fontdat(&advances).unwrap());

    // The money display on the main screen: right-aligned in 120 pixels.
    let lines = font.wrap(b"12.345 DM", 120, Align::Right);
    assert_eq!(lines.len(), 1);

    // Drawn on black, the text is in the colour and everything else stays
    // black.
    let mut dst = Surface::new(200, 2 * LINE_HEIGHT as usize, 0);
    font.draw(&mut dst, b"Holiday Island", (4, 1), 120, Align::Left, 10);
    let (min, max) = dst
        .pixels
        .iter()
        .enumerate()
        .filter(|(_, p)| **p != 0)
        .fold((usize::MAX, 0), |(min, max), (i, _)| {
            (min.min(i % 200), max.max(i % 200))
        });
    assert!(min >= 4 && max < 124, "text between x {min} and {max}");
    if let Some(out) = std::env::var_os("FONT_PPM") {
        let palette = bitmap.palette;
        let mut ppm = format!("P6\n200 {}\n255\n", dst.height).into_bytes();
        for &i in &dst.pixels {
            let c = palette[usize::from(i)];
            ppm.extend_from_slice(&[c.r, c.g, c.b]);
        }
        std::fs::write(out, ppm).unwrap();
    }
}
