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
/// Measured, not transcribed. Over eight levels, taking every box cell whose
/// four side slots are three-of-a-kind plus one odd one out and that has
/// exactly one boxless neighbour: the odd slot is 0 or 1 when the exposed
/// neighbour is in z (160 and 129 cases, against 4 in x), and 2 or 3 when it is
/// in x (145 and 141 cases, against 0 in z). So the pairing of slots to axes is
/// settled; which member of each pair faces which way is not - see
/// [`BoxFace::AXIS_ONLY`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxFace {
    /// Slot 0. One of the two z-facing sides.
    Z0 = 0,
    /// Slot 1. The other z-facing side.
    Z1 = 1,
    /// Slot 2. One of the two x-facing sides.
    X0 = 2,
    /// Slot 3. The other x-facing side.
    X1 = 3,
    /// Slot 4. The top. Carries by far the most variety - 80 distinct values
    /// across `FLOAT`'s 1,142 boxes, against 18 for the bottom.
    Top = 4,
    /// Slot 5. The bottom, and the least varied.
    Bottom = 5,
}

impl BoxFace {
    /// The signs within each axis pair are still unknown, so a renderer that
    /// needs them must decide and say so. Kept as a constant rather than a
    /// comment so it shows up in a search for what is not yet settled.
    pub const AXIS_ONLY: bool = true;

    pub const ALL: [BoxFace; 6] = [
        BoxFace::Z0,
        BoxFace::Z1,
        BoxFace::X0,
        BoxFace::X1,
        BoxFace::Top,
        BoxFace::Bottom,
    ];
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
    ///
    /// Consumed in exactly one place, `0x413940`, which takes a cell's four
    /// corners and permutes their texture coordinates in the table at
    /// `0x59d36c`:
    ///
    /// ```text
    /// (code >> 2)     tested first, a 2-bit selector
    /// code & 2        swaps corner 0 with 1 and corner 3 with 2
    /// code & 1        a further swap
    /// ```
    ///
    /// So it is a mirror-and-rotate applied to the cell's UVs. Which bit is
    /// which mirror depends on the corner argument order at that call site,
    /// which is not yet pinned - see the Unknown section of
    /// `docs/formats/terrain.md`.
    pub fn orientation(self) -> u8 {
        (self.0 >> 12) as u8
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
    /// Two per cell, one per triangle half.
    pub ground: Vec<[u8; 2]>,
    /// One per cell.
    pub box_a: Vec<u8>,
    /// Three per cell.
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
