//! The news archive and the TV over a CD scenario: the news a few months
//! bring fill the archive, which moves up after 26; the archive and the
//! TV's fields go through a saved game; the TV pauses the game while it's
//! open. Skipped when `orig/` isn't there. With `FERNSEHER_PPM=path` it
//! also writes the TV showing a message, as the screen shows it, to
//! `path`, and the archive browsed back two pages to `path` with
//! `.archive.ppm`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use formats::bmp::Bitmap;
use formats::data::{
    animdat1, animdat4, darstell, fontdat, groundnr, hausdata, hausnr, hausprei, message_media_names, modatyp, monate,
    nachrich, sonstige,
};
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use formats::savegame::SaveGame;
use game::auction;
use game::auktionsform::picture_file;
use game::command::{self, Action, Command};
use game::fernseher::{BITMAP_FILES, Fernseher, LAYOUT, Response};
use game::hauptschirm::InfoTexts;
use game::hintergrund::load_world;
use game::news::{ARCHIVE_ENTRIES, BUILDING_SALE_NEWS, Entry, Message, NO_DAY, Posted};
use game::savegame::MapView;
use game::simulation::Tick;
use game::startform::Settings;
use game::text::Font;
use game::widgets::ShiftState;
use game::world::{Tables, World};
use vcl::Palette;
use vcl::form::{Form, Kind};
use vcl::random::Random;
use vcl::surface::Surface;

fn orig() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../orig")
}

/// Reads `name` from `orig/`, whatever the case of its letters.
fn find(name: &str) -> Option<PathBuf> {
    let mut path = orig();
    for part in name.split('/') {
        let found = std::fs::read_dir(&path)
            .ok()?
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))?;
        path = found.path();
    }
    Some(path)
}

fn read(name: &str) -> Vec<u8> {
    std::fs::read(find(name).unwrap_or_else(|| panic!("{name} not found"))).unwrap()
}

fn bitmap(name: &str) -> Bitmap {
    Bitmap::parse(&read(&format!("GRAFIK/{name}"))).unwrap()
}

fn tables() -> Arc<Tables> {
    Arc::new(Tables {
        hausnr: hausnr(&read("DATA/HAUSNR.DAT")).unwrap(),
        hausdata: hausdata(&read("DATA/HAUSDATA.DAT")).unwrap(),
        hausprei: hausprei(&read("DATA/HAUSPREI.DAT")).unwrap(),
        groundnr: groundnr(&read("DATA/GROUNDNR.DAT")).unwrap(),
        animdat1: animdat1(&read("DATA/ANIMDAT1.DAT")).unwrap(),
        animdat4: animdat4(&read("DATA/ANIMDAT4.DAT")).unwrap(),
        form_korrektur: darstell(&read("DATA/DARSTELL.DAT")).unwrap().form_korrektur,
        road_cost: modatyp(&read("DATA/MODATYP.DAT")).unwrap().strassenbaukosten,
        sabotage: Vec::new(),
    })
}

fn info() -> InfoTexts {
    let texts = sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap();
    InfoTexts {
        months: monate(&read("TEXTE/MONATE.TXT")).unwrap(),
        land: modatyp(&read("DATA/MODATYP.DAT")).unwrap().land,
        currency_factor: texts.waehrungsfaktor,
        currency: texts.texts.get(5).cloned().unwrap_or_default(),
    }
}

/// The fourth scenario, `Luxus.his`, ready to run.
fn scenario() -> World {
    let map = SaveGame::parse(&read("SZENARIO/Luxus.his")).unwrap();
    let settings = Settings {
        scenario: 4,
        difficulty: 1,
        ..Settings::default()
    };
    let mut world = load_world(&map, &settings, true).unwrap();
    world.tables = tables();
    world.random = Random::new(0x1996_0101);
    let info = info();
    world.news.date_format = game::news::DateFormat {
        months: info.months.clone(),
        land: info.land as u8,
    };
    world.clock.paused = false;
    world
}

fn make_tv(screen: (i32, i32)) -> (Fernseher, Palette) {
    let font = Font::new(
        &bitmap("FONT.BMP"),
        &fontdat(&read("DATA/FONTDAT1.DAT")).unwrap(),
        &fontdat(&read("DATA/FONTDAT2.DAT")).unwrap(),
    );
    let bitmaps: Vec<Bitmap> = BITMAP_FILES.iter().map(|f| bitmap(&f.to_ascii_uppercase())).collect();
    let palette = bitmaps[0].palette;
    let pictures = message_media_names(&read("DATA/FILENAME.DAT"))
        .iter()
        .map(|name| {
            find(&format!("GRAFIK/{}", picture_file(name)))
                .map(|p| Surface::from_bitmap(&Bitmap::parse(&std::fs::read(p).unwrap()).unwrap()))
        })
        .collect();
    let tv = Fernseher::new(
        font,
        bitmaps.iter().map(Surface::from_bitmap).collect(),
        pictures,
        nachrich(&read("TEXTE/NACHRICH.TXT")),
        message_media_names(&read("DATA/FILMNAME.DAT")),
        screen,
    );
    (tv, palette)
}

fn apply(world: &mut World, r: &Response) {
    for a in &r.actions {
        world.apply(Command {
            player: 1,
            action: a.clone(),
        });
    }
}

/// A tick at `now` ms, with a player who leaves every auction as soon as
/// its form opens and lets the TV close by itself.
fn step(world: &mut World, tv: &mut Fernseher, info: &InfoTexts, now: u32) -> Tick {
    let mut tick = world.tick();
    if world.auction.open {
        let left = world.apply(Command {
            player: 1,
            action: command::Action::Auction(auction::Action::Leave),
        });
        tick.messages.extend(left.messages);
        tick.news.extend(left.news);
    }
    let r = tv.news(&tick.news, [true; 4], world, info, now);
    apply(world, &r);
    if tv.visible {
        let r = tv.timer(world, now);
        apply(world, &r);
    }
    tick
}

/// The archive the posted news make, by the rule of 1048:0a54.
fn expected_archive(posted: &[Posted]) -> Vec<Option<Entry>> {
    let mut a: Vec<Option<Entry>> = vec![None; ARCHIVE_ENTRIES];
    for p in posted {
        let n = a
            .iter()
            .take(ARCHIVE_ENTRIES - 1)
            .position(Option::is_none)
            .unwrap_or(ARCHIVE_ENTRIES - 1);
        if n == ARCHIVE_ENTRIES - 1 {
            a.remove(0);
            let last = a[ARCHIVE_ENTRIES - 2].clone();
            a.push(last);
        }
        assert_eq!(usize::try_from(p.index).unwrap(), n);
        a[n] = Some(p.entry.clone());
    }
    a
}

#[test]
fn news_fill_the_archive() {
    if find("SZENARIO/Luxus.his").is_none() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let info = info();
    let mut world = scenario();
    let (mut tv, _) = make_tv((800, 600));
    // The scenario's file starts with an empty archive.
    assert!(world.news.entries.iter().all(Entry::is_empty));
    let mut posted = Vec::new();
    let (mut now, mut opened, mut paused_ticks) = (0u32, 0, 0);
    let start = world.days;
    let squares: Vec<_> = world.facilities.iter().map(game::world::Facility::pos).collect();
    while posted.len() < 30 && world.days < start + 365 {
        let (date, paused) = (world.date, world.clock.paused);
        let mut tick = step(&mut world, &mut tv, &info, now);
        // The scenario's own news are few: one more every third day, on
        // its facilities.
        if tick.new_day && world.days % 3 == 0 {
            let k = posted.len();
            let mut more = Tick::default();
            world.post_message(
                Message {
                    number: [0, 9, 10, 12, 21, 27, 37, 40][k % 8],
                    a: 1,
                    pos: Some(squares[k % squares.len()]),
                    centre: k % 2 == 0,
                    ..Message::default()
                },
                &mut more,
            );
            let r = tv.news(&more.news, [true; 4], &world, &info, now);
            apply(&mut world, &r);
            tick.messages.extend(more.messages);
            tick.news.extend(more.news);
        }
        assert_eq!(tick.news.len(), tick.messages.len());
        for (p, m) in tick.news.iter().zip(&tick.messages) {
            assert_eq!((p.entry.number, p.entry.text), (m.number, m.text));
            assert_eq!(p.entry.pos, m.pos.unwrap_or(p.entry.origin));
            opened += usize::from(p.opens_tv([true; 4]));
        }
        posted.extend(tick.news);
        if paused {
            paused_ticks += 1;
            assert_eq!(world.date, date, "the date stands while the TV is open");
        }
        now += 100;
    }
    assert!(posted.len() >= 27, "{} messages", posted.len());
    // The TV opened for them and stood the game for up to 6 s each (less
    // where the next opened it again).
    assert!(
        opened > 0 && paused_ticks > 50 * opened && paused_ticks <= 60 * opened,
        "{opened} TVs, {paused_ticks} ticks"
    );
    let expected = expected_archive(&posted);
    let got: Vec<Option<Entry>> = world
        .news
        .entries
        .iter()
        .map(|e| (!e.is_empty()).then(|| e.clone()))
        .collect();
    assert_eq!(got, expected);
    // The oldest are gone; the 26th and 27th are the wrong way round.
    let days: Vec<i32> = world.news.entries.iter().map(|e| e.day).collect();
    assert_ne!(days[0], NO_DAY);
    let first = &posted[0].entry;
    let months = &world.news.date_format.months;
    assert!(
        months.iter().any(|m| first.date.windows(m.len()).any(|w| w == &m[..])),
        "a month's name in the date"
    );
    assert_eq!(world.news.last, posted.last().unwrap().index);
    assert_eq!(tv.cursor, world.news.last);
}

#[test]
fn the_archive_saves_and_loads() {
    if find("SZENARIO/Luxus.his").is_none() {
        return;
    }
    let mut world = scenario();
    let mut tick = Tick::default();
    for k in 0..30 {
        world.days = k;
        world.post_message(
            Message {
                number: 9,
                a: 2,
                x: 1000 * i32::from(u8::try_from(k).unwrap()),
                pos: Some((50, 60)),
                ..Message::default()
            },
            &mut tick,
        );
    }
    let (mut tv, _) = make_tv((800, 600));
    tv.cursor = 7;
    tv.left = 33;
    tv.top = 44;
    tv.centred = true;
    let mut view = MapView {
        news: [true, false, true, false],
        ..MapView::default()
    };
    tv.store_fields(&mut view.unported);
    let file = SaveGame::parse(&world.to_map(&view).to_bytes()).unwrap();
    let loaded = World::from_map(&file).unwrap();
    assert_eq!(loaded.news.entries, world.news.entries);
    assert_eq!(loaded.news.last, world.news.last);
    assert!(!loaded.news.nothing_yet);
    assert_eq!(MapView::from_map(&file).news, [true, false, true, false]);
    let (mut other, _) = make_tv((800, 600));
    other.load_fields(&MapView::from_map(&file).unported);
    assert_eq!((other.cursor, other.left, other.top, other.centred), (7, 33, 44, true));
    // A new game's archive is empty; so is the scenario's.
    assert!(World::default().news.nothing_yet);
}

#[test]
fn the_tv_pauses_the_game() {
    if find("SZENARIO/Luxus.his").is_none() {
        return;
    }
    let info = info();
    let mut world = scenario();
    let (mut tv, _) = make_tv((800, 600));
    // Nothing posted yet: the button does nothing.
    let r = tv.show(&world, &info, 0);
    assert!(r.hidden && r.actions.is_empty() && !tv.visible);
    // The announcement of a building's sale on a facility: the TV puts
    // its signs up.
    let facility = world.facilities[0].pos();
    let mut tick = Tick::default();
    world.post_message(
        Message {
            number: BUILDING_SALE_NEWS,
            pos: Some(facility),
            ..Message::default()
        },
        &mut tick,
    );
    assert!(!tick.news[0].opens_tv([false; 4]));
    let signs = |w: &World| w.people.records.iter().filter(|p| p.kind() == 0x46).count();
    let before = signs(&world);
    tv.cursor = 0;
    let r = tv.show(&world, &info, 0);
    assert_eq!(r.actions, [Action::ShowTv(0)]);
    apply(&mut world, &r);
    assert!(world.clock.paused);
    assert!(signs(&world) > before, "the building's signs");
    assert_eq!(tv.left, 0, "the TV moves to the left edge");
    let date = world.date;
    for _ in 0..1000 {
        world.tick();
    }
    assert_eq!(world.date, date);
    // Escape switches it off and the game goes on.
    let r = tv.key_down(0x1b, ShiftState::default(), &world, &info);
    assert_eq!(r.actions, [Action::TvPause(false)]);
    apply(&mut world, &r);
    for _ in 0..1000 {
        world.tick();
    }
    assert_ne!(world.date, date);
}

fn write_ppm(path: &Path, s: &Surface, palette: &Palette) {
    let mut ppm = format!("P6\n{} {}\n255\n", s.width, s.height).into_bytes();
    for &i in &s.pixels {
        let c = palette[usize::from(i)];
        ppm.extend_from_slice(&[c.r, c.g, c.b]);
    }
    std::fs::write(path, ppm).unwrap();
}

/// The TV on an 800 × 600 screen, through the VCL form from the DFM.
fn paint(tv: &Fernseher, palette: &Palette) -> Surface {
    let exe = read("HOLIDAY.EXE");
    let res = resources(&exe)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == ResId::Id(RT_RCDATA) && r.name == ResId::Name(b"TFERNSEHER".to_vec()))
        .unwrap();
    let mut vcl = Form::load(&Object::parse(res.data).unwrap()).unwrap();
    vcl.components[0].bounds = tv.bounds();
    vcl.components[0].visible = true;
    for ((name, bounds), (_, image)) in LAYOUT.iter().zip(tv.images()) {
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
fn paints_the_tv() {
    if find("GRAFIK/TVBILD.BMP").is_none() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let info = info();
    let mut world = scenario();
    let (mut tv, palette) = make_tv((800, 600));
    // Two of the news first, then a few months until the scenario's own
    // news open the TV.
    let mut more = Tick::default();
    for number in [23, 21] {
        world.post_message(
            Message {
                number,
                pos: Some(world.facilities[0].pos()),
                ..Message::default()
            },
            &mut more,
        );
    }
    let mut now = 0;
    let mut shown = None;
    for _ in 0..200_000 {
        let tick = world.tick();
        if world.auction.open {
            world.apply(Command {
                player: 1,
                action: command::Action::Auction(auction::Action::Leave),
            });
        }
        let r = tv.news(&tick.news, [true; 4], &world, &info, now);
        apply(&mut world, &r);
        if r.shown {
            shown = Some(tv.cursor);
            break;
        }
        now += 100;
    }
    let n = shown.expect("a message on the TV");
    // The lights, once.
    let _ = tv.timer(&world, now + 100);
    let out = paint(&tv, &palette);
    let bare = Surface::from_bitmap(&bitmap("TVBILD.BMP"));
    let (_, image) = tv.images()[1];
    let drawn = (0..image.height)
        .flat_map(|y| (0..image.width).map(move |x| (x, y)))
        .filter(|&(x, y)| image.pixel(x, y) != bare.pixel(x, y))
        .count();
    assert!(drawn > 2000, "{drawn} pixels of picture and text");
    // Back two pages with the mouse.
    for _ in 0..2 {
        let (x, y) = (150, 290 + LAYOUT[1].1.top);
        let _ = tv.mouse_down(x, y, ShiftState::LEFT);
        let r = tv.mouse_up(x, y, &world, &info);
        assert!(r.refresh);
    }
    assert_eq!(tv.cursor, n - 2);
    let archive = paint(&tv, &palette);
    if let Some(path) = std::env::var_os("FERNSEHER_PPM") {
        write_ppm(Path::new(&path), &out, &palette);
        write_ppm(&PathBuf::from(path).with_extension("archive.ppm"), &archive, &palette);
    }
}
