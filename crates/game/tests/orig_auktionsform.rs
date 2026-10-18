//! Runs an auction with computer bidders and the human, and paints the
//! auction form from the original's files. Skipped when `orig/` isn't
//! there. With `AUKTIONSFORM_PPM=path` it also writes the form, as the
//! screen shows it, to `path`.

use std::path::{Path, PathBuf};

use formats::bmp::Bitmap;
use formats::data::{auktion, fontdat, message_media_names, modatyp, monate, sonstige};
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use game::auction::{self, Change};
use game::auktionsform::{Auktionsform, BITMAP_FILES, LAYOUT, picture_file};
use game::command::{self, Command};
use game::enhance::Enhancement;
use game::events::Event;
use game::hauptschirm::InfoTexts;
use game::text::Font;
use game::widgets::ShiftState;
use game::world::World;
use vcl::Palette;
use vcl::form::{Form, Kind};
use vcl::random::Random;
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

/// Four players with money and names, and an auction of the surfing
/// championship (item 7) at 20 000.
fn world() -> World {
    let mut w = World {
        player_count: 4,
        events: vec![Event::default(); 41],
        random: Random::new(0x1996),
        player_names: [
            "keiner", "Beginner", "Andras", "Martin", "Michael", "Moritz", "Paul", "Mark", "Peter",
        ]
        .map(|n| n.as_bytes().to_vec())
        .to_vec(),
        ..World::default()
    };
    w.options.auction_waits = false;
    w.finances.money[..4].copy_from_slice(&[100_000, 60_000, 45_000, 30_000]);
    w.finances.max_debt = 20_000;
    w
}

fn start(w: &mut World) {
    // Event 0 of kind 0 on day 1: its message and the auction.
    w.events[0] = Event {
        chance: 0x10000,
        first_day: 0,
        last_day: 10,
        value: 20_000,
        start: 7,
        kind: 0,
        ..Event::default()
    };
    let mut started = false;
    for _ in 0..30 {
        started |= w.tick().auction;
        if w.auction.open {
            break;
        }
    }
    assert!(started && w.auction.open);
}

#[test]
fn computers_and_the_human_bid() {
    // The same seed, the same auction.
    let finish = |human: bool| {
        let mut w = world();
        start(&mut w);
        let mut changes = Vec::new();
        let mut n = 0;
        while w.auction.open || w.auction.sold.is_some() {
            let tick = w.tick();
            // The human bids whenever a computer has.
            if human
                && tick
                    .auction_changes
                    .iter()
                    .any(|c| matches!(c, Change::Bid(p) if *p != 1))
            {
                let bid = Command {
                    player: 1,
                    action: command::Action::Auction(auction::Action::Bid),
                };
                changes.extend(w.apply(bid).auction_changes);
            }
            changes.extend(tick.auction_changes);
            n += 1;
            assert!(n < 100_000);
        }
        (w, changes)
    };
    let (a, changes) = finish(false);
    assert_eq!(a, finish(false).0);
    assert!(changes.iter().any(|c| matches!(c, Change::Bid(2..=4))));
    // Without the human a computer wins; with them, unless one outbids
    // their money, the human does.
    assert!((2..=4).contains(&a.auction.bidder));
    let (b, changes) = finish(true);
    assert!(changes.contains(&Change::Bid(1)));
    assert_eq!(b.auction.bidder, 1);
    assert_eq!(b.finances.money[0], 100_000 - b.auction.price);
}

/// The form on an 800 × 600 screen, through the VCL form from the DFM.
fn paint(form: &Auktionsform, palette: &Palette) -> Surface {
    let exe = read("HOLIDAY.EXE");
    let res = resources(&exe)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == ResId::Id(RT_RCDATA) && r.name == ResId::Name(b"TAUKTIONSFORM".to_vec()))
        .unwrap();
    let mut vcl = Form::load(&Object::parse(res.data).unwrap()).unwrap();
    vcl.components[0].bounds = form.bounds();
    vcl.components[0].visible = true;
    for ((name, bounds), (_, image)) in LAYOUT.iter().zip(form.images()) {
        let c = vcl.component_mut(name).unwrap();
        c.bounds = *bounds;
        c.visible = true;
        if let Kind::Image { picture } = &mut c.kind {
            *picture = Some(image.clone());
        }
    }
    let mut out = Surface::new(800, 600, 0);
    vcl.paint(&mut out, palette);
    out
}

#[test]
fn paints_the_auction_form() {
    if !orig().join("GRAFIK/AUKTBILD.BMP").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let font = Font::new(
        &bitmap("font.bmp"),
        &fontdat(&read("DATA/FONTDAT1.DAT")).unwrap(),
        &fontdat(&read("DATA/FONTDAT2.DAT")).unwrap(),
    );
    let texts = sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap();
    let info = InfoTexts {
        months: monate(&read("TEXTE/MONATE.TXT")).unwrap(),
        land: modatyp(&read("DATA/MODATYP.DAT")).unwrap().land,
        currency_factor: texts.waehrungsfaktor,
        currency: texts.texts.get(5).cloned().unwrap_or_default(),
    };
    let bitmaps: Vec<Bitmap> = BITMAP_FILES.iter().map(|f| bitmap(f)).collect();
    let palette = bitmaps[0].palette;
    let mut form = Auktionsform::new(
        bitmaps.iter().map(Surface::from_bitmap).collect(),
        auktion(&read("TEXTE/AUKTION.TXT")),
    );
    let mut w = world();
    start(&mut w);
    let names = message_media_names(&read("DATA/FILENAME.DAT"));
    let picture = Surface::from_bitmap(&bitmap(&picture_file(
        &names[usize::from(w.auction.item.unsigned_abs())],
    )));
    let screen = (800, 600);
    form.open(&w, &font, &info, Some(&picture), screen);
    assert!(form.visible);
    assert_eq!((form.left, form.top), (230, 182));
    let bare = Surface::from_bitmap(&bitmaps[2]);
    let drawn = (0..form.knopf.height)
        .flat_map(|y| (0..form.knopf.width).map(move |x| (x, y)))
        .filter(|&(x, y)| form.knopf.pixel(x, y) != bare.pixel(x, y))
        .count();
    assert!(drawn > 500, "{drawn} pixels of text");

    // The human bids with the mouse on "Mehr bieten".
    let (x, y) = (250, LAYOUT[2].1.top + 20);
    let down = form.mouse_down(x, y, ShiftState::LEFT, &w, &font, &info);
    assert_eq!(down, [auction::Action::Hold(true)]);
    let up = form.mouse_up(x, y, &w, &font);
    assert_eq!(up, [auction::Action::Bid, auction::Action::Hold(false)]);
    for action in down.into_iter().chain(up) {
        let tick = w.apply(Command {
            player: 1,
            action: command::Action::Auction(action),
        });
        form.update(&w, &tick.auction_changes, &font, &info);
    }
    assert_eq!((w.auction.bidder, w.auction.price), (1, 21_000));
    // The paddle goes up and down, then the computers have their go until
    // the first call.
    form.advance(16 * 25, &w, &font, &info);
    let paddle = paint(&form, &palette);
    while form.advance(100, &w, &font, &info) {}
    loop {
        let tick = w.tick();
        form.update(&w, &tick.auction_changes, &font, &info);
        form.draw_auctioneer(&w);
        form.advance(100, &w, &font, &info);
        if w.auction.going == 1 && w.auction.bid_pause == 0 {
            break;
        }
    }

    let out = paint(&form, &palette);
    if let Some(path) = std::env::var_os("AUKTIONSFORM_PPM") {
        write_ppm(Path::new(&path), &out, &palette);
        write_ppm(&PathBuf::from(path).with_extension("paddle.ppm"), &paddle, &palette);
    }
    // Leaving closes the form at once.
    let leave = form.key_down(0x1b, ShiftState::default()).unwrap();
    let tick = w.apply(Command {
        player: 1,
        action: command::Action::Auction(leave),
    });
    form.update(&w, &tick.auction_changes, &font, &info);
    assert!(!form.visible && !w.auction.open);
}

/// `form_positions` centres the form by its own height, and with
/// `auction_title` its title stays on its row. With `UI_PPM=dir` the
/// form opened and its title bar clicked goes there as
/// `ui-form_positions-original.ppm` and `-fixed.ppm` (on an 800 × 600
/// screen).
#[test]
fn the_auction_form_with_the_fixes() {
    if !orig().join("GRAFIK/AUKTBILD.BMP").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let font = Font::new(
        &bitmap("font.bmp"),
        &fontdat(&read("DATA/FONTDAT1.DAT")).unwrap(),
        &fontdat(&read("DATA/FONTDAT2.DAT")).unwrap(),
    );
    let texts = sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap();
    let info = InfoTexts {
        months: monate(&read("TEXTE/MONATE.TXT")).unwrap(),
        land: modatyp(&read("DATA/MODATYP.DAT")).unwrap().land,
        currency_factor: texts.waehrungsfaktor,
        currency: texts.texts.get(5).cloned().unwrap_or_default(),
    };
    let bitmaps: Vec<Bitmap> = BITMAP_FILES.iter().map(|f| bitmap(f)).collect();
    let palette = bitmaps[0].palette;
    let names = message_media_names(&read("DATA/FILENAME.DAT"));
    let mut images = Vec::new();
    for fix in [false, true] {
        let mut form = Auktionsform::new(
            bitmaps.iter().map(Surface::from_bitmap).collect(),
            auktion(&read("TEXTE/AUKTION.TXT")),
        );
        form.enhance.set(Enhancement::FormPositions, fix);
        form.enhance.set(Enhancement::AuctionTitle, fix);
        let mut w = world();
        start(&mut w);
        let picture = Surface::from_bitmap(&bitmap(&picture_file(
            &names[usize::from(w.auction.item.unsigned_abs())],
        )));
        form.open(&w, &font, &info, Some(&picture), (800, 600));
        // Centred: the original by the picture's 235, 40 pixels low.
        assert_eq!(form.top, if fix { (600 - 316) / 2 } else { (600 - 235) / 2 });
        let title = form.leiste.clone();
        // A click on the title bar draws the title again.
        let _ = form.mouse_down(100, 5, ShiftState::LEFT, &w, &font, &info);
        let _ = form.mouse_up(100, 5, &w, &font);
        assert_eq!(form.leiste == title, fix, "the title where it was");
        images.push(paint(&form, &palette));
    }
    if let Some(dir) = std::env::var_os("UI_PPM") {
        let dir = PathBuf::from(dir);
        write_ppm(&dir.join("ui-form_positions-original.ppm"), &images[0], &palette);
        write_ppm(&dir.join("ui-form_positions-fixed.ppm"), &images[1], &palette);
    }
}
