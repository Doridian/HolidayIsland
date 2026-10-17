//! Builds the start menu from the original's files and paints it. Skipped
//! when `orig/` isn't there. With `STARTFORM_PPM=path` it also writes the
//! menu to `path` and the scenario page next to it (`path` with
//! `-scenario` before the extension).

use std::path::{Path, PathBuf};

use formats::bmp::Bitmap;
use formats::data::{fontdat, start_txt, szenario_txt};
use game::startform::{Action, BITMAP_FILES, HEIGHT, Settings, StartForm, WIDTH};
use game::text::Font;
use game::widgets::ShiftState;
use vcl::Palette;
use vcl::surface::Surface;

fn orig() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../orig")
}

fn read(name: &str) -> Vec<u8> {
    std::fs::read(orig().join(name)).unwrap()
}

fn bitmap(name: &str) -> Bitmap {
    Bitmap::parse(&read(&format!("GRAFIK/{}", name.to_ascii_uppercase()))).unwrap()
}

fn write_ppm(path: &Path, s: &Surface, palette: &Palette) {
    let mut ppm = format!("P6\n{} {}\n255\n", s.width, s.height).into_bytes();
    for &i in &s.pixels {
        let c = palette[usize::from(i)];
        ppm.extend_from_slice(&[c.r, c.g, c.b]);
    }
    std::fs::write(path, ppm).unwrap();
}

#[test]
fn paints_the_start_menu() {
    if !orig().join("GRAFIK/START1.BMP").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let font = Font::new(
        &bitmap("font.bmp"),
        &fontdat(&read("DATA/FONTDAT1.DAT")).unwrap(),
        &fontdat(&read("DATA/FONTDAT2.DAT")).unwrap(),
    );
    let bitmaps: Vec<Bitmap> = BITMAP_FILES.iter().map(|f| bitmap(f)).collect();
    let palette = bitmaps[0].palette;
    let mut form = StartForm::new(
        font,
        bitmaps.iter().map(Surface::from_bitmap).collect(),
        &start_txt(&read("TEXTE/START.TXT")),
        &szenario_txt(&read("TEXTE/SZENARIO.TXT")),
        Settings::default(),
    );
    form.show();
    let bild = form.bild().clone();
    assert_eq!((bild.width, bild.height), (WIDTH as usize, HEIGHT as usize));
    // The text and widgets change the background.
    let start1 = Surface::from_bitmap(&bitmaps[0]);
    let changed = (0..bild.height)
        .flat_map(|y| (0..bild.width).map(move |x| (x, y)))
        .filter(|&(x, y)| bild.pixel(x, y) != start1.pixel(x, y))
        .count();
    assert!(changed > 5000, "{changed} pixels drawn");

    // The scenario page.
    let _ = form.mouse_down(0x12 + 4, 0x132 + 4, ShiftState::LEFT);
    let _ = form.mouse_up(0x12 + 4, 0x132 + 4, true);
    assert!(form.scenario_mode());
    let _ = form.mouse_down(0xb3 + 4, 0xf0 + 4, ShiftState::LEFT);
    assert_eq!(form.settings().scenario, 4);
    let _ = form.mouse_down(0x152 + 4, 0x132 + 4, ShiftState::LEFT);
    let up = form.mouse_up(0x152 + 4, 0x132 + 4, true);
    assert_eq!(up.action, Some(Action::Start { scenario: true }));

    if let Some(out) = std::env::var_os("STARTFORM_PPM") {
        let out = PathBuf::from(out);
        write_ppm(&out, &bild, &palette);
        let mut scenario = out.file_stem().unwrap().to_owned();
        scenario.push("-scenario.ppm");
        write_ppm(&out.with_file_name(scenario), form.bild(), &palette);
    }
}
