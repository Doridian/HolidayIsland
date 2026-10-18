//! A view's own people, with `fix.view_people` (`docs/people.md`, "Who
//! is whose"): the spectators at a full tennis court and at Miss Holiday
//! Island's stage, the builders on a building site, the planes taxiing at
//! an airport, the boats at the `Bigport`'s jetties and the harbours'
//! sounds. The original sends them only to the squares the human's view
//! looks at (`spawn_people`, 1090:2373, and `people_refresh`, 1088:7fb1,
//! after the view moved), with the world's random numbers, and nothing in
//! the world reads them but that spawning. Here each view has its own,
//! with its own random numbers ([`ViewPeople`]), driven by its own ticks
//! and its own viewpoint, and the world never sees them.
//!
//! Their code is the original's for those kinds (1090:2373's squares,
//! 1088:0324's cases, 1098:109f's animations, 1088:7a14's placing), on
//! the view's records instead of the world's. Two things are the port's:
//! with `people_everywhere` they come to every square of the island, not
//! only the view's; and they go when their building does (a finished
//! site, a court whose championship ended, a demolished jetty), where the
//! original's world code takes them away (1088:6bac, 10a0:0026,
//! 1088:6975).

use std::collections::HashSet;

use vcl::random::Random;

use super::movement::{Frames, IN_PLACE, time_up};
use super::{
    AIRPORT_PLANES, At, BUILDER, BUILDER_SOUNDS, DOORS, FIRST, FIRST_JETTY, Fill, GENERAL_END, JETTY_BOAT,
    JETTY_OFFSETS, NOBODY, People, Person, Placer, SIMPLE, STAGE_SPECTATORS, TAXIING, TAXIING_OFFSETS_35,
    TAXIING_OFFSETS_36, TAXIING_OFFSETS_37, TAXIING_OFFSETS_38, TAXIING_SPRITES, TENNIS_SPECTATORS, Viewpoint,
    is_view_kind, sees_in, view_square,
};
use crate::enhance::Fix;
use crate::simulation::Tick;
use crate::world::{Pos, World};

/// As many as the world's general range holds (0x2f..0x578).
#[allow(clippy::cast_sign_loss)] // Both are positive.
const CAPACITY: usize = (GENERAL_END - FIRST) as usize;

/// What a view looks at, for its own people.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Looking {
    /// The original's 20 × 27 view: its rotation, and its 11 × 61 squares
    /// from the origin where new people come ([`view_square`]).
    pub viewpoint: Viewpoint,
    /// The view as it's drawn (with `big_map` a bigger one), `columns`
    /// wide and `half_rows` high: what it sees, which hears the sounds.
    pub seen: Viewpoint,
    pub columns: i16,
    pub half_rows: i16,
    /// `people_everywhere`: new people come to every square.
    pub everywhere: bool,
}

impl Looking {
    /// The original's view `viewpoint`, as it draws it.
    #[must_use]
    pub fn original(viewpoint: Viewpoint) -> Self {
        Looking {
            viewpoint,
            seen: viewpoint,
            columns: crate::darstellung::COLUMNS,
            half_rows: crate::darstellung::HALF_ROWS,
            everywhere: false,
        }
    }

    fn sees(&self, world: &World, pos: Pos) -> bool {
        sees_in(&self.seen, &world.island, pos, self.columns, self.half_rows)
    }
}

/// One view's own people ([`super::is_view_kind`]) and its random numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewPeople {
    /// The records, a free one of kind 0, up to [`CAPACITY`]; and the
    /// runway of the next taxiing plane (`lane`, `1110:6fcc`).
    people: People,
    /// While new ones come: who's where, by kind, so that looking for one
    /// (1088:4ae2) doesn't go through every record for every square of
    /// the island. Nothing moves meanwhile.
    at: HashSet<(u8, Pos)>,
    /// While new ones come: every record is taken.
    full: bool,
    /// The view's random numbers, never the world's.
    pub random: Random,
}

impl ViewPeople {
    /// None yet, with random numbers from `seed`.
    #[must_use]
    pub fn new(seed: u32) -> Self {
        ViewPeople {
            people: People::empty(),
            at: HashSet::new(),
            full: false,
            random: Random::new(seed),
        }
    }

    /// The records, for drawing: a free one is of kind 0.
    #[must_use]
    pub fn records(&self) -> &[Person] {
        &self.people.records
    }

    /// How many there are.
    #[must_use]
    pub fn count(&self) -> usize {
        self.people.records.iter().filter(|p| p.kind() != NOBODY).count()
    }

    /// Everyone goes: the fix is off, or another game came.
    pub fn clear(&mut self) {
        self.people.records.clear();
    }

    /// Takes over the people the world handed to the views
    /// ([`Tick::view_people`]), as they are.
    pub fn adopt(&mut self, records: &[Person]) {
        for p in records.iter().filter(|p| is_view_kind(p.kind())) {
            if let Some(n) = self.free()
                && let Some(q) = self.people.get_mut(n)
            {
                *q = p.clone();
            }
        }
    }

    /// A clock tick with `world` as it is after its own: unless the fix is
    /// off (when they all go) or the animations are, those whose building
    /// went leave, the rest move while people move (1098:109f), and new
    /// ones come while they're animated and spawn (1090:2373). The sounds
    /// this view plays, in order.
    pub fn tick(&mut self, world: &World, look: &Looking) -> Vec<u8> {
        let mut sounds = Vec::new();
        if !world.people_split() {
            self.clear();
            return sounds;
        }
        if !world.options.animations {
            return sounds;
        }
        self.leave_gone_buildings(world);
        if world.options.people_move {
            for n in 0..self.len() {
                self.animate(world, n, look);
            }
        }
        if world.options.people_spawn {
            self.spawn(world, look, &mut sounds);
        }
        sounds
    }

    /// The view scrolled or turned (`people_refresh`, 1088:7fb1): its
    /// people are placed again for its rotation (1088:7a14), and while
    /// they're animated and spawn new ones come where it looks. The
    /// sounds this view plays.
    pub fn refresh(&mut self, world: &World, look: &Looking) -> Vec<u8> {
        let mut sounds = Vec::new();
        if !world.people_split() {
            return sounds;
        }
        self.placer(world, look).place_again_from_home(); // 1088:7a14
        if world.options.animations && world.options.people_spawn {
            self.spawn(world, look, &mut sounds);
        }
        sounds
    }

    /// The sounds a world's tick `t` has for this view: [`Tick::sounds`],
    /// and among them in their places those of [`Tick::placed_sounds`]
    /// this view sees, each picked with the view's random numbers; and
    /// where `people_tick` counts the sounds' times down
    /// ([`Tick::sound_countdown`]) in that list.
    pub fn hear(&mut self, world: &World, t: &Tick, look: &Looking) -> (Vec<u8>, Option<usize>) {
        let mut out = Vec::new();
        let mut countdown = None;
        let mut placed = t.placed_sounds.iter().peekable();
        for k in 0..=t.sounds.len() {
            if t.sound_countdown == Some(k) {
                countdown = Some(out.len());
            }
            while let Some(s) = placed.next_if(|s| s.after <= k) {
                if !s.sounds.is_empty() && look.sees(world, s.at) {
                    let pick = if s.sounds.len() > 1 {
                        let len = u16::try_from(s.sounds.len()).unwrap_or(u16::MAX);
                        usize::from(self.random.random(len))
                    } else {
                        0
                    };
                    out.extend(s.sounds.get(pick));
                }
            }
            if let Some(&s) = t.sounds.get(k) {
                out.push(s);
            }
        }
        (out, countdown)
    }

    /// The people a saved game gets with `fix.view_people`
    /// ([`crate::savegame::MapView::people`]): the world's placed for
    /// `viewpoint` ([`World::people_for_view`]), without the buildings'
    /// squares the original doesn't keep, and this view's own in the
    /// general range's free records, as 1088:0324 would have put them.
    #[must_use]
    pub fn saved_with(&self, world: &World, viewpoint: Viewpoint) -> People {
        let mut people = world.people_for_view(viewpoint);
        for p in &mut people.records {
            let kind = p.kind();
            if SIMPLE.iter().any(|s| s.kind == kind) || (0x1a..=0x1f).contains(&kind) {
                p.set_home((0, 0));
            }
        }
        for own in self.people.records.iter().filter(|p| p.kind() != NOBODY) {
            // 1088:0324's search: up to the last, which grows when it's
            // the one taken.
            let Some(n) = (FIRST..=people.last.min(GENERAL_END - 1)).find(|&n| people.kind(n) == NOBODY) else {
                break;
            };
            if n == people.last {
                people.last += 1;
            }
            if let Some(p) = people.get_mut(n) {
                *p = own.clone();
                p.set_home((0, 0));
            }
        }
        people
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // Up to CAPACITY.
    fn len(&self) -> i16 {
        self.people.records.len() as i16
    }

    fn placer<'a>(&'a mut self, world: &'a World, look: &Looking) -> Placer<'a> {
        Placer {
            w: world,
            people: &mut self.people,
            r: look.viewpoint.rotation(),
        }
    }

    /// The first free record, or a new one while there's room.
    fn free(&mut self) -> Option<i16> {
        if self.full {
            return None;
        }
        let records = &mut self.people.records;
        let n = match records.iter().position(|p| p.kind() == NOBODY) {
            Some(n) => n,
            None if records.len() < CAPACITY => {
                records.push(Person::default());
                records.len() - 1
            }
            None => {
                self.full = true;
                return None;
            }
        };
        i16::try_from(n).ok()
    }

    fn remove(&mut self, n: i16) {
        if let Some(p) = self.people.get_mut(n) {
            p.set_kind(NOBODY);
        }
    }

    /// Whether someone of `kind` is on `pos`. // 1088:4ae2
    fn person_at(&self, kind: u8, pos: Pos) -> bool {
        self.at.contains(&(kind, pos))
    }

    /// Those whose building (the square they came for, `+0x22`) isn't the
    /// kind that sent them any more leave. The port's: the original's
    /// world takes them away when it changes the building (1088:6bac,
    /// 10a0:0026, 1088:6975, 10a0:0b49). Those taken over from the world
    /// don't know their building and stay until their time is up.
    fn leave_gone_buildings(&mut self, world: &World) {
        for p in &mut self.people.records {
            let kind = p.kind();
            let home = p.home();
            if kind == NOBODY || home == (0, 0) {
                continue;
            }
            let b = world.building_or_none(home);
            let stays = match kind {
                BUILDER => b == 0x1f,
                JETTY_BOAT => (0x68..=0x6b).contains(&b),
                STAGE_SPECTATORS => b == 0x7b || b == 0x7c,
                k if TAXIING.contains(&k) => (0x28..=0x33).contains(&b),
                k if TENNIS_SPECTATORS.contains(&k) => b == 0x70,
                _ => true,
            };
            if !stays {
                p.set_kind(NOBODY);
            }
        }
    }

    /// 1098:109f for the views' kinds: the taxiing planes roll, the rest
    /// count their frames until their time is up.
    fn animate(&mut self, world: &World, n: i16, look: &Looking) {
        let kind = self.people.kind(n);
        if TAXIING.contains(&kind) {
            self.animate_taxiing(world, n, kind, look);
        } else if let Some(&(_, frames, first)) = IN_PLACE.iter().find(|a| a.0 == kind) {
            let Some(p) = self.people.get_mut(n) else {
                return;
            };
            p.set_path(p.path().wrapping_add(1));
            if time_up(p) {
                self.remove(n); // 1088:2df2
                return;
            }
            let frame = p.path();
            let picture = match frames {
                Frames::Each => frame,
                Frames::Half => frame / 2,
                Frames::Cycle(k) => frame % k,
            };
            p.set_sprite(picture.wrapping_add(first));
        }
    }

    /// A plane taxiing to its runway, as `World::animate_taxiing`
    /// (1098:109f).
    fn animate_taxiing(&mut self, world: &World, n: i16, kind: u8, look: &Looking) {
        let k = usize::from(kind - 0x35);
        let Some(p) = self.people.get_mut(n) else {
            return;
        };
        p.set_path(p.path().wrapping_add(1));
        if time_up(p) {
            let (along_j, steps) = match kind {
                0x35 => (true, Some(4)),
                0x36 => (false, Some(4)),
                0x37 => (true, Some(2)),
                0x38 => (false, Some(2)),
                0x39 => (true, None),
                _ => (false, None),
            };
            let advance = |p: &mut Person| {
                let (i, j) = p.pos();
                p.set_pos(if along_j {
                    (i, j.wrapping_sub(1))
                } else {
                    (i.wrapping_sub(1), j)
                });
                p.set_target((p.target().0.wrapping_sub(1), p.target().1));
            };
            if let Some(steps) = steps {
                p.set_target((p.target().0, p.target().1.wrapping_sub(1)));
                let mut placer = self.placer(world, look);
                match kind {
                    0x35 => placer.turn_taxiing(n, &TAXIING_OFFSETS_35), // 1098:0745
                    0x36 => placer.turn_taxiing(n, &TAXIING_OFFSETS_36), // 1098:0a14
                    0x37 => placer.turn_taxiing(n, &TAXIING_OFFSETS_37), // 1098:0ce3
                    _ => placer.turn_taxiing(n, &TAXIING_OFFSETS_38),    // 1098:0e6d
                }
                let Some(p) = self.people.get_mut(n) else {
                    return;
                };
                if p.target().1 == 0 {
                    advance(p);
                    p.set_target((p.target().0, steps));
                }
            } else {
                advance(p);
            }
            let Some(p) = self.people.get_mut(n) else {
                return;
            };
            if p.target().0 == 0 {
                p.set_kind(NOBODY); // 1088:2df2
            }
            p.set_path(1);
        }
        let first = TAXIING_SPRITES[look.viewpoint.rotation()][k];
        if let Some(p) = self.people.get_mut(n) {
            p.set_sprite(first.wrapping_add(p.path()));
        }
    }

    /// New people, while people move: 1090:2373's second part, for the
    /// view's 11 × 61 squares ([`view_square`]) or, with
    /// `people_everywhere`, every square of the island.
    fn spawn(&mut self, world: &World, look: &Looking, sounds: &mut Vec<u8>) {
        if !world.options.people_move {
            return;
        }
        self.at = self
            .people
            .records
            .iter()
            .filter(|p| p.kind() != NOBODY)
            .map(|p| (p.kind(), p.pos()))
            .collect();
        self.full = false;
        if look.everywhere {
            for i in 1..250 {
                for j in 1..250 {
                    self.spawn_at(world, (i, j), look, sounds);
                }
            }
            return;
        }
        let origin = look.viewpoint.origin;
        for l in 0..=60 {
            for e in 0..=10 {
                let (di, dj) = view_square(e, l);
                let pos = (origin.0.wrapping_add(di), origin.1.wrapping_add(dj));
                if (1..250).contains(&pos.0) && (1..250).contains(&pos.1) {
                    self.spawn_at(world, pos, look, sounds);
                }
            }
        }
    }

    /// `World::spawn_at` for the view: one square. // 1090:2373
    fn spawn_at(&mut self, world: &World, pos: Pos, look: &Looking, sounds: &mut Vec<u8>) {
        let r = look.viewpoint.rotation();
        let b = world.building_or_none(pos);
        match b {
            0x70 => {
                if world.part_or_zero(pos) == 0 {
                    let d = DOORS[r][1];
                    let at = (pos.0.wrapping_add(d.0), pos.1.wrapping_add(d.1));
                    for kind in TENNIS_SPECTATORS {
                        if !self.person_at(kind, at) {
                            self.add(world, kind, pos, look, sounds);
                        }
                    }
                }
            }
            0x7b | 0x7c => {
                // BUG(original): works out the stage's offset (1110:3a58), then looks for spectators on the square itself.
                let at = if world.fixes.has(Fix::StageSpectators) {
                    super::place_at(world, r, At::Stage, pos)
                } else {
                    pos
                };
                if world.part_or_zero(pos) == 0 && !self.person_at(STAGE_SPECTATORS, at) {
                    self.add(world, STAGE_SPECTATORS, pos, look, sounds);
                }
            }
            0x1f => {
                if world.part_or_zero(pos) == 0 && self.random.random(50) == 1 && !self.person_at(BUILDER, pos) {
                    self.add(world, BUILDER, pos, look, sounds);
                }
            }
            0x28..=0x33 => {
                if self.random.random(100) != 1 {
                    return;
                }
                let h = AIRPORT_PLANES[usize::from(b - 0x28)];
                let part = if h.random && self.random.random(2) != 1 {
                    h.parts[1]
                } else {
                    h.parts[0]
                };
                if world.part_or_zero(pos) == part {
                    self.people.lane = h.lane;
                    if !self.person_at(h.kind, pos) {
                        self.add(world, h.kind, pos, look, sounds);
                    }
                }
            }
            0x58..=0x6b => {
                if self.random.random(500) == 1 && look.sees(world, pos) {
                    match self.random.random(2) {
                        0 => sounds.push(0x22),
                        1 => sounds.push(0x10),
                        _ => {}
                    }
                }
                if (0x68..=0x6b).contains(&b) && world.part_or_zero(pos) == 0 {
                    let (di, dj) = JETTY_OFFSETS
                        .get(usize::from(b - FIRST_JETTY) + 4 * r)
                        .copied()
                        .unwrap_or((0, 0));
                    let at = (di.wrapping_add(pos.0), dj.wrapping_add(pos.1));
                    if self.random.random(10) == 1 && !self.person_at(JETTY_BOAT, at) {
                        self.add(world, JETTY_BOAT, pos, look, sounds);
                    }
                }
            }
            _ => {}
        }
    }

    /// Someone of `kind` for the building on `pos`, as 1088:0324 fills
    /// them, and their building's square (`+0x22`), which the original
    /// leaves 0. The builders make a sound (`Random(5)`) where the view
    /// sees them.
    fn add(&mut self, world: &World, kind: u8, (i, j): Pos, look: &Looking, sounds: &mut Vec<u8>) {
        let Some(n) = self.free() else {
            return;
        };
        let r = look.viewpoint.rotation();
        if TAXIING.contains(&kind) {
            let k = usize::from(kind - 0x35);
            let (target_j, offset) = match kind {
                0x35 => (4, (-1, 0xc)),
                0x36 => (4, (0x1b, 0xc)),
                0x37 => (2, (7, 0xb)),
                0x38 => (2, (0x19, 0xb)),
                _ => (0, (0x10, 7)),
            };
            let lane = self.people.lane;
            if let Some(p) = self.people.get_mut(n) {
                p.fill(&Fill {
                    pos: (i, j),
                    offset,
                    heading: 0,
                    sprite: TAXIING_SPRITES[r][k],
                    flags: [1, 0, 1],
                    kind,
                    path: 0,
                    step: 4,
                    target: (lane, target_j),
                    home: (i, j),
                    word_26: 0,
                });
            }
            self.at.insert((kind, (i, j)));
            let mut placer = self.placer(world, look);
            match kind {
                0x35 => placer.turn_taxiing(n, &TAXIING_OFFSETS_35),
                0x36 => placer.turn_taxiing(n, &TAXIING_OFFSETS_36),
                0x37 => placer.turn_taxiing(n, &TAXIING_OFFSETS_37),
                0x38 => placer.turn_taxiing(n, &TAXIING_OFFSETS_38),
                _ => {}
            }
            return;
        }
        let Some(s) = SIMPLE.iter().find(|s| s.kind == kind).copied() else {
            return;
        };
        let pos = super::place_at(world, r, s.at, (i, j));
        if let Some(p) = self.people.get_mut(n) {
            p.fill(&Fill {
                pos,
                offset: s.offset,
                sprite: s.sprite,
                flags: [1, 0, 1],
                kind,
                step: s.step,
                home: (i, j),
                ..Fill::default()
            });
        }
        self.at.insert((kind, pos));
        if kind == BUILDER && look.sees(world, pos) {
            sounds.push(BUILDER_SOUNDS[usize::from(self.random.random(5))]);
        }
    }
}
