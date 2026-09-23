//! The lamps: faces painted with a light's texture, which light what is near
//! them, blink when damaged, and go out when shot.
//!
//! A `.GLT` record names three textures - lit, unlit, broken - and eight
//! numbers. As the level loads, `0x48bd60` walks every face of the world - the
//! ground, the chamber floor and ceiling, and the six faces of both box sets -
//! and each face wearing a record's lit or unlit texture becomes up to three
//! lamps (`0x48b6b0`), one for each bit of the record's fifth number:
//!
//! - bit 1 (`0x48b9dd`): a light of the record's reach, its first number times
//!   eight, falling off to nothing there
//! - bit 2 (`0x48b6e5`): the same reach, in a cone of 45 degrees about the face
//! - bit 4 (`0x48b87b`): eight units, at full strength inside its box
//!
//! Every shipped record but one sets only bit 1.
//!
//! Each frame `0x48ae30` puts the lamps within ten cells of the eye that are
//! lit into the frame's lights. A damaged lamp **blinks**: on for the record's
//! third number, off for its fourth, swapping the face between the lit and
//! unlit textures as it goes. A shot on a lamp's face (`0x48c800`) takes one
//! of its hits - the record's seventh number - and at none left the lamp is
//! broken, its face wears the broken texture, and the shot bursts. With the
//! eighth number or fewer hits left it starts to blink.
//!
//! What the lights light is `light_at` (`0x48b550`, `0x48b1e0`): the sum of
//! every light whose box reaches the point, clamped to full. The engine adds it
//! to an object's ambient when the object is below ground, with the sun turned
//! off (`0x42f559`) - the lamps are what light the tunnels. Box corners take it
//! too (`0x415825`); how is not read yet.

use hb_formats::fixed::{from_units, to_units};
use hb_formats::vector::{dot, length, offset, within};
use hb_formats::terrain::{Layer, Terrain};
use hb_world::grid::{Cell, Grid};

/// The most lamps the engine keeps (`0x48b860`, 500).
pub const MOST: usize = 500;

/// How far from the eye, in cells, a lamp still gives light (`0x48ae8c`).
pub const NEAR_CELLS: i32 = 10;

/// A `.GLT` record with its textures found in the level's list.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Record {
    pub lit: Option<u16>,
    pub unlit: Option<u16>,
    pub broken: Option<u16>,
    /// The eight numbers.
    pub numbers: [i64; 8],
}

impl Record {
    /// Reach in units: the first number, 16.16, times eight (`0x48b72f`).
    pub fn reach(&self) -> f32 {
        to_units(self.numbers[0] as i32) * 8.0
    }
    /// Strength, 1.0 full: the second number (`+0x14`).
    pub fn strength(&self) -> f32 {
        to_units(self.numbers[1] as i32)
    }
    /// How long a blinking lamp stays on, and off: the third and fourth
    /// numbers, compared as they are with a 16.16 clock (`0x48afc1`,
    /// `0x48af1f`). The shipped off time is 6 - a frame.
    pub fn on_for(&self) -> f32 {
        to_units(self.numbers[2] as i32)
    }
    pub fn off_for(&self) -> f32 {
        to_units(self.numbers[3] as i32)
    }
    /// Which kinds of light its faces give (`+0x4c`).
    pub fn kinds(&self) -> i64 {
        self.numbers[4]
    }
    /// Shots to break it; 0 breaks on the first (`+0x54`).
    pub fn hits(&self) -> i32 {
        self.numbers[6] as i32
    }
    /// It blinks with this many hits left or fewer (`+0x58`).
    pub fn shaky(&self) -> i32 {
        self.numbers[7] as i32
    }
    /// Whether it blinks from the start (`0x48b7ab`).
    fn blinks_at_once(&self) -> bool {
        (self.numbers[2] != 0 || self.numbers[3] != 0) && self.shaky() >= self.hits()
    }
}

/// A face of the world: which surface, which cell, and for a box which of its
/// six faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Face {
    pub layer: Layer,
    pub cell: (usize, usize),
    pub side: usize,
}

impl Face {
    fn slot(&self) -> usize {
        (self.cell.1 & 127) * 128 + (self.cell.0 & 127)
    }

    /// The texture word it wears: an index in the low twelve bits, flags in
    /// the top four.
    pub fn texture(&self, t: &Terrain) -> u16 {
        let at = self.slot();
        match self.layer {
            Layer::Ground => t.colour.values[at],
            Layer::BoxA => t.boxes_a.textures.values[at * t.boxes_a.textures.per_cell + self.side],
            Layer::ChamberFloor => t.chambers.textures.values[at * t.chambers.textures.per_cell],
            Layer::ChamberCeiling => t.chambers.textures.values[at * t.chambers.textures.per_cell + 1],
            Layer::BoxB => t.boxes_b.textures.values[at * t.boxes_b.textures.per_cell + self.side],
        }
    }

    /// Paint it, keeping the top four bits (`0x48b060`).
    pub fn paint(&self, t: &mut Terrain, index: u16) {
        let at = self.slot();
        let slot = match self.layer {
            Layer::Ground => &mut t.colour.values[at],
            Layer::BoxA => &mut t.boxes_a.textures.values[at * t.boxes_a.textures.per_cell + self.side],
            Layer::ChamberFloor => &mut t.chambers.textures.values[at * t.chambers.textures.per_cell],
            Layer::ChamberCeiling => &mut t.chambers.textures.values[at * t.chambers.textures.per_cell + 1],
            Layer::BoxB => &mut t.boxes_b.textures.values[at * t.boxes_b.textures.per_cell + self.side],
        };
        *slot = (*slot & 0xf000) | (index & 0x0fff);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Unlit,
    Lit,
    /// On and off by the record's times; `on` is which it is now.
    Blinking { on: bool },
    Broken,
}

/// One kind of light (`+0x00`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Falls off to nothing at its reach.
    Round,
    /// Full strength inside a 45-degree cone about the face's normal.
    Cone,
    /// Full strength anywhere in its box.
    Flat,
}

/// A light this frame (`0x5caff0`, 44 bytes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Light {
    pub kind: Kind,
    pub at: [f32; 3],
    pub reach: f32,
    pub strength: f32,
    /// For a cone.
    pub normal: [f32; 3],
}

impl Light {
    /// A shot's or a missile's own light (`0x48a7f0`): round, reach 1.414
    /// times eight.
    pub fn moving(at: [f32; 3], strength: f32) -> Light {
        Light { kind: Kind::Round, at, reach: to_units(92_662) * 8.0, strength, normal: [0.0; 3] }
    }
}

/// A shot carries a light every fourth one drawn (`0x476e8c`), a quarter
/// strong (`0x3fff`).
pub const SHOT_LIGHT: f32 = to_units(16383);

#[derive(Debug, Clone, PartialEq)]
pub struct Lamp {
    pub record: usize,
    pub face: Face,
    pub kind: Kind,
    pub at: [f32; 3],
    pub normal: [f32; 3],
    pub state: State,
    /// Seconds into this on or off (`+0x34`).
    pub clock: f32,
    /// Shots left (`+0x4c`).
    pub hits: i32,
}

/// A face's texture to change, for the caller to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paint {
    pub face: Face,
    pub texture: u16,
}

#[derive(Debug, Clone, Default)]
pub struct Lamps {
    pub records: Vec<Record>,
    pub lamps: Vec<Lamp>,
}

impl Lamps {
    /// `0x48bd60`: every face wearing a light, as lamps.
    pub fn scan(records: Vec<Record>, terrain: &Terrain) -> Lamps {
        let grid = Grid::new(terrain);
        let mut lamps = Vec::new();
        let which = |word: u16| -> Option<(usize, bool)> {
            let index = word & 0x0fff;
            records.iter().enumerate().find_map(|(i, r)| {
                if r.lit == Some(index) {
                    Some((i, true))
                } else if r.unlit == Some(index) {
                    Some((i, false))
                } else {
                    None
                }
            })
        };
        for z in 0..128 {
            for x in 0..128 {
                let cell = Cell::new(x as i32, z as i32);
                let (ox, oz) = cell.signed_origin();
                let (cx, cz) = (ox + (4 << 16), oz + (4 << 16));
                let units = to_units;
                let mut faces: Vec<(Face, [f32; 3], [f32; 3])> = Vec::new();
                let flat = |layer: Layer, up: f32| -> Option<(Face, [f32; 3], [f32; 3])> {
                    let h = grid.height_at(layer, cx, cz)?;
                    Some((Face { layer, cell: (x, z), side: 0 }, [units(cx), units(h), units(cz)], [0.0, up, 0.0]))
                };
                faces.extend(flat(Layer::Ground, 1.0));
                if grid.has_chamber(cell) {
                    faces.extend(flat(Layer::ChamberFloor, 1.0));
                    faces.extend(flat(Layer::ChamberCeiling, -1.0));
                }
                for layer in [Layer::BoxA, Layer::BoxB] {
                    if !grid.has_box(layer, cell) {
                        continue;
                    }
                    let Some((bottom, top)) = grid.box_span(layer, cell) else { continue };
                    let mid = units(bottom + top) / 2.0;
                    // The middle of each face; `0x48bb50`, which the engine
                    // asks, is not read.
                    let sides: [([f32; 3], [f32; 3]); 6] = [
                        ([units(cx), mid, units(oz)], [0.0, 0.0, -1.0]),
                        ([units(cx), mid, units(oz) + 8.0], [0.0, 0.0, 1.0]),
                        ([units(ox) + 8.0, mid, units(cz)], [1.0, 0.0, 0.0]),
                        ([units(ox), mid, units(cz)], [-1.0, 0.0, 0.0]),
                        ([units(cx), units(top), units(cz)], [0.0, 1.0, 0.0]),
                        ([units(cx), units(bottom), units(cz)], [0.0, -1.0, 0.0]),
                    ];
                    for (side, (at, normal)) in sides.into_iter().enumerate() {
                        faces.push((Face { layer, cell: (x, z), side }, at, normal));
                    }
                }
                for (face, at, normal) in faces {
                    let Some((record, lit)) = which(face.texture(terrain)) else { continue };
                    let r = &records[record];
                    let state = if r.blinks_at_once() {
                        // The engine starts it on the record's sixth number
                        // as its phase, which is neither on (1) nor off (0)
                        // for every shipped record: dark until shot.
                        State::Blinking { on: r.numbers[5] == 1 }
                    } else if lit {
                        State::Lit
                    } else {
                        State::Unlit
                    };
                    // Bit 2, bit 4, then bit 1 - the order `0x48b6b0` makes them.
                    for (bit, kind) in [(2, Kind::Cone), (4, Kind::Flat), (1, Kind::Round)] {
                        if r.kinds() & bit != 0 && lamps.len() < MOST {
                            lamps.push(Lamp {
                                record,
                                face,
                                kind,
                                at,
                                normal,
                                state,
                                clock: 0.0,
                                hits: r.hits(),
                            });
                        }
                    }
                }
            }
        }
        Lamps { records, lamps }
    }

    /// `0x48ae30`: a frame of `dt` seconds with the eye in `eye_cell`. The
    /// lamps near enough that are lit give light; the blinking ones move on
    /// and repaint their faces.
    pub fn step(&mut self, dt: f32, eye_cell: (usize, usize)) -> (Vec<Light>, Vec<Paint>) {
        let (mut lights, mut paints) = (Vec::new(), Vec::new());
        for lamp in &mut self.lamps {
            let near = |a: usize, b: usize| {
                let d = (a as i32 - b as i32).rem_euclid(128);
                d <= NEAR_CELLS || d >= 128 - NEAR_CELLS
            };
            if !near(eye_cell.0, lamp.face.cell.0) || !near(eye_cell.1, lamp.face.cell.1) {
                continue;
            }
            let r = self.records[lamp.record];
            let gives = match lamp.state {
                State::Lit => true,
                State::Blinking { on } => {
                    lamp.clock += dt;
                    let now = if !on && lamp.clock > r.off_for() {
                        lamp.clock = 0.0;
                        paints.extend(r.lit.map(|texture| Paint { face: lamp.face, texture }));
                        true
                    } else if on && lamp.clock > r.on_for() {
                        lamp.clock = 0.0;
                        paints.extend(r.unlit.or(r.lit).map(|texture| Paint { face: lamp.face, texture }));
                        false
                    } else {
                        on
                    };
                    lamp.state = State::Blinking { on: now };
                    now
                }
                State::Unlit | State::Broken => false,
            };
            if gives {
                let reach = if lamp.kind == Kind::Flat { 8.0 } else { r.reach() };
                lights.push(Light { kind: lamp.kind, at: lamp.at, reach, strength: r.strength(), normal: lamp.normal });
            }
        }
        (lights, paints)
    }

    /// `0x48c800`: a shot on `face`. Every lamp on that surface of that cell
    /// takes a hit. Returns the repaint, if one broke, and whether one did.
    pub fn shot(&mut self, face: Face) -> (Option<Paint>, bool) {
        let mut paint = None;
        let mut broke = false;
        for lamp in &mut self.lamps {
            if lamp.face.layer != face.layer || lamp.face.cell != face.cell || lamp.state == State::Broken {
                continue;
            }
            let r = self.records[lamp.record];
            lamp.hits -= 1;
            if lamp.hits <= 0 {
                lamp.state = State::Broken;
                broke = true;
                paint = r.broken.or(r.lit).map(|texture| Paint { face, texture });
            } else if r.shaky() >= lamp.hits {
                lamp.state = State::Blinking { on: matches!(lamp.state, State::Blinking { on: true }) };
                lamp.clock = 0.0;
            }
        }
        (paint, broke)
    }
}

/// `0x48b550`: the light at a point from this frame's lights, 0 to 1. A light
/// counts where the point is inside its reach on every axis; within that,
/// `0x48b1e0` weighs it by kind. A round light's weight goes negative in the
/// corners of its box, past its reach, as the engine's does - the clamp at the
/// end is the only floor.
pub fn light_at(point: [f32; 3], lights: &[Light]) -> f32 {
    let mut sum = 0.0;
    for l in lights {
        if !within(l.at, point, l.reach) {
            continue;
        }
        let d = offset(l.at, point);
        let distance = length(d);
        sum += match l.kind {
            Kind::Round => l.strength * (l.reach - distance) / l.reach,
            Kind::Cone => {
                if distance * std::f32::consts::FRAC_1_SQRT_2 < dot(d, l.normal) { l.strength } else { 0.0 }
            }
            Kind::Flat => l.strength,
        };
    }
    sum.clamp(0.0, 1.0)
}

/// The face a shot that stopped at `at` (units) stopped on, as `0x48c800`
/// finds it (`0x41ba50`, `0x41bc00`): below the ground the chamber floor if
/// the point is at or under it, else box set B if it is under that box's
/// bottom, else the ceiling; above it box set A if the point is in or under
/// a box, else the ground. For a box, the side nearest the point -
/// `intersectingBoxSurface` (`0x4294c0`), which the engine asks, is not read.
pub fn face_at(at: [f32; 3], terrain: &Terrain) -> Face {
    let grid = Grid::new(terrain);
    let (x, y, z) = (from_units(at[0]), from_units(at[1]), from_units(at[2]));
    let cell = Cell::containing(x, z);
    let here = (cell.x as usize, cell.z as usize);
    let side = |layer: Layer| -> usize {
        let (ox, oz) = cell.signed_origin();
        let (bottom, top) = grid.box_span(layer, cell).unwrap_or((y, y));
        let within = |v: i32, o: i32| to_units(v.wrapping_sub(o));
        let (fx, fz) = (within(x, ox), within(z, oz));
        let distances = [
            fz,
            8.0 - fz,
            8.0 - fx,
            fx,
            to_units(top - y),
            to_units(y - bottom),
        ];
        (0..6).min_by(|&a, &b| distances[a].abs().total_cmp(&distances[b].abs())).unwrap_or(0)
    };
    let face = |layer: Layer, side: usize| Face { layer, cell: here, side };
    if y < 0 {
        let floor = grid.height_at(Layer::ChamberFloor, x, z).unwrap_or(i32::MIN);
        if floor >= y {
            return face(Layer::ChamberFloor, 0);
        }
        if grid.has_box(Layer::BoxB, cell) && grid.box_span(Layer::BoxB, cell).is_some_and(|(b, _)| b > y) {
            return face(Layer::BoxB, side(Layer::BoxB));
        }
        return face(Layer::ChamberCeiling, 0);
    }
    if grid.has_box(Layer::BoxA, cell) && grid.box_span(Layer::BoxA, cell).is_some_and(|(_, t)| y <= t) {
        return face(Layer::BoxA, side(Layer::BoxA));
    }
    face(Layer::Ground, 0)
}
