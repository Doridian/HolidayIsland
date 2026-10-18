//! What the human's build menu (`TBaumenue`, segment `1018`) offers: for
//! each of its 12 submenus up to 20 rows of building types. This is the
//! world's part of the form: the lists start as `BAUMENU.DAT` has them
//! (1018:080f), events and auctions add to them (`make_available`,
//! 1018:23f8) and building a type there's only one of takes it off
//! (`start_building`, 1018:26b9), and they're saved with the map (unit
//! 1018:34a0). The form itself, the selected rows and the scrolling are
//! the view's ([`crate::baumenue`]).
//!
//! Single human: the original has one build menu, player 1's. A human in
//! seats 2 to 8 has their own lists ([`crate::seats::SeatSlots::build_menu`],
//! [`World::build_menu_of`]), a copy of seat 1's when a client first takes
//! the seat; what an event gives everyone goes onto every human's.

use formats::data::BaumenuData;
use formats::savegame::SaveGame;

use crate::world::{Error, World};

/// The submenus, numbered by the tool (and button) that opens them:
/// houses 9, hotels 10, restaurants 11, shops 12, sport 13, leisure 14,
/// parks and palms 15, the road's 16 (never opened), rentals 17,
/// airports 18, harbours 19, utilities 20.
pub const FIRST_MENU: u8 = 9;
pub const MENUS: usize = 12;
/// The rows each submenu has room for.
pub const ROWS: usize = 20;
/// No building type in a row's cell.
pub const NONE: i16 = -1;

/// The submenu each kind of building (`HAUSDATA.DAT` column 5) goes into,
/// -1 for kind 7, which has none. // 1110:00ea
pub const KIND_MENUS: [i16; 12] = [9, 10, 11, 12, 13, 14, 15, -1, 0x11, 0x12, 0x13, 0x14];

/// The hotels whose four rows of looks are the type and the three after
/// it. // 1018:23f8
const FOUR_LOOKS: [u8; 7] = [0x71, 0x75, 0x58, 0x5c, 0x60, 0x64, 0x68];

/// The build menu's lists.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuildMenu {
    /// How many rows each submenu has. // *[1110:6516]+0x180 (`Byte`s)
    pub counts: [u8; MENUS],
    /// Each submenu's rows: a building type and the types of its other
    /// looks (the rotate button's), or [`NONE`]. // *[1110:6516]+0x184
    pub rows: [[[i16; 4]; ROWS]; MENUS],
    /// The rows again, as the selection checks them: a copy of
    /// [`BuildMenu::counts`] as `Integer`s, which the original keeps in
    /// step with them. // [1110:6516]+0x1b4 (saved from +0x1c6)
    pub lengths: [i16; MENUS],
}

impl Default for BuildMenu {
    fn default() -> Self {
        BuildMenu {
            counts: [0; MENUS],
            rows: [[[NONE; 4]; ROWS]; MENUS],
            lengths: [0; MENUS],
        }
    }
}

/// The index of submenu `menu` (9..=20) in the lists.
fn slot(menu: i16) -> Option<usize> {
    usize::try_from(menu - i16::from(FIRST_MENU))
        .ok()
        .filter(|&k| k < MENUS)
}

impl BuildMenu {
    /// The lists as `BAUMENU.DAT` has them, and the form's creation copies
    /// the counts (1018:2fa2). // 1018:080f
    #[must_use]
    pub fn new(data: &BaumenuData) -> Self {
        let mut menu = BuildMenu::default();
        for (k, entries) in data.menus.iter().enumerate().take(MENUS) {
            menu.rows[k] = entries.rows;
            menu.counts[k] = entries.count;
            menu.lengths[k] = i16::from(entries.count);
        }
        menu
    }

    /// The lists a map file saved. // 1018:34a0
    ///
    /// # Errors
    /// If a field is missing or has the wrong size.
    pub fn from_map(save: &SaveGame) -> Result<Self, Error> {
        let field = |dest: &'static str, size: usize| {
            save.fields(dest)
                .last()
                .filter(|d| d.len() == size)
                .ok_or(Error::Field(dest))
        };
        let counts = field("*[1110:6516]+0x180", MENUS)?;
        let rows = field("*[1110:6516]+0x184", MENUS * ROWS * 8)?;
        let lengths = field("[1110:6516]+0x1c6", MENUS * 2)?;
        Ok(Self::from_bytes(counts, rows, lengths).unwrap_or_default())
    }

    /// The lists from their three fields' bytes ([`BuildMenu::bytes`]),
    /// none if a size is wrong.
    #[must_use]
    pub fn from_bytes(counts: &[u8], rows: &[u8], lengths: &[u8]) -> Option<Self> {
        if counts.len() != MENUS || rows.len() != MENUS * ROWS * 8 || lengths.len() != MENUS * 2 {
            return None;
        }
        let mut menu = BuildMenu::default();
        menu.counts.copy_from_slice(counts);
        for (n, cell) in rows.as_chunks::<2>().0.iter().enumerate() {
            menu.rows[n / (ROWS * 4)][n / 4 % ROWS][n % 4] = i16::from_le_bytes(*cell);
        }
        for (k, n) in lengths.as_chunks::<2>().0.iter().enumerate() {
            menu.lengths[k] = i16::from_le_bytes(*n);
        }
        Some(menu)
    }

    /// The bytes of the three fields `save_map` writes: the counts, the
    /// rows and the lengths. // 1018:3350
    #[must_use]
    pub fn bytes(&self) -> [Vec<u8>; 3] {
        ["*[1110:6516]+0x180", "*[1110:6516]+0x184", "[1110:6516]+0x1c6"].map(|d| self.field(d).unwrap_or_default())
    }

    /// The bytes `save_map` writes for the field at `dest`, if it's one of
    /// the lists. // 1018:3350
    #[must_use]
    pub fn field(&self, dest: &str) -> Option<Vec<u8>> {
        Some(match dest {
            "*[1110:6516]+0x180" => self.counts.to_vec(),
            "*[1110:6516]+0x184" => self
                .rows
                .iter()
                .flatten()
                .flatten()
                .flat_map(|n| n.to_le_bytes())
                .collect(),
            "[1110:6516]+0x1c6" => self.lengths.iter().flat_map(|n| n.to_le_bytes()).collect(),
            _ => return None,
        })
    }

    /// Submenu `menu`'s rows (9..=20), none for another number.
    #[must_use]
    pub fn menu(&self, menu: u8) -> Option<(&[[i16; 4]; ROWS], u8, i16)> {
        let k = slot(i16::from(menu))?;
        Some((&self.rows[k], self.counts[k], self.lengths[k]))
    }
}

impl World {
    /// The submenu of `building`'s kind. // 1110:00ea
    fn kind_menu(&self, building: u8) -> i16 {
        let kind = self.tables.hausdata(building, 5);
        // BUG(original): a kind past 11 reads the words after the table; the port says none.
        usize::try_from(kind)
            .ok()
            .and_then(|k| KIND_MENUS.get(k))
            .copied()
            .unwrap_or(NONE)
    }

    /// `building` goes onto the end of its kind's submenu, with its other
    /// looks: the hotels of [`FOUR_LOOKS`] with the three types after
    /// them, a type that turns into another with the view (`HAUSNR.DAT`
    /// column 1) with that one and itself again, others alone. Nothing
    /// when the submenu is full. The menu is seat `p`'s
    /// ([`World::build_menu_of`]). // 1018:23f8
    pub(crate) fn add_to_build_menu(&mut self, p: u8, building: u8) {
        let menu = self.kind_menu(building);
        // BUG(original): kind 7 has no submenu (-1) and the original writes before the lists; the port leaves them.
        let Some(k) = slot(menu) else {
            return;
        };
        let turned = self.tables.hausnr(building, 1);
        let m = self.build_menu_mut(p);
        let count = m.counts[k];
        if count >= 0x14 {
            return;
        }
        let b = i16::from(building);
        m.rows[k][usize::from(count)] = if FOUR_LOOKS.contains(&building) {
            [b, b + 1, b + 2, b + 3]
        } else if turned == b {
            [b, NONE, NONE, NONE]
        } else {
            [b, turned, b, turned]
        };
        m.counts[k] = count.wrapping_add(1);
        m.lengths[k] = m.lengths[k].wrapping_add(1);
    }

    /// `building` comes off its kind's submenu: the first row that has it
    /// in any look, the rows after it moving up. Returns the submenu if it
    /// was there, whose selected row the original puts back to the first
    /// (the view's, [`crate::simulation::Tick::build_menu_removed`]).
    /// The menu is seat `p`'s ([`World::build_menu_of`]). // 1018:26b9
    pub(crate) fn remove_from_build_menu(&mut self, p: u8, building: u8) -> Option<u8> {
        let menu = self.kind_menu(building);
        // BUG(original): kind 7 has no submenu (-1) and the original reads before the lists; the port leaves them.
        let k = slot(menu)?;
        let m = self.build_menu_mut(p);
        let count = usize::from(m.counts[k]);
        let b = i16::from(building);
        let found = m.rows[k].iter().take(count).position(|row| row.contains(&b))?;
        for n in found..count.saturating_sub(1) {
            m.rows[k][n] = m.rows[k][n + 1];
        }
        if let Some(last) = count.checked_sub(1) {
            m.rows[k][last] = [NONE; 4];
        }
        m.counts[k] = m.counts[k].wrapping_sub(1);
        m.lengths[k] = m.lengths[k].wrapping_sub(1);
        u8::try_from(menu).ok()
    }

    /// The seats whose build menus what an event gives everyone (or a
    /// scenario takes off) changes: seat 1's, the original's one, and
    /// each of seats 2 to 8 that has its own. Single human: only seat 1's.
    pub(crate) fn build_menu_seats(&self) -> Vec<u8> {
        std::iter::once(1)
            .chain((2..=8).filter(|&p| self.slots(p).is_some_and(|s| s.build_menu.is_some())))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::world::Tables;

    fn world() -> World {
        let mut hausdata = vec![[0i16; 10]; 0x80];
        let mut hausnr = vec![[0i16; 8]; 0x80];
        for (b, row) in hausnr.iter_mut().enumerate() {
            row[1] = i16::try_from(b).unwrap();
        }
        hausdata[3][5] = 2; // a restaurant
        hausdata[13][5] = 0; // a house that turns
        hausnr[13][1] = 14;
        hausdata[0x60][5] = 1; // a hotel with four looks
        hausdata[5][5] = 7; // no menu
        World {
            tables: Arc::new(Tables {
                hausnr,
                hausdata,
                ..Tables::default()
            }),
            ..World::default()
        }
    }

    #[test]
    fn adding_and_removing() {
        let mut w = world();
        w.add_to_build_menu(1, 3);
        w.add_to_build_menu(1, 13);
        w.add_to_build_menu(1, 0x60);
        w.add_to_build_menu(1, 5);
        let m = &w.build_menu;
        assert_eq!((m.counts[2], m.lengths[2]), (1, 1));
        assert_eq!(m.rows[2][0], [3, -1, -1, -1]);
        assert_eq!(m.rows[0][0], [13, 14, 13, 14]);
        assert_eq!(m.rows[1][0], [0x60, 0x61, 0x62, 0x63]);
        w.add_to_build_menu(1, 4); // kind 0: houses
        assert_eq!(w.build_menu.counts[0], 2);
        // Found by another look.
        assert_eq!(w.remove_from_build_menu(1, 14), Some(9));
        assert_eq!(w.build_menu.rows[0][0][0], 4);
        assert_eq!(w.build_menu.rows[0][1], [-1; 4]);
        assert_eq!(w.build_menu.counts[0], 1);
        assert_eq!(w.remove_from_build_menu(1, 13), None);
        // A human in seat 2 has their own lists, a copy of seat 1's when
        // they first take the seat.
        w.apply_client(crate::seats::ClientCommand {
            client: crate::seats::HOST,
            action: crate::command::Action::Seat(crate::seats::SeatChange::Join { client: 4, seat: 2 }),
        });
        assert_eq!(w.build_menu_of(2), &w.build_menu);
        w.remove_from_build_menu(2, 4);
        assert_eq!(w.build_menu.counts[0], 1);
        assert_eq!(w.build_menu_of(2).counts[0], 0);
        assert_eq!(w.build_menu_seats(), [1, 2]);
    }
}
