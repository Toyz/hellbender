//! Cell geometry: coordinates, wrapping, and the triangle split.

use hb_formats::terrain::{Altitudes, BoxFace, Layer, Terrain, CELL_SIZE, SIDE};

/// The grid wraps rather than clamps. `heightAtGrid` masks both indices with
/// `and eax, 0x7f` before touching the array, and
/// `groundTriangleMidpoint` reaches a cell's world origin with
/// `shl 25` then `sar 6`, which is the same mask followed by a shift of 19.
pub const WRAP: i32 = SIDE as i32 - 1;

/// The world is 128 cells of 8.0 units: 1024.0 units square.
pub const WORLD_SIZE: i64 = SIDE as i64 * CELL_SIZE as i64;

/// A cell index pair. Always in range: constructing one wraps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub x: i32,
    pub z: i32,
}

impl Cell {
    pub fn new(x: i32, z: i32) -> Cell {
        Cell { x: x & WRAP, z: z & WRAP }
    }

    /// The cell containing a world position in 16.16 fixed point.
    ///
    /// An arithmetic shift then a mask, which is what `heightAtGrid` and
    /// `groundTriangleMidpoint` do - the latter as
    /// `(p & 0x3f80000) >> 19`, the middle seven bits. Negative coordinates
    /// fold onto the upper half of the grid, which is why the world wraps.
    pub fn containing(x: i32, z: i32) -> Cell {
        Cell::new(x >> 19, z >> 19)
    }

    /// The cell's origin corner in 16.16 world units, taking the index at face
    /// value. Cells 0 to 127 map to 0 to +1016 units.
    ///
    /// Use this when positions only have to be consistent with each other, as
    /// in a renderer that places the camera the same way. Use
    /// [`Cell::signed_origin`] when a position has to agree with the
    /// coordinates in a `.CRS` or `.NAV` file.
    pub fn origin(self) -> (i32, i32) {
        (self.x << 19, self.z << 19)
    }

    /// The cell's origin corner in the world's own signed coordinates.
    ///
    /// Course points run from about -511.3 to +511.8 units in x and z across
    /// all 26 levels, so the world is 1024 units square and centred on the
    /// origin rather than starting at it. A cell index is the middle seven bits
    /// of a coordinate, which folds -512..+512 onto 0..127: cells 0 to 63 are
    /// the positive half and 64 to 127 the negative one.
    pub fn signed_origin(self) -> (i32, i32) {
        let fold = |c: i32| (((c + 64) & WRAP) - 64) << 19;
        (fold(self.x), fold(self.z))
    }

    pub fn index(self) -> usize {
        self.z as usize * SIDE + self.x as usize
    }

    /// Which way the cell's diagonal runs. The engine tests `(x ^ z) & 1`.
    pub fn diagonal(self) -> Diagonal {
        if (self.x ^ self.z) & 1 != 0 {
            Diagonal::Anti
        } else {
            Diagonal::Main
        }
    }
}

/// A cell is two triangles, and which pair of corners the diagonal joins
/// alternates across the grid like a checkerboard. Nothing in the data says
/// so; the engine derives it from the cell indices alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diagonal {
    /// `(x ^ z) & 1 == 0`. The diagonal joins the origin corner to the far
    /// corner, so the halves' sample points are at (2/3, 1/3) and (1/3, 2/3) -
    /// the two centroids either side of that line.
    Main,
    /// `(x ^ z) & 1 == 1`. The diagonal joins the other two corners, and the
    /// sample points are at (1/3, 1/3) and (2/3, 2/3).
    Anti,
}

/// Which of a cell's two triangles. The engine passes this as an `int` that it
/// only tests against zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Half {
    First,
    Second,
}

/// A cell corner, named by which way it lies from the origin corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    /// (x, z)
    Origin,
    /// (x + 1, z)
    X,
    /// (x, z + 1)
    Z,
    /// (x + 1, z + 1)
    Far,
}

impl Corner {
    pub fn offset(self) -> (i32, i32) {
        match self {
            Corner::Origin => (0, 0),
            Corner::X => (1, 0),
            Corner::Z => (0, 1),
            Corner::Far => (1, 1),
        }
    }
}

/// The three corners of one half of a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Triangle {
    pub cell: Cell,
    pub half: Half,
    pub corners: [Corner; 3],
}

/// The two fractions the engine adds to a cell's origin to reach a triangle's
/// sample point, as 16.16 multipliers of the cell size.
///
/// `LOW` is 1/3 to within a rounding. `HIGH` is 0.670791, not the 0.666667 a
/// centroid wants - it is 0xABB9 where 2/3 would be 0xAAAB. The engine ships
/// that constant and the port keeps it, because a sample point that drifts
/// 0.4% of a cell toward one corner changes which triangle a borderline query
/// lands in.
pub const SAMPLE_LOW: i32 = 0x5555;
pub const SAMPLE_HIGH: i32 = 0xABB9;

/// The sample point of one half of a cell, in 16.16 world units.
///
/// `groundTriangleMidpoint`, `0x428900`. The four cases are the cross product
/// of the cell's parity and the half.
pub fn sample_point(cell: Cell, half: Half) -> (i32, i32) {
    let (ox, oz) = cell.origin();
    let (fx, fz) = match (cell.diagonal(), half) {
        (Diagonal::Anti, Half::First) => (SAMPLE_HIGH, SAMPLE_HIGH),
        (Diagonal::Anti, Half::Second) => (SAMPLE_LOW, SAMPLE_LOW),
        (Diagonal::Main, Half::First) => (SAMPLE_LOW, SAMPLE_HIGH),
        (Diagonal::Main, Half::Second) => (SAMPLE_HIGH, SAMPLE_LOW),
    };
    (ox + scale(fx), oz + scale(fz))
}

/// `(CELL_SIZE * fraction) >> 16`, the engine's `imul` plus `shrd ..., 0x10`.
fn scale(fraction: i32) -> i32 {
    ((CELL_SIZE as i64 * fraction as i64) >> 16) as i32
}

/// The three corners of a half.
///
/// Derived from the sample points `0x428900` produces, and then confirmed
/// independently: the ground normal routine at `0x41b0b0` reads three corner
/// altitudes per case, and the corners it reads are exactly these. See
/// [`half_containing`] for the test it uses to choose between them.
pub fn triangle(cell: Cell, half: Half) -> Triangle {
    let corners = match (cell.diagonal(), half) {
        // Anti diagonal, joining X to Z. Centroids (2/3, 2/3) and (1/3, 1/3).
        (Diagonal::Anti, Half::First) => [Corner::X, Corner::Far, Corner::Z],
        (Diagonal::Anti, Half::Second) => [Corner::Origin, Corner::X, Corner::Z],
        // Main diagonal, joining Origin to Far. Centroids (1/3, 2/3), (2/3, 1/3).
        (Diagonal::Main, Half::First) => [Corner::Origin, Corner::Z, Corner::Far],
        (Diagonal::Main, Half::Second) => [Corner::Origin, Corner::X, Corner::Far],
    };
    Triangle { cell, half, corners }
}

/// The centroid of a triangle's corners, in the same 16.16 units as
/// [`sample_point`]. Used to check the corner sets against the constants the
/// engine actually uses.
pub fn centroid(tri: Triangle) -> (i32, i32) {
    let (ox, oz) = tri.cell.origin();
    let (mut sx, mut sz) = (0i64, 0i64);
    for corner in tri.corners {
        let (dx, dz) = corner.offset();
        sx += dx as i64 * CELL_SIZE as i64;
        sz += dz as i64 * CELL_SIZE as i64;
    }
    (ox + (sx / 3) as i32, oz + (sz / 3) as i32)
}

/// A position's fraction across its cell, 0 to 0xffff.
///
/// `0x41b0b0` computes it as `(p & 0x7ffff) * 0x10000 / 0x80000`, which is the
/// low 19 bits scaled to 16 bits.
pub fn cell_fraction(p: i32) -> i32 {
    (p & (CELL_SIZE - 1)) >> 3
}

/// One whole cell as a fraction: the right-hand side of the anti-diagonal test.
pub const FRACTION_ONE: i32 = 0x1_0000;

/// Which half of its cell a world position falls in.
///
/// Transcribed from `0x41b0b0`, which branches on the cell's parity and then
/// on the two fractions:
///
/// ```text
/// parity 0    fz <  fx        the main diagonal, fx == fz
/// parity 1    fx + fz < 1.0   the anti diagonal
/// ```
pub fn half_containing(x: i32, z: i32) -> Half {
    let (fx, fz) = (cell_fraction(x), cell_fraction(z));
    match Cell::containing(x, z).diagonal() {
        Diagonal::Main if fz < fx => Half::Second,
        Diagonal::Main => Half::First,
        Diagonal::Anti if FRACTION_ONE - fx > fz => Half::Second,
        Diagonal::Anti => Half::First,
    }
}

/// The triangle a world position falls in.
pub fn triangle_containing(x: i32, z: i32) -> Triangle {
    triangle(Cell::containing(x, z), half_containing(x, z))
}

/// The terrain, with the queries the engine's own diagnostics name.
pub struct Grid<'a> {
    pub terrain: &'a Terrain,
}

impl<'a> Grid<'a> {
    pub fn new(terrain: &'a Terrain) -> Grid<'a> {
        Grid { terrain }
    }

    fn layer(&self, layer: Layer) -> Option<&Altitudes> {
        Some(match layer {
            Layer::Ground => &self.terrain.ground,
            Layer::ChamberFloor => &self.terrain.chambers.floor,
            Layer::ChamberCeiling => &self.terrain.chambers.ceiling,
            // The engine's height queries reject the box layers outright, with
            // "heightAtGrid: bad value passed for parameter layer". A box is
            // not a height field: it has a bottom and a top.
            Layer::BoxA | Layer::BoxB => return None,
        })
    }

    /// `heightAtGrid`, `0x428c40`. The stored altitude shifted left by 8, so
    /// the result is the byte scaled by 2^15 - in 16.16, half a unit per step
    /// and 127.5 units from the bottom of the range to the top.
    ///
    /// `None` for the box layers, which the engine treats as a fatal error.
    pub fn height_at_grid(&self, layer: Layer, x: i32, z: i32) -> Option<i32> {
        Some((self.layer(layer)?.at(x, z) as i32) << 8)
    }

    /// The altitude at each corner of one half of a cell, in the same units as
    /// [`Grid::height_at_grid`].
    pub fn triangle_heights(&self, layer: Layer, tri: Triangle) -> Option<[i32; 3]> {
        let mut out = [0; 3];
        for (slot, corner) in out.iter_mut().zip(tri.corners) {
            let (dx, dz) = corner.offset();
            *slot = self.height_at_grid(layer, tri.cell.x + dx, tri.cell.z + dz)?;
        }
        Some(out)
    }

    /// `intersectingBoxSurface`, `0x4294c0`: a box cell's bottom and top, in
    /// the same units as [`Grid::height_at_grid`]. `None` for the layers that
    /// are not box layers.
    ///
    /// A box spans exactly one cell in x and z, so with these two values it is
    /// fully determined: the routine builds its corners from `index << 19` and
    /// `(index + 1) << 19`.
    pub fn box_span(&self, layer: Layer, cell: Cell) -> Option<(i32, i32)> {
        let set = match layer {
            Layer::BoxA => &self.terrain.boxes_a,
            Layer::BoxB => &self.terrain.boxes_b,
            _ => return None,
        };
        Some((
            (set.bottom.at(cell.x, cell.z) as i32) << 8,
            (set.top.at(cell.x, cell.z) as i32) << 8,
        ))
    }

    /// The texture index on one face of a box cell.
    pub fn box_texture(&self, layer: Layer, cell: Cell, face: BoxFace) -> Option<u16> {
        let set = match layer {
            Layer::BoxA => &self.terrain.boxes_a,
            Layer::BoxB => &self.terrain.boxes_b,
            _ => return None,
        };
        Some(set.textures.at(cell.x, cell.z, face as usize))
    }

    /// The surface normal of one triangle, before normalisation.
    ///
    /// `0x41b0b0` builds `(h0 - h1, CELL_SIZE, h2 - h3)` from two pairs of
    /// corner altitudes and hands it to the vector normaliser at `0x42bb80`.
    /// Which pairs depends on the case, and in every case they are the two
    /// axis-aligned edges of the triangle:
    ///
    /// ```text
    /// Main,  Second  {Origin, X, Far}   (h(x,z)   - h(x+1,z),   h(x+1,z) - h(x+1,z+1))
    /// Anti,  Second  {Origin, X, Z}     (h(x,z)   - h(x+1,z),   h(x,z)   - h(x,z+1))
    /// ```
    ///
    /// The y component is the cell size, so the vector is a height gradient per
    /// cell and +y is up.
    pub fn triangle_normal(&self, layer: Layer, tri: Triangle) -> Option<[i32; 3]> {
        let h = |dx: i32, dz: i32| {
            self.height_at_grid(layer, tri.cell.x + dx, tri.cell.z + dz)
        };
        // The two axis-aligned edges of the triangle, as corner pairs. Each
        // triangle has exactly one edge along x and one along z; the diagonal
        // edge is the one that is left out.
        let along_x = Self::edge(tri, |c| c.offset().1);
        let along_z = Self::edge(tri, |c| c.offset().0);
        let (ax, bx) = along_x?;
        let (az, bz) = along_z?;
        Some([
            h(ax.offset().0, ax.offset().1)? - h(bx.offset().0, bx.offset().1)?,
            CELL_SIZE,
            h(az.offset().0, az.offset().1)? - h(bz.offset().0, bz.offset().1)?,
        ])
    }

    /// The triangle's edge whose endpoints agree on `key` - the x-aligned edge
    /// when `key` reads the z offset, and the z-aligned edge when it reads x.
    /// Ordered so the lower coordinate comes first, which is the order the
    /// engine subtracts in.
    fn edge(tri: Triangle, key: fn(Corner) -> i32) -> Option<(Corner, Corner)> {
        let c = tri.corners;
        for (a, b) in [(c[0], c[1]), (c[1], c[2]), (c[2], c[0])] {
            if key(a) == key(b) {
                let (a, b) = if a.offset() < b.offset() { (a, b) } else { (b, a) };
                return Some((a, b));
            }
        }
        None
    }

    /// Whether a cell carries a box in the given set.
    ///
    /// A box is absent when its bottom and top are equal, not when either is
    /// zero. That matters for box set B, whose altitudes are measured downward
    /// so that its "nothing here" byte of 255 becomes an altitude of zero and
    /// its byte 0 becomes -32,640: testing the top against zero calls almost
    /// every cell a box. In `FLOAT`, bottom equals top in 15,242 cells for set
    /// A and 15,308 for set B, which is the right order of magnitude for a
    /// level with a few hundred structures.
    pub fn has_box(&self, layer: Layer, cell: Cell) -> bool {
        match self.box_span(layer, cell) {
            Some((bottom, top)) => bottom != top,
            None => false,
        }
    }

    pub fn has_box_a(&self, cell: Cell) -> bool {
        self.has_box(Layer::BoxA, cell)
    }

    pub fn has_box_b(&self, cell: Cell) -> bool {
        self.has_box(Layer::BoxB, cell)
    }

    /// Whether the cell has a chamber - a floor and ceiling that are not the
    /// same altitude.
    pub fn has_chamber(&self, cell: Cell) -> bool {
        self.terrain.chambers.floor.at(cell.x, cell.z)
            != self.terrain.chambers.ceiling.at(cell.x, cell.z)
    }
}
