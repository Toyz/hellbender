//! The terrain grids: a 128 x 128 world in thirteen headerless arrays.
//!
//! One base surface, two sets of extruded boxes, one set of chambers. See
//! `docs/formats/terrain.md`; the layer names are the engine's own, taken from
//! the diagnostics next to each `sprintf` format string in the loader at
//! `HELLBEND.EXE:0x412c00`.

use crate::{u16_at, Error, Result};

pub const SIDE: usize = 128;
pub const CELLS: usize = SIDE * SIDE;

/// Altitude bytes are scaled by 128, so a level has 256 heights and a vertical
/// step of 128 world units.
pub const ALTITUDE_SHIFT: u32 = 7;

/// One cell is 8.0 world units square: `0x80000` in 16.16.
///
/// The engine derives a cell's world origin as `(index & 0x7f) << 19`, which
/// `groundTriangleMidpoint` at `0x428900` does with `shl 25` followed by
/// `sar 6`. The mask is why the world wraps at 128 cells rather than clamping.
pub const CELL_SIZE: i32 = 0x0008_0000;

/// How an altitude byte becomes a stored altitude.
///
/// The loader uses two different scalings and which one it uses is per layer,
/// not per level. Getting this wrong puts chambers and the second box set in
/// the wrong half of the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    /// `byte << 7`, giving 0 to 32,640. The ground and box set A.
    Up,
    /// `(byte - 255) << 7`, giving -32,640 to 0. Chambers and box set B, which
    /// the loader reads with a `sub ax, 0xff` before the shift.
    Down,
}

impl Scale {
    pub fn apply(self, byte: u8) -> i16 {
        match self {
            Scale::Up => (byte as i16) << ALTITUDE_SHIFT,
            Scale::Down => (byte as i16 - 255) << ALTITUDE_SHIFT,
        }
    }
}

/// Six texture slots per ground box: four sides, top and bottom.
pub const BOX_TEXTURES: usize = 6;

/// Which face each of a box's six texture slots covers.
///
/// Read from the box drawer, which draws each face from its own call site
/// into `0x414f60` with the slot's word from the 18-byte box cell (slot n at
/// `+4 + 2n`), the face's normal for the back-face test at `0x456650`, and the
/// neighbouring box cell it checks for a face that cannot be seen:
///
/// ```text
/// slot  site      normal        neighbour   quad (box vertex numbers)
///  0    0x415cc0  (0, 0, -1)    z - 1       0 1 5 4
///  1    0x416773  (0, 0, +1)    z + 1       2 3 7 6
///  2    0x417222  (+1, 0, 0)    x + 1       1 2 6 5
///  3    0x417cb6  (-1, 0, 0)    x - 1       3 0 4 7
///  4    0x4186de  (0, +1, 0)    -           12 13 14 15
///  5    0x4188a1  (0, -1, 0)    -           8 9 10 11
/// ```
///
/// Box vertices 0-3 are the bottom corners and 4-7 the top, each in the
/// ground's corner order: (x, z), (x+1, z), (x+1, z+1), (x, z+1). Vertices
/// 8-15 are copies of 0-7 - each call site follows the face with a
/// `rep movs` of 0x48 dwords from `0x59d360` to `0x59d480` - so the bottom is
/// the four bottom corners and the top the four top corners, in that order. The earlier
/// measurement - odd-one-out slots against exposed neighbours - had already
/// paired slots 0 and 1 with z and 2 and 3 with x; this gives the signs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxFace {
    /// Slot 0, the side facing -z.
    NegZ = 0,
    /// Slot 1, the side facing +z.
    PosZ = 1,
    /// Slot 2, the side facing +x.
    PosX = 2,
    /// Slot 3, the side facing -x.
    NegX = 3,
    /// Slot 4. The top. Carries by far the most variety - 80 distinct values
    /// across `FLOAT`'s 1,142 boxes, against 18 for the bottom.
    Top = 4,
    /// Slot 5. The bottom, and the least varied.
    Bottom = 5,
}

impl BoxFace {
    pub const ALL: [BoxFace; 6] = [
        BoxFace::NegZ,
        BoxFace::PosZ,
        BoxFace::PosX,
        BoxFace::NegX,
        BoxFace::Top,
        BoxFace::Bottom,
    ];

    /// The face's four corners as the engine hands them to `0x414f60`: cell
    /// offsets in x and z, and whether each is at the box's top.
    pub fn corners(self) -> [(i32, i32, bool); 4] {
        match self {
            BoxFace::NegZ => [(0, 0, false), (1, 0, false), (1, 0, true), (0, 0, true)],
            BoxFace::PosZ => [(1, 1, false), (0, 1, false), (0, 1, true), (1, 1, true)],
            BoxFace::PosX => [(1, 0, false), (1, 1, false), (1, 1, true), (1, 0, true)],
            BoxFace::NegX => [(0, 1, false), (0, 0, false), (0, 0, true), (0, 1, true)],
            BoxFace::Top => [(0, 0, true), (1, 0, true), (1, 1, true), (0, 1, true)],
            BoxFace::Bottom => [(0, 0, false), (1, 0, false), (1, 1, false), (0, 1, false)],
        }
    }

    /// The way the face looks, which the box draw hands to `0x48a6a0` to
    /// light it by the sun (`0x41655a` and the five calls like it).
    pub fn normal(self) -> [i32; 3] {
        match self {
            BoxFace::NegZ => [0, 0, -1],
            BoxFace::PosZ => [0, 0, 1],
            BoxFace::PosX => [1, 0, 0],
            BoxFace::NegX => [-1, 0, 0],
            BoxFace::Top => [0, 1, 0],
            BoxFace::Bottom => [0, -1, 0],
        }
    }
}
/// Two per chamber: floor and ceiling.
pub const CHAMBER_TEXTURES: usize = 2;

/// A 128 x 128 plane of altitudes, scaled the way the engine scales that layer.
///
/// Signed, because [`Scale::Down`] layers are entirely negative.
#[derive(Debug, Clone)]
pub struct Altitudes {
    pub scale: Scale,
    pub values: Vec<i16>,
}

impl Altitudes {
    pub fn parse(data: &[u8], scale: Scale) -> Result<Altitudes> {
        if data.len() != CELLS {
            return Err(Error::WrongSize {
                what: "altitude layer",
                want: format!("{CELLS} bytes"),
                have: data.len(),
            });
        }
        Ok(Altitudes { scale, values: data.iter().map(|&b| scale.apply(b)).collect() })
    }

    /// The world wraps at 128 cells, so out-of-range indices are masked rather
    /// than rejected - exactly as `heightAtGrid` does with `and eax, 0x7f`.
    pub fn at(&self, x: i32, z: i32) -> i16 {
        self.values[(z as usize & (SIDE - 1)) * SIDE + (x as usize & (SIDE - 1))]
    }

    pub fn span(&self) -> (i16, i16) {
        let lo = self.values.iter().copied().min().unwrap_or(0);
        let hi = self.values.iter().copied().max().unwrap_or(0);
        (lo, hi)
    }
}

/// A terrain texture word: an index into the level's `.TEX` list in the low
/// twelve bits, and a UV orientation code in the top four.
///
/// The same word appears in `.CLR` for the ground, in `.CL0` and `.CL2` for a
/// box's six faces, and in `.CL1` for a chamber's two.
///
/// The split is proven by the data as well as by the code: across all 26
/// levels and 425,984 ground cells, `word & 0xfff` is always a valid index into
/// that level's texture list, while the raw `u16` is out of range 3,658 times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextureRef(pub u16);

impl TextureRef {
    /// Index into the level's `.TEX` list. `HELLBEND.EXE:0x413c20` resolves it
    /// as `textureTable[word & 0xfff]`, entries of 24 bytes at `0x753cd0`.
    pub fn index(self) -> u16 {
        self.0 & 0x0fff
    }

    /// The orientation code, 0 to 15.
    pub fn orientation(self) -> u8 {
        (self.0 >> 12) as u8
    }

    /// The texture coordinates of a quad's four corners after this word's
    /// orientation code, in the order the corners were given.
    ///
    /// Both of the engine's callers hand `0x413940` a quad whose corners start
    /// at `a (lo, hi)`, `b (hi, hi)`, `c (hi, lo)`, `d (lo, lo)` - the ground
    /// at `0x413c20`, whose corners are (x, z), (x+1, z), (x+1, z+1), (x, z+1),
    /// and every box face at `0x414f60`. So u runs with x and **v runs against
    /// z**. `lo` and `hi` are 2.0 and 254.0 in the 256-unit texture space, half
    /// a texel in from each edge of a 64-texel texture. Then `0x413940`:
    ///
    /// ```text
    /// r = code >> 2     if r != 0: a, b, c, d take the uvs of corners
    ///                   r, r+1, r+2, r+3 (mod 4) - a quarter turn per step
    /// code & 2          swaps a with b and d with c
    /// code & 1          swaps a with d and b with c
    /// ```
    pub fn corner_uvs(self, lo: f32, hi: f32) -> [(f32, f32); 4] {
        let base = [(lo, hi), (hi, hi), (hi, lo), (lo, lo)];
        let code = self.orientation();
        let r = (code >> 2) as usize;
        let mut uv = [base[r], base[(r + 1) & 3], base[(r + 2) & 3], base[(r + 3) & 3]];
        if code & 2 != 0 {
            uv.swap(0, 1);
            uv.swap(3, 2);
        }
        if code & 1 != 0 {
            uv.swap(0, 3);
            uv.swap(1, 2);
        }
        uv
    }

    /// Whether the texture is drawn in its authored orientation.
    pub fn is_upright(self) -> bool {
        self.orientation() == 0
    }
}

/// A 128 x 128 plane of `n` `u16` per cell.
///
/// The loader accepts a narrow form - one byte per value instead of two - and
/// picks it on file length. No shipped level uses it, but third-party data
/// might, so both are read here.
#[derive(Debug, Clone)]
pub struct Indices {
    pub per_cell: usize,
    pub values: Vec<u16>,
}

impl Indices {
    pub fn parse(data: &[u8], per_cell: usize, what: &'static str) -> Result<Indices> {
        let wide = CELLS * per_cell * 2;
        let narrow = CELLS * per_cell;
        let values = if data.len() == wide {
            (0..CELLS * per_cell).map(|i| u16_at(data, i * 2)).collect()
        } else if data.len() == narrow {
            data.iter().map(|&b| b as u16).collect()
        } else {
            return Err(Error::WrongSize {
                what,
                want: format!("{wide} or {narrow} bytes"),
                have: data.len(),
            });
        };
        Ok(Indices { per_cell, values })
    }

    pub fn at(&self, x: i32, z: i32, slot: usize) -> u16 {
        let cell = (z as usize & (SIDE - 1)) * SIDE + (x as usize & (SIDE - 1));
        self.values[cell * self.per_cell + slot]
    }

    /// The same value read as an index plus an orientation.
    pub fn texture_at(&self, x: i32, z: i32, slot: usize) -> TextureRef {
        TextureRef(self.at(x, z, slot))
    }
}

/// The four layer indices `heightAtGrid` and `groundTriangleInt` accept. There
/// is no layer 1: the engine's switch handles 0, 2 and 3 and falls through to a
/// fatal error otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// The ground surface. Array at `0x0073bcc0`, 6 bytes per cell, altitude
    /// at +0.
    Ground = 0,
    /// Box set A. Array at `0x00675fa0`, 18 bytes per cell. Only
    /// `intersectingBoxSurface` accepts it.
    BoxA = 1,
    /// The chamber floor. Array at `0x006bdfb0`, 12 bytes per cell, at +0.
    ChamberFloor = 2,
    /// The chamber ceiling. Same array, at +2.
    ChamberCeiling = 3,
    /// Box set B. Array at `0x006edfc0`, 18 bytes per cell. Also only
    /// `intersectingBoxSurface`.
    BoxB = 4,
}

impl Layer {
    /// The three layers `heightAtGrid`, `groundTriangleInt` and
    /// `groundTriangleMidpoint` accept. They reject 1 and 4 with a fatal error.
    pub const HEIGHT_QUERYABLE: [Layer; 3] =
        [Layer::Ground, Layer::ChamberFloor, Layer::ChamberCeiling];

    /// The two layers `intersectingBoxSurface` accepts. It rejects everything
    /// else with "intersectingBoxSurface: bad layer passed".
    pub const BOX_LAYERS: [Layer; 2] = [Layer::BoxA, Layer::BoxB];
}

/// One extruded layer: a bottom altitude, a top altitude and six textures.
#[derive(Debug, Clone)]
pub struct BoxLayer {
    pub bottom: Altitudes,
    pub top: Altitudes,
    pub textures: Indices,
}

/// The inverse: a floor, a ceiling and two textures.
#[derive(Debug, Clone)]
pub struct ChamberLayer {
    pub floor: Altitudes,
    pub ceiling: Altitudes,
    pub textures: Indices,
}

/// `DATA\<stem>.LTE`: the precomputed shading database, seven bytes per cell.
///
/// Not the same format as `FOG\<stem>.LTE`, which is the 16-row
/// [colour ramp](crate::colour::Ramp). The terrain loader opens this one
/// itself with `sprintf("%s.lte")` against the `data` directory, at
/// `HELLBEND.EXE:0x413460`, and the two files simply share an extension.
///
/// The loader reads the seven bytes in this order and scatters them into the
/// four cell arrays, which is what fixes both the order and the destinations:
///
/// ```text
/// ground[+4]    ground[+5]    two bytes, one per triangle half
/// boxA[+0x10]   one byte
/// chamber[+8]   chamber[+9]   chamber[+10]    three bytes
/// boxB[+0x10]   one byte
/// ```
///
/// It is a cache. When the file is missing the engine says so - "No .LTE file.
/// Shading database for the last time during loading.  Phew!" - and computes
/// the same values at `0x41c5d0` from the terrain and a light direction.
#[derive(Debug, Clone)]
pub struct Shading {
    /// Two per cell, one little-endian word: the intensity of the grid point at
    /// the cell's origin in the low byte, and in bit 8 a flag that makes the
    /// renderer use the level's ambient instead (`0x414e0b`).
    pub ground: Vec<[u8; 2]>,
    /// One per cell.
    pub box_a: Vec<u8>,
    /// Three per cell: the chamber floor's intensity at the cell's origin,
    /// the ceiling's, and two flags - see [`Shading::chamber_intensity`].
    pub chambers: Vec<[u8; 3]>,
    /// One per cell.
    pub box_b: Vec<u8>,
}

impl Shading {
    /// Seven bytes per cell.
    pub const PER_CELL: usize = 7;
    pub const BYTES: usize = CELLS * Self::PER_CELL;

    pub fn parse(data: &[u8]) -> Result<Shading> {
        if data.len() != Self::BYTES {
            return Err(Error::WrongSize {
                what: "LTE shading database",
                want: format!("{} bytes", Self::BYTES),
                have: data.len(),
            });
        }
        let mut out = Shading {
            ground: Vec::with_capacity(CELLS),
            box_a: Vec::with_capacity(CELLS),
            chambers: Vec::with_capacity(CELLS),
            box_b: Vec::with_capacity(CELLS),
        };
        for cell in data.chunks_exact(Self::PER_CELL) {
            out.ground.push([cell[0], cell[1]]);
            out.box_a.push(cell[2]);
            out.chambers.push([cell[3], cell[4], cell[5]]);
            out.box_b.push(cell[6]);
        }
        Ok(out)
    }

    pub fn ground_at(&self, x: i32, z: i32) -> [u8; 2] {
        self.ground[(z as usize & (SIDE - 1)) * SIDE + (x as usize & (SIDE - 1))]
    }

    /// The ground cell's two bytes read as one little-endian value.
    ///
    /// The range is 0 to 511 in every level, so it is nine bits: an intensity
    /// in the low byte and a single flag in bit 8. The intensity's ceiling is
    /// 255 everywhere and its floor is per level - 64 in `HOTH`, 96 in `ROID`,
    /// 160 in `FLOAT` - which reads as the level's ambient minimum.
    pub fn ground_word(&self, x: i32, z: i32) -> u16 {
        let [lo, hi] = self.ground_at(x, z);
        lo as u16 | ((hi as u16) << 8)
    }

    /// The low byte: 0 is dark, 255 is full brightness.
    pub fn ground_intensity(&self, x: i32, z: i32) -> u8 {
        self.ground_at(x, z)[0]
    }

    /// Bit 8, whatever it means.
    pub fn ground_flag(&self, x: i32, z: i32) -> bool {
        self.ground_at(x, z)[1] & 1 != 0
    }

    pub fn chamber_at(&self, x: i32, z: i32) -> [u8; 3] {
        self.chambers[(z as usize & (SIDE - 1)) * SIDE + (x as usize & (SIDE - 1))]
    }

    /// The chamber floor's or ceiling's intensity at the grid point `(x, z)`,
    /// or `None` where it takes the level's ambient instead. The chamber
    /// draws light each corner from its own cell as the ground's does: the
    /// floor from the first byte unless the third's bit 0 is set (`0x418fe4`),
    /// the ceiling from the second unless bit 1 is (`0x41944c`).
    pub fn chamber_intensity(&self, x: i32, z: i32, ceiling: bool) -> Option<u8> {
        let [floor, roof, flags] = self.chamber_at(x, z);
        match ceiling {
            false => (flags & 1 == 0).then_some(floor),
            true => (flags & 2 == 0).then_some(roof),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Terrain {
    /// From `.RAW`.
    pub ground: Altitudes,
    /// From `.CLR`, one value per cell.
    pub colour: Indices,
    /// From `.RA0`, `.RA1`, `.CL0`.
    pub boxes_a: BoxLayer,
    /// From `.RA2`, `.RA3`, `.CL1`.
    pub chambers: ChamberLayer,
    /// From `.RA4`, `.RA5`, `.CL2`.
    pub boxes_b: BoxLayer,
    /// From `DATA\<stem>.LTE`. Absent when the level ships without one, in
    /// which case the engine computes it at load time.
    pub shading: Option<Shading>,
}

/// The thirteen files, in load order, as extension suffixes appended to the
/// level stem. The engine builds each name with `sprintf("%s.ra0", stem)`.
pub const EXTENSIONS: [&str; 13] = [
    "raw", "clr", "ra0", "ra1", "cl0", "ra2", "ra3", "cl1", "ra4", "ra5", "cl2",
    // Named on the .LVL rather than derived, but loaded with the terrain.
    "tex", "act",
];

impl Terrain {
    /// `fetch` resolves an extension to that file's bytes - normally a closure
    /// over a `Pod` looking in the `data` directory.
    pub fn load<F>(mut fetch: F) -> Result<Terrain>
    where
        F: FnMut(&str) -> Option<Vec<u8>>,
    {
        let mut get = |ext: &'static str, what: &'static str| -> Result<Vec<u8>> {
            fetch(ext).ok_or(Error::WrongSize { what, want: "a file".into(), have: 0 })
        };
        Ok(Terrain {
            ground: Altitudes::parse(&get("raw", "ground altitude")?, Scale::Up)?,
            colour: Indices::parse(&get("clr", "ground colour")?, 1, "ground colour")?,
            boxes_a: BoxLayer {
                bottom: Altitudes::parse(&get("ra0", "box A bottom")?, Scale::Up)?,
                top: Altitudes::parse(&get("ra1", "box A top")?, Scale::Up)?,
                textures: Indices::parse(
                    &get("cl0", "box A textures")?,
                    BOX_TEXTURES,
                    "box A textures",
                )?,
            },
            chambers: ChamberLayer {
                floor: Altitudes::parse(&get("ra2", "chamber floor")?, Scale::Down)?,
                ceiling: Altitudes::parse(&get("ra3", "chamber ceiling")?, Scale::Down)?,
                textures: Indices::parse(
                    &get("cl1", "chamber textures")?,
                    CHAMBER_TEXTURES,
                    "chamber textures",
                )?,
            },
            boxes_b: BoxLayer {
                bottom: Altitudes::parse(&get("ra4", "box B bottom")?, Scale::Down)?,
                top: Altitudes::parse(&get("ra5", "box B top")?, Scale::Down)?,
                textures: Indices::parse(
                    &get("cl2", "box B textures")?,
                    BOX_TEXTURES,
                    "box B textures",
                )?,
            },
            shading: match fetch("lte") {
                Some(bytes) => Some(Shading::parse(&bytes)?),
                None => None,
            },
        })
    }
}

#[cfg(test)]
mod orientation {
    use super::TextureRef;

    const LO: f32 = 2.0;
    const HI: f32 = 254.0;

    fn uvs(code: u16) -> [(f32, f32); 4] {
        TextureRef(code << 12).corner_uvs(LO, HI)
    }

    #[test]
    fn code_zero_leaves_v_running_against_z() {
        // a (x, z), b (x+1, z), c (x+1, z+1), d (x, z+1).
        assert_eq!(uvs(0), [(LO, HI), (HI, HI), (HI, LO), (LO, LO)]);
    }

    #[test]
    fn the_top_two_bits_turn_a_quarter_each() {
        assert_eq!(uvs(4), [(HI, HI), (HI, LO), (LO, LO), (LO, HI)]);
        assert_eq!(uvs(8), [(HI, LO), (LO, LO), (LO, HI), (HI, HI)]);
        assert_eq!(uvs(12), [(LO, LO), (LO, HI), (HI, HI), (HI, LO)]);
    }

    #[test]
    fn bit_one_mirrors_u_and_bit_zero_mirrors_v() {
        // Swapping a with b and d with c exchanges u at the same v.
        assert_eq!(uvs(2), [(HI, HI), (LO, HI), (LO, LO), (HI, LO)]);
        // Swapping a with d and b with c exchanges v at the same u.
        assert_eq!(uvs(1), [(LO, LO), (HI, LO), (HI, HI), (LO, HI)]);
        // Both is a half turn, the same as rotation 2.
        assert_eq!(uvs(3), uvs(8));
    }

    #[test]
    fn every_code_is_one_of_the_eight_symmetries_of_a_square() {
        use std::collections::HashSet;
        let distinct: HashSet<Vec<(i32, i32)>> = (0..16)
            .map(|c| uvs(c).iter().map(|&(u, v)| (u as i32, v as i32)).collect())
            .collect();
        assert_eq!(distinct.len(), 8);
    }
}
