//! Drawing the island (segment `1050`, which also loads `DARSTELL.DAT`).
//!
//! The view is isometric: each square of the map is a diamond 62 pixels
//! wide and 32 high, and each corner can be raised in steps of 9 pixels.
//! The main screen's `Hauptbild` shows up to 20 columns of 32 pixels and 27
//! half rows of 16. The drawing goes column by column, top to bottom. Each
//! column cuts through the diamonds, so it draws the left half of one
//! square, then the right half of the square below and to the left, and so
//! on; `DARSTELL.DAT`'s tables say where on the map each column and half row
//! starts for each of the four rotations, and how to step down.
//!
//! Every tile is two pictures in the same bitmap: a mask (black where the
//! tile is, white around it) that's `SRCAND`ed and the picture on black,
//! `SRCPAINT`ed or `SRCINVERT`ed over it. The ground mostly goes without the
//! mask: the column starts out black and the ground tiles don't overlap.
//!
//! This is all view code. It reads the [`World`] and draws into the
//! player's [`Surface`].

use formats::data::Darstell;
use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::world::{Island, NO_BUILDING, PALM, Pos, World};

/// The bitmaps the island is drawn from, from `GRAFIK`, with their index:
/// the device contexts `1030:1418` keeps in the word array at `1110:5fc0`.
/// `HAUSNR.DAT`'s last column picks one for each building type. The files
/// are loaded by 1030:307a.
pub const BITMAP_FILES: [(usize, &str); 10] = [
    (0, "gfxPalm2.BMP"),  // 1110:5fe6
    (1, "gfxhaus1.BMP"),  // 1110:5ff2
    (2, "gfxhaus6.BMP"),  // 1110:5ff6
    (3, "granim2.BMP"),   // 1110:603a
    (4, "gfxhaus5.BMP"),  // 1110:5ffa
    (5, "auto.bmp"),      // 1110:603e
    (7, "gfxgroun.BMP"),  // 1110:5fee
    (9, "gfxhaus9.BMP"),  // 1110:5ffe
    (11, "groundan.BMP"), // 1110:6036
    (15, "gfxflug.BMP"),  // 1110:5fea
];
const BITMAPS: usize = 16;
/// `gfxgroun.BMP`: ground, roads, rivers, the sea and the pointers.
const GROUND: usize = 7;
/// `gfxhaus1.BMP`, which also has the foundation.
const HAUS1: usize = 1;

/// A column's width.
pub const COLUMN_WIDTH: i32 = 32;
/// A half row's height.
pub const HALF_ROW: i32 = 16;
/// The columns the view has (`AnsichtSpaltenxKorrektur`'s size).
pub const COLUMNS: i16 = 20;
/// The half rows the view has (`Halbblockanzahly + 1`).
pub const HALF_ROWS: i16 = 27;
/// One height step.
const STEP: i32 = 9;

/// The drawing area's size. `Hauptbild` is 624×404, so the last column and
/// row are cut off. // 1110:12bc, 1110:12be, set by `init_game_defaults`
pub const VIEW_WIDTH: i32 = 624;
pub const VIEW_HEIGHT: i32 = 405;

/// How wide the left and right half of a diamond are. // 1110:1a8a
const HALF_WIDTH: [i32; 2] = [32, 30];
/// The first `GROUNDNR.DAT` row of each kind of ground; each has one row per
/// form (slope). // 1110:1202
pub(crate) const GROUND_ROWS: [u8; 10] = [0, 19, 38, 38, 57, 57, 0, 0, 0, 76];
/// How far each form lifts its tile, beyond the height of the corner it's
/// measured from. // 1110:00ac
const FORM_LIFT: [i32; 19] = [9, -9, -9, 9, 9, 0, -9, 0, 18, 0, -18, 0, 9, 0, -9, 0, 0, 0, 0];
/// A flat square: buildings on it get no foundation.
const FLAT: u8 = 16;
/// The foundation under buildings on a slope, in `gfxhaus1.BMP`: the
/// picture at 186,70, its mask at 124,70, 25 pixels high, drawn 16 pixels
/// below the highest corner. // 1110:120c..1110:1218
const FOUNDATION: Tile = Tile {
    height: 25,
    picture: (186, 70),
    mask: (124, 70),
};
const FOUNDATION_DROP: i32 = 16;
/// The sea outside the map: row 54 of `GFXPOS.DAT`. // 1050:0e68 (gfxpos+0x28a)
const SEA: usize = 54;
/// How the parts of the 2×2 building type 11 turn with the view.
/// // 1110:1a8e
const TURN_11: [[u8; 4]; 4] = [[0, 1, 2, 3], [1, 0, 3, 2], [0, 1, 2, 3], [1, 0, 3, 2]];
/// The same for building type 52's six parts. // 1110:1a9e
const TURN_52: [[u8; 6]; 4] = [
    [0, 1, 2, 3, 4, 5],
    [1, 2, 3, 0, 5, 4],
    [2, 3, 0, 1, 4, 5],
    [3, 0, 1, 2, 5, 4],
];

/// Where a tile and its mask are in a bitmap.
#[derive(Debug, Clone, Copy)]
struct Tile {
    height: i32,
    picture: (i32, i32),
    mask: (i32, i32),
}

impl Tile {
    /// A `GROUNPOS.DAT`/`STRASPOS.DAT`/`FLUSSPOS.DAT`/`GFXPOS.DAT` row:
    /// height, picture x, y, (unused), mask x, y.
    fn ground(row: &[i16; 6]) -> Self {
        Tile {
            height: row[0].into(),
            picture: (row[1].into(), row[2].into()),
            mask: (row[4].into(), row[5].into()),
        }
    }
}

/// A `HAUSPOS.DAT` row, or one of `DARSTELL.DAT`'s pointer positions,
/// which have the same layout (`Tgfxhausposindex`).
#[derive(Debug, Clone, Copy)]
struct Sprite {
    tile: Tile,
    width: i32,
    /// How far above the corner it stands on its top is.
    rise: i32,
    /// Where it starts in the left half's column.
    left: i32,
}

impl Sprite {
    fn new(row: &[i16; 12]) -> Self {
        Sprite {
            tile: Tile {
                height: row[0].into(),
                picture: (row[2].into(), row[3].into()),
                mask: (row[4].into(), row[5].into()),
            },
            width: row[1].into(),
            rise: row[6].into(),
            left: row[7].into(),
        }
    }
}

/// The tables and bitmaps the drawing uses, loaded when a game starts
/// (1030:3932).
#[derive(Debug, Clone)]
pub struct Tiles {
    /// `HAUSPOS.DAT`. // 1110:54f6
    pub hauspos: Vec<[i16; 12]>,
    /// `HAUSNR.DAT`: per building type, its turned type, first `HAUSPOS`
    /// row, width, depth, parts and bitmap. // 1110:54f2
    pub hausnr: Vec<[i16; 8]>,
    /// `GROUNPOS.DAT`. // 1110:54e2
    pub grounpos: Vec<[i16; 6]>,
    /// `GROUNDNR.DAT`: per kind of ground and form, the variants and the
    /// first `GROUNPOS` row. // 1110:54e6
    pub groundnr: Vec<[i16; 2]>,
    /// `GFXPOS.DAT`. // 1110:54de
    pub gfxpos: Vec<[i16; 6]>,
    /// `STRASPOS.DAT`. // 1110:54ea
    pub straspos: Vec<[i16; 6]>,
    /// `FLUSSPOS.DAT`. // 1110:54ee
    pub flusspos: Vec<[i16; 6]>,
    /// `DARSTELL.DAT`. // 1050:271b
    pub darstell: Darstell,
    bitmaps: [Option<Surface>; BITMAPS],
}

impl Tiles {
    /// The tables, and the bitmaps of [`BITMAP_FILES`] in that order.
    ///
    /// # Panics
    /// If there aren't as many bitmaps as files.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        hauspos: Vec<[i16; 12]>,
        hausnr: Vec<[i16; 8]>,
        grounpos: Vec<[i16; 6]>,
        groundnr: Vec<[i16; 2]>,
        gfxpos: Vec<[i16; 6]>,
        straspos: Vec<[i16; 6]>,
        flusspos: Vec<[i16; 6]>,
        darstell: Darstell,
        bitmaps: Vec<Surface>,
    ) -> Self {
        assert_eq!(bitmaps.len(), BITMAP_FILES.len(), "one bitmap per file");
        let mut slots: [Option<Surface>; BITMAPS] = Default::default();
        for ((index, _), bitmap) in BITMAP_FILES.iter().zip(bitmaps) {
            slots[*index] = Some(bitmap);
        }
        Tiles {
            hauspos,
            hausnr,
            grounpos,
            groundnr,
            gfxpos,
            straspos,
            flusspos,
            darstell,
            bitmaps: slots,
        }
    }

    /// A `HAUSNR.DAT` column for a building type, 0 for an unknown type.
    fn hausnr(&self, building: u8, column: usize) -> i16 {
        self.hausnr.get(usize::from(building)).map_or(0, |r| r[column])
    }

    /// How many parts (squares) a building type has. // hausnr[.][6]
    fn parts(&self, building: u8) -> i16 {
        self.hausnr(building, 6)
    }
}

/// What the player's view looks at. The original keeps these in globals;
/// a map file saves them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Camera {
    /// The rotation, 0–3 (`TAnsichtart`). // 1110:007c
    pub view: u8,
    /// The map position of the view's top left, before `DARSTELL.DAT`'s
    /// corrections. // 1110:5530, 1110:5532
    pub origin: Pos,
    /// Whether the columns' halves start the other way round. // 1110:5534
    pub parity: i16,
    /// Buildings show their owner's colour on a pointer. // 1110:0058
    pub owner_pointers: bool,
    /// Without `owner_pointers`: buildings show how full they are.
    /// // 1110:0059
    pub occupancy_pointers: bool,
}

impl Camera {
    fn view(&self) -> usize {
        usize::from(self.view & 3)
    }
}

/// The height the ground tile at `(i, j)` is measured from: one of the
/// square's corners, which one depending on the view. 0 off the map's top
/// or left edge. // 1050:075e
fn ground_height(island: &Island, view: usize, (i, j): Pos) -> u8 {
    let corner = match view {
        0 => (i, j),
        1 => (i - 1, j),
        2 => (i - 1, j - 1),
        _ => (i, j - 1),
    };
    if corner.0 < 0 || corner.1 < 0 {
        0
    } else {
        island.heights.get(corner).unwrap_or(0)
    }
}

/// Which of a building's `width × depth` parts a square shows from `view`,
/// given the part it is from the front. // 1050:0811
fn turn_part(part: u8, width: u8, depth: u8, view: usize) -> u8 {
    let (part, w, d) = (u16::from(part), u16::from(width).max(1), u16::from(depth));
    let turned = match view {
        0 => part,
        1 => part / w + d * ((w - 1).wrapping_sub(part % w)),
        2 => (w * d).wrapping_sub(1).wrapping_sub(part),
        _ => (d.wrapping_sub(1).wrapping_sub(part / w)).wrapping_add(d * (part % w)),
    };
    turned.to_le_bytes()[0]
}

/// The four-way groups of building types whose pictures are the same
/// thing turned: each view moves them round by one. // 1050:0997
fn turn_group(building: u8, view: usize) -> u8 {
    let start = match building {
        0x58..=0x6b => 0x58 + (building - 0x58) / 4 * 4,
        0x71..=0x78 => 0x71 + (building - 0x71) / 4 * 4,
        _ => return building,
    };
    // `view` is 0..=3.
    #[allow(clippy::cast_possible_truncation)]
    let turn = view as u8;
    start + (building - start + turn) % 4
}

/// The part of a building a square shows from the player's view, and the
/// building type whose pictures to use (some types have a turned twin).
/// // 1050:0997
fn turn_building(tiles: &Tiles, part: u8, building: u8, view: usize) -> (u8, u8) {
    let byte = |n: i16| n.to_le_bytes()[0];
    match building {
        0x0b => (*TURN_11[view].get(usize::from(part)).unwrap_or(&0), building),
        0x34 => (*TURN_52[view].get(usize::from(part)).unwrap_or(&0), building),
        0x00 | 0x0c | 0x15 | 0x1f | 0x24 => (part, building),
        _ => {
            let parts = tiles.parts(building);
            let part = i16::from(part).checked_rem(parts).unwrap_or(0);
            let turned = turn_part(
                byte(part),
                byte(tiles.hausnr(building, 3)),
                byte(tiles.hausnr(building, 4)),
                view,
            );
            let mut b = building;
            if view == 1 || view == 3 {
                b = byte(tiles.hausnr(b, 1));
            }
            if view == 1 || view == 2 {
                b = match b {
                    0x28 => 0x51,
                    0x2c => 0x53,
                    0x30 => 0x55,
                    _ => b,
                };
            }
            if view == 2 || view == 3 {
                b = match b {
                    0x29 => 0x52,
                    0x2d => 0x54,
                    0x31 => 0x56,
                    _ => b,
                };
            }
            (turned, turn_group(b, view))
        }
    }
}

/// The `HAUSPOS.DAT` row for a building's part, and the building type to
/// draw it as. The part count is the type's before turning, the first row
/// the turned type's, as in the original. // 1050:0e68
fn building_sprite(tiles: &Tiles, building: u8, part: u8, view: usize) -> (Option<usize>, u8) {
    let parts = tiles.parts(building);
    let (turned, building) = turn_building(tiles, part, building, view);
    let row = tiles.hausnr(building, 2) + i16::from(turned).checked_rem(parts).unwrap_or(0);
    (usize::try_from(row).ok().filter(|&r| r < tiles.hauspos.len()), building)
}

/// The drawing's running state, the globals at `1110:66a2`..`1110:66c1`.
struct Column<'a> {
    dst: &'a mut Surface,
    tiles: &'a Tiles,
    /// The column's left edge. // 1110:66b0
    x: i32,
    /// The bottom of what's drawn. // 1110:66b6
    bottom: i32,
    /// Which half of the diamonds this step draws: 0 left, 1 right.
    /// // 1110:66a8
    half: usize,
    /// A foundation was just drawn, so the next ground tile needs its mask.
    /// // 1110:66bc
    masked: bool,
}

impl Column<'_> {
    /// One `BitBlt` from bitmap `index` into the column.
    fn blit(&mut self, index: usize, (x, y): (i32, i32), (w, h): (i32, i32), from: (i32, i32), rop: Rop) {
        if w <= 0 || h <= 0 {
            return;
        }
        let Some(src) = self.tiles.bitmaps.get(index).and_then(Option::as_ref) else {
            return;
        };
        let clip = self.dst.bounds();
        self.dst.blit(x, y, src, Rect::new(from.0, from.1, w, h), rop, clip);
    }

    /// The half diamond of a ground, road or river tile whose top is at `y`,
    /// with its mask first when it has to cover a foundation.
    fn ground(&mut self, tile: Tile, y: i32) {
        let w = HALF_WIDTH[self.half];
        let h = tile.height.min(self.bottom - y);
        let dx = to_i32(self.half) * COLUMN_WIDTH;
        if self.masked {
            self.blit(GROUND, (self.x, y), (w, h), (tile.mask.0 + dx, tile.mask.1), Rop::And);
            self.masked = false;
        }
        self.blit(
            GROUND,
            (self.x, y),
            (w, h),
            (tile.picture.0 + dx, tile.picture.1),
            Rop::Paint,
        );
    }

    /// This column's share of a sprite whose top is at `y`: the left half
    /// draws from `sprite.left` to the column's right edge, the right half
    /// the rest. Draws the mask and then the picture with `rop`.
    fn sprite(&mut self, bitmap: usize, sprite: &Sprite, y: i32, height: i32, rop: Rop) {
        let half = to_i32(self.half);
        let cut = COLUMN_WIDTH - sprite.left;
        let w = (1 - 2 * half) * (cut - sprite.width * half);
        let x = (1 - half) * sprite.left + self.x;
        let dx = cut * half;
        let t = sprite.tile;
        self.blit(bitmap, (x, y), (w, height), (t.mask.0 + dx, t.mask.1), Rop::And);
        self.blit(bitmap, (x, y), (w, height), (t.picture.0 + dx, t.picture.1), rop);
    }
}

fn to_i32(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(0)
}

/// Draws columns `first` to `last` of the view. `rows` limits it to a band
/// for scrolling: 0 draws everything, `n > 0` the bottom `n` half rows and
/// `-n` the top `n`. The band is blacked out first. // 1050:0e68
///
/// The column's squares are drawn top to bottom until a step draws
/// nothing (its ground is below the bottom), then the 15 squares below
/// that once more for the buildings tall enough to reach into the view.
#[allow(clippy::too_many_lines)]
pub fn draw_columns(dst: &mut Surface, world: &World, tiles: &Tiles, cam: &Camera, first: i16, last: i16, rows: i16) {
    let island = &world.island;
    let dt = &tiles.darstell;
    let view = cam.view();
    let rows = i32::from(rows);
    let bottom = if rows < 0 { -HALF_ROW * rows } else { VIEW_HEIGHT };
    let start_row: usize = if rows < 1 {
        0
    } else {
        usize::try_from(26 - rows).unwrap_or(0)
    };
    let start_y = to_i32(start_row) * HALF_ROW;
    let width = (i32::from(last) + 1 - i32::from(first)) * COLUMN_WIDTH;
    let left = i32::from(first) * COLUMN_WIDTH;
    // BLACKNESS over the band.
    let band = match rows {
        ..0 => Rect::new(left, 0, width, bottom),
        0 => Rect::new(left, 0, width, VIEW_HEIGHT),
        _ => Rect::new(left, start_y, width, VIEW_HEIGHT - start_y),
    };
    let all = dst.bounds();
    dst.fill_rect(band, all, 0);

    // Which halves the first column starts with. // 1110:66aa
    let mut parity = usize::try_from((first % 2 + cam.parity).rem_euclid(2)).unwrap_or(0);
    let mut c = Column {
        dst,
        tiles,
        x: left,
        bottom,
        half: 0,
        masked: false,
    };
    for col in first..=last {
        let ci = usize::try_from(col).unwrap_or(0).min(19);
        let mut y = start_y - HALF_ROW; // 1110:66b2
        let mut i =
            dt.spalten_x_korrektur[view][parity][ci] + cam.origin.0 + dt.zeilen_x_korrektur[view][parity][start_row];
        let mut j =
            dt.spalten_y_korrektur[view][parity][ci] + cam.origin.1 + dt.zeilen_y_korrektur[view][parity][start_row];
        c.half = (1 - parity + start_row) % 2;
        c.masked = false;
        let step = |i: &mut i16, j: &mut i16, y: &mut i32, half: &mut usize| {
            *y += HALF_ROW;
            *i += dt.extratab_x_korrektur[view][*half];
            *j += dt.extratab_y_korrektur[view][*half];
            *half = 1 - *half;
        };
        loop {
            let drawn = if (0..=250).contains(&i) && (0..=250).contains(&j) {
                draw_square(&mut c, world, cam, (i, j), y)
            } else {
                // The sea around the map.
                let top = if world.flood { y - STEP } else { y };
                if top <= c.bottom {
                    if let Some(sea) = tiles.gfxpos.get(SEA) {
                        let sea = Tile::ground(sea);
                        let size = (HALF_WIDTH[c.half], 32.min(c.bottom - top));
                        let from = (sea.picture.0 + to_i32(c.half) * COLUMN_WIDTH, sea.picture.1);
                        c.blit(GROUND, (c.x, top), size, from, Rop::Paint);
                    }
                    true
                } else {
                    false
                }
            };
            step(&mut i, &mut j, &mut y, &mut c.half);
            if !drawn {
                break;
            }
        }
        for _ in 1..=15 {
            if (0..=250).contains(&i) && (0..=250).contains(&j) {
                draw_tall_building(&mut c, island, view, (i, j), y);
            }
            step(&mut i, &mut j, &mut y, &mut c.half);
        }
        c.x += COLUMN_WIDTH;
        parity = 1 - parity;
    }
}

/// Draws the whole view. // 1050:2700
pub fn draw_view(dst: &mut Surface, world: &World, tiles: &Tiles, cam: &Camera) {
    draw_columns(dst, world, tiles, cam, 0, COLUMNS - 1, 0);
}

/// One step down a column on the map: the square's ground, the foundation
/// and the building on it, and the building's pointer. Returns whether
/// anything was above the bottom. // 1050:0e68
#[allow(clippy::too_many_lines)]
fn draw_square(c: &mut Column, world: &World, cam: &Camera, pos: Pos, y: i32) -> bool {
    let island = &world.island;
    let tiles = c.tiles;
    let dt = &tiles.darstell;
    let view = cam.view();
    let mut height = ground_height(island, view, pos); // 1110:66bd
    let mut form = *dt.form_korrektur[view]
        .get(usize::from(island.forms.at(pos)))
        .unwrap_or(&0); // 1110:66be
    let mut ground = island.ground.at(pos); // 1110:66bf
    let mut building = island.buildings.at(pos); // 1110:66c0
    let mut part = island.parts.at(pos); // 1110:66c1
    let mut drawn = false;

    if world.flood {
        let lowest = island.lowest_corner(pos);
        if lowest <= 1 {
            if lowest == 0 {
                if ground > 0x36 {
                    ground = 2;
                }
                height = 1;
                match form {
                    8 | 9 | 11 => form -= 4,
                    10 => {
                        form = 6;
                        height = 2;
                    }
                    _ => form = FLAT,
                }
            } else {
                ground = match ground {
                    0x47 => 0x53,
                    0x4a => 0x54,
                    0x4d => 0x55,
                    0x50 => 0x56,
                    _ => 2,
                };
            }
            if building == PALM {
                part = u8::try_from(i16::from(part).checked_rem(tiles.parts(PALM)).unwrap_or(0)).unwrap_or(0);
            } else {
                part = 0;
                building = NO_BUILDING;
            }
        }
    }

    // The building's sprite, and whether the ground shows around it.
    let mut sprite = None;
    let mut with_ground = true;
    if building != NO_BUILDING {
        let (row, turned) = building_sprite(tiles, building, part, view);
        building = turned;
        sprite = row;
        with_ground = row.is_some_and(|r| tiles.hauspos[r][10] == 1);
    }

    if with_ground {
        let lift = height_px(height) + FORM_LIFT.get(usize::from(form)).copied().unwrap_or(0);
        let tile = match ground {
            0..10 => {
                let row = usize::from(GROUND_ROWS[usize::from(ground)]) + usize::from(form);
                let variant = if building == NO_BUILDING {
                    i16::from(part)
                } else {
                    i16::from(part).checked_div(tiles.parts(building)).unwrap_or(0)
                };
                tiles
                    .groundnr
                    .get(row)
                    .and_then(|g| usize::try_from(g[1] + variant).ok())
                    .and_then(|r| tiles.grounpos.get(r))
            }
            10..55 => dt.strassen_korrektur[view]
                .get(usize::from(ground - 10))
                .and_then(|&r| tiles.straspos.get(usize::from(r))),
            55..95 => dt.fluss_korrektur[view]
                .get(usize::from(ground - 55))
                .and_then(|&r| tiles.flusspos.get(usize::from(r))),
            _ => None,
        };
        if let Some(tile) = tile {
            let top = y - lift;
            if top <= c.bottom {
                c.ground(Tile::ground(tile), top);
                drawn = true;
            }
        }
    }

    let Some(row) = sprite else {
        return drawn;
    };
    let hp = &tiles.hauspos[row];
    let base = island.highest_corner(pos); // 1110:66a6
    if form != FLAT && hp[9] == 1 {
        let top = y - (height_px(base) - FOUNDATION_DROP);
        let h = if height == base {
            FOUNDATION.height - STEP
        } else {
            FOUNDATION.height
        };
        if top <= c.bottom {
            let w = HALF_WIDTH[c.half];
            let h = h.min(c.bottom - top);
            let dx = to_i32(c.half) * COLUMN_WIDTH;
            c.blit(
                HAUS1,
                (c.x, top),
                (w, h),
                (FOUNDATION.mask.0 + dx, FOUNDATION.mask.1),
                Rop::And,
            );
            c.blit(
                HAUS1,
                (c.x, top),
                (w, h),
                (FOUNDATION.picture.0 + dx, FOUNDATION.picture.1),
                Rop::Paint,
            );
            c.masked = true;
            drawn = true;
        }
    }

    let bitmap = usize::try_from(tiles.hausnr(building, 7)).unwrap_or(BITMAPS);
    let house = Sprite::new(hp);
    let top = y - (height_px(base) + house.rise);
    if top <= c.bottom {
        let h = house.tile.height.min(c.bottom - top);
        c.sprite(bitmap, &house, top, h, Rop::Invert);
        drawn = true;
    }

    // The pointer over the building.
    let kind = usize::try_from(hp[8]).unwrap_or(0);
    if kind == 0 {
        return drawn;
    }
    let pointer = if cam.owner_pointers {
        // The owner's colour: player_colours is at 1110:0074, and the
        // original reads 1110:0073 + owner. Owner 0 would read outside the
        // table, so it gets no pointer here.
        let owner = usize::from(island.owners.at(pos));
        owner
            .checked_sub(1)
            .and_then(|o| world.player_colours.get(o))
            .and_then(|&colour| usize::from(colour).checked_sub(1))
            .and_then(|colour| dt.spielerzeiger_pos.get(kind - 1)?.get(colour))
    } else if cam.occupancy_pointers {
        let tenth = usize::from((island.occupancy.at(pos) / 10).min(10));
        dt.auslastungszeiger_pos.get(kind - 1).map(|p| &p[tenth])
    } else {
        None
    };
    if let Some(p) = pointer {
        let sprite = Sprite::new(p);
        let top = y - (height_px(base) + sprite.rise);
        if top <= c.bottom {
            let h = sprite.tile.height.min(c.bottom - top);
            c.sprite(GROUND, &sprite, top, h, Rop::Paint);
            drawn = true;
        }
    }
    drawn
}

/// A building on a square whose ground is below the view: only its top
/// can show. Drawn down to the bottom of the view, not just the sprite's
/// height. // 1050:0e68
fn draw_tall_building(c: &mut Column, island: &Island, view: usize, pos: Pos, y: i32) {
    let tiles = c.tiles;
    let building = island.buildings.at(pos);
    if building == NO_BUILDING {
        return;
    }
    let base = island.highest_corner(pos);
    let (row, building) = building_sprite(tiles, building, island.parts.at(pos), view);
    let Some(row) = row else {
        return;
    };
    let house = Sprite::new(&tiles.hauspos[row]);
    let top = y - (height_px(base) + house.rise);
    if top <= c.bottom {
        let bitmap = usize::try_from(tiles.hausnr(building, 7)).unwrap_or(BITMAPS);
        let h = c.bottom - top;
        c.sprite(bitmap, &house, top, h, Rop::Invert);
    }
}

fn height_px(h: u8) -> i32 {
    i32::from(h) * STEP
}

impl Tiles {
    /// Whether all the bitmaps the island needs are there.
    #[must_use]
    pub fn complete(&self) -> bool {
        BITMAP_FILES.iter().all(|(i, _)| self.bitmaps[*i].is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turning_parts() {
        // A 2×3 building (width 2, depth 3), part 1 (front row, right).
        assert_eq!(turn_part(1, 2, 3, 0), 1);
        assert_eq!(turn_part(1, 2, 3, 1), 0);
        assert_eq!(turn_part(1, 2, 3, 2), 4);
        assert_eq!(turn_part(1, 2, 3, 3), 5);
    }

    #[test]
    fn turning_groups() {
        assert_eq!(turn_group(0x58, 1), 0x59);
        assert_eq!(turn_group(0x58, 2), 0x5a);
        assert_eq!(turn_group(0x58, 3), 0x5b);
        assert_eq!(turn_group(0x5f, 1), 0x5c);
        assert_eq!(turn_group(0x6b, 2), 0x69);
        assert_eq!(turn_group(0x71, 2), 0x73);
        assert_eq!(turn_group(0x78, 3), 0x77);
        assert_eq!(turn_group(0x6c, 3), 0x6c);
        assert_eq!(turn_group(0x40, 1), 0x40);
    }
}
