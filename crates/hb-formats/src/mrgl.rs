//! The `.BIN` model: a flat stream of variable-length MRGL nodes.
//!
//! The size table is transcribed from the jump table at `HELLBEND.EXE:0x47442c`
//! and is what lets a reader step over a node it does not understand. It walks
//! all 342 shipped models to the byte.

use crate::{cstr, i32_at, u32_at, Error, Result};

/// End of the node stream.
pub const END: u32 = 0x00;
/// Vertex list: count at +8, then three `i32` per vertex.
pub const VERTEX_LIST: u32 = 0x02;
/// Material: a 16-byte texture name at +8.
pub const MATERIAL: u32 = 0x0D;
/// Textured polygon: corner count at +4, a normal and plane constant, then
/// 12 bytes per corner from +0x18.
pub const POLYGON: u32 = 0x0E;
/// Start of a mesh. Also one of the two node types a model may begin with.
pub const MESH_START: u32 = 0x14;
/// Group: up to sixteen child model names. The other legal first node.
pub const GROUP: u32 = 0x20;

/// A group node's fixed capacity. Its 344 bytes are 24 of header, sixteen
/// 16-byte name slots, sixteen runtime pointer slots, and nothing else.
pub const GROUP_CHILDREN: usize = 16;
/// Where the name slots start inside a group node.
pub const GROUP_NAMES: usize = 0x18;
/// Where the runtime pointer slots start. The engine's free routine at
/// `0x473e70` walks `[node + 0x118 + 4 * i]`, which is what fixes this.
pub const GROUP_POINTERS: usize = 0x118;
/// Animated model, produced by the `.TXT` parser rather than read from a file.
pub const ANIMATED: u32 = 0x26;

/// The most common polygon node by far - 98% of all nodes in the shipped
/// models - with the same size formula and, as far as every measurement goes,
/// the same payload as [`POLYGON`]. What distinguishes the two is not known;
/// the binary has separate flat and Gouraud shading paths, which is a guess at
/// why, not evidence for it.
pub const POLYGON_ALT: u32 = 0x18;

/// A flat colour, 12 bytes: a zero `i32` at +4 and a palette index at +8.
///
/// 267 of the 270 in both archives are between 9 and 239, which is exactly the
/// [shadeable palette range](crate::colour) - none falls in the reserved 240 to
/// 255. The other three are 270, 360 and 482, whose low bytes are 14, 104 and
/// 226 and whose bit 8 is set, so the field looks like an index with a flag
/// above it, the same shape as a terrain texture word.
pub const FLAT_COLOUR: u32 = 0x17;
/// Texel coordinates for a run of vertices: `+4` the first vertex, `+8` the
/// count, then `(u, v)` pairs in 16.16. The draw handler (`0x4567c0`) writes
/// them into the transformed-vertex array beside each vertex, so a polygon
/// that names vertices by index takes its texture coordinates from them.
pub const VERTEX_TEXELS: u32 = 0x04;
/// An indexed polygon: `+4` the corner count, `+8` a 16.16 normal, `+0x14`
/// the plane constant, `+0x18` the vertex indices. Drawn textured with the
/// vertices' texels (`0x457bb0`), after the same back-face test as the other
/// polygons - skipped for a zero normal, which no shipped one has. The
/// powerups' single quad faces -z, and the engine turns the model toward the
/// eye to show it.
pub const INDEXED_POLYGON: u32 = 0x0F;
/// A material that cycles through frames (`0x458fd0`): `+8` the frame count,
/// `+0xc` the current frame, `+0x10` seconds a frame in 16.16, `+0x14` the
/// time run, `+0x18` a changed flag, then from `+0x1c` one 32-byte record per
/// frame - a texture name, and at `+0x10` a sound played when the frame comes
/// round.
pub const FLIPBOOK: u32 = 0x1D;
/// The colour the flat polygons that follow are filled with (`0x457850`
/// stores `+4` at `0x5b3888`): 0 and up picks a band of the palette from the
/// tables at `0x50c4e8` and `0x50c528`, the polygon's light choosing within
/// it; below 0 is a palette index outright.
pub const SHADE_COLOUR: u32 = 0x0A;
/// An indexed polygon ([`INDEXED_POLYGON`]'s layout) filled flat with the
/// current [`SHADE_COLOUR`] at its light (`0x458a00`).
pub const FLAT_POLYGON: u32 = 0x19;
/// The same again (`0x456810`), which the dispatch table at `0x50c448` gives
/// node 5: the same back-face test, the same band lookup on
/// [`SHADE_COLOUR`], and a fill colour replicated into all four bytes of a
/// word - a solid fill written a word at a time. 41 nodes in the archives
/// and one of them is the reticle.
pub const SOLID_POLYGON: u32 = 0x05;
/// And once more (`0x456930`), which is node 6: the same back-face test and
/// the same corner list, but no light and no band lookup - it never reads
/// [`SHADE_COLOUR`]. What it changes is the span routine at `0x59d10c`.
/// Node 5 leaves `0x4a76ac`, which fills the span with one colour; node 6
/// leaves `0x4a723f`, which interpolates a shade across it and takes every
/// pixel through the remap on its own. So node 5 is flat and node 6 is
/// shaded - see `docs/engine/rasteriser.md`. 24 nodes in the archives, and
/// this port draws them flat.
pub const SHADED_POLYGON: u32 = 0x06;

/// The palette bands a [`SHADE_COLOUR`] of 0 to 15 picks: the darkest and
/// brightest index of each (`0x50c4e8`, `0x50c528`). A polygon's light,
/// 0 to 1, goes from one to the other.
pub const BANDS: [(u8, u8); 16] = [
    (0x00, 0x1f),
    (0x00, 0x0f),
    (0x20, 0x3f),
    (0x40, 0x5f),
    (0x60, 0x7f),
    (0x80, 0x8f),
    (0x90, 0x9f),
    (0xa0, 0xaf),
    (0xb0, 0xbf),
    (0xc0, 0xcf),
    (0xd0, 0xdf),
    (0xe0, 0xef),
    (0xf0, 0xf0),
    (0xff, 0xff),
    (0x00, 0x00),
    (0xd8, 0xd8),
];

/// The index a [`SHADE_COLOUR`] fills with at a light of 0 to 0xffff
/// (`0x458ae0`).
pub fn shade_colour(colour: i32, light: i32) -> u8 {
    if colour < 0 {
        return colour.unsigned_abs() as u8;
    }
    let (lo, hi) = BANDS[colour as usize & 15];
    let (lo, hi) = (lo as i64, hi as i64);
    (lo + (((hi - lo) * light as i64) >> 16)) as u8
}

/// Every node type that carries a polygon payload.
pub const POLYGON_KINDS: [u32; 5] = [0x0E, 0x11, 0x18, 0x1E, 0x22];

/// Byte length of a node, or `None` if the type is not defined.
///
/// `at` must point at the node's type word. Nodes whose length depends on a
/// count read that count out of the node itself.
pub fn node_size(data: &[u8], at: usize) -> Option<usize> {
    let word = |off: usize| -> usize {
        if at + off + 4 <= data.len() {
            i32_at(data, at + off).max(0) as usize
        } else {
            0
        }
    };
    let kind = u32_at(data, at);
    Some(match kind {
        0x00 => 4,
        0x01 => 16,
        0x02 => 12 + 12 * word(8),
        0x03 => 12 + 4 * word(8),
        0x04 => 12 + 8 * word(8),
        0x05 | 0x06 | 0x07 | 0x08 | 0x0F | 0x15 | 0x19 | 0x1A | 0x1B | 0x21 => 24 + 4 * word(4),
        0x09 => 32,
        0x0A | 0x0B => 8,
        0x0C => 28,
        0x0D => 24,
        0x0E | 0x11 | 0x18 | 0x1E | 0x22 => 24 + 12 * word(4),
        0x10 => 20,
        0x12 | 0x14 => 8,
        0x16 => 8 + 4 * word(4),
        0x17 => 12,
        0x1D => 28 + 32 * word(8),
        0x1F => 12 + 4 * word(8),
        0x20 => 344,
        0x26 => 15_712,
        // 0x13, 0x1c, 0x23, 0x24 and 0x25 reach the engine's "Bad MRGL type"
        // arm: holes in the numbering, not nodes.
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy)]
pub struct Node {
    pub offset: usize,
    pub kind: u32,
    pub size: usize,
}

/// Steps the node stream. Stops after the type-0 node, or at the first
/// undefined type.
pub struct Walk<'a> {
    data: &'a [u8],
    at: usize,
    done: bool,
}

impl<'a> Walk<'a> {
    pub fn new(data: &'a [u8]) -> Walk<'a> {
        Walk { data, at: 0, done: false }
    }
}

impl Iterator for Walk<'_> {
    type Item = Result<Node>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done || self.at + 4 > self.data.len() {
            return None;
        }
        let at = self.at;
        let kind = u32_at(self.data, at);
        let Some(size) = node_size(self.data, at) else {
            self.done = true;
            return Some(Err(Error::BadTag { what: "MRGL", tag: kind, at }));
        };
        if at + size > self.data.len() {
            self.done = true;
            return Some(Err(Error::Truncated {
                what: "MRGL",
                at,
                need: size,
                have: self.data.len() - at,
            }));
        }
        self.at += size;
        if kind == END {
            self.done = true;
        }
        Some(Ok(Node { offset: at, kind, size }))
    }
}

/// Model coordinates are normalised: 235 of the 238 `.BIN` models in GAME.POD
/// have a maximum absolute component of exactly 16,383 or 16,384 and none
/// exceeds 16,384. So model space is 2.14 fixed point, spanning -1.0 to +1.0,
/// and a placement's `scale` is the object's half-extent in world units.
pub const MODEL_ONE: i32 = 16_384;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Vertex {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Vertex {
    /// This vertex as a 16.16 world offset from the object's origin, for an
    /// object placed at the given 16.16 scale.
    pub fn world(self, scale: i32) -> [i32; 3] {
        let one = |v: i32| ((v as i64 * scale as i64) >> 14) as i32;
        [one(self.x), one(self.y), one(self.z)]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Corner {
    pub vertex: u32,
    /// Texture coordinate, stored as a texel shifted left 16. Every corner of
    /// every polygon in both archives has both coordinates in 0..=255, so the
    /// texture space is 256 units wide whatever the texture's real size is.
    pub u: i32,
    pub v: i32,
}

impl Corner {
    /// The texel, 0 to 255.
    pub fn texel(self) -> (u8, u8) {
        ((self.u >> 16) as u8, (self.v >> 16) as u8)
    }
}

#[derive(Debug, Clone)]
pub struct Polygon {
    /// 0x0e or one of its variants. 0x18 is 98% of them.
    pub kind: u32,
    /// The outward face normal, 16.16, unit length in 33,648 of the 33,728
    /// polygons in both archives.
    pub normal: [i32; 3],
    /// The plane constant: the plane is `dot(normal, p) == plane`, 16.16.
    /// Matches `dot(normal, centroid)` to within 0.2% for 96% of polygons.
    pub plane: i32,
    /// Three or four. Nothing in either archive has any other count.
    pub corners: Vec<Corner>,
    /// The last [`FLAT_COLOUR`] node seen before this polygon, if any.
    ///
    /// A polygon with no material uses this instead. Seven models are
    /// untextured throughout and between them account for all 1,030 polygons
    /// with no material before them: `GLOBE.BIN` 576, `IRIS1.BIN` and
    /// `IRIS4.BIN` 168 each, `SHELL.BIN` 32, `JAW1.BIN` 30, `FANBODY.BIN` and
    /// `JAW2.BIN` 28 each.
    ///
    /// Four of those seven have no colour node before their polygons either -
    /// `FANBODY`, `JAW1`, `JAW2` and `SHELL`, 118 polygons in all - so nothing
    /// in the stream says what colour they are, and both fields are `None`.
    pub colour: Option<u16>,
    /// The last [`SHADE_COLOUR`] before this polygon, which is what a
    /// [`FLAT_POLYGON`] is filled with.
    pub shade: Option<i32>,
    /// Which of [`Model::materials`] this polygon is textured with: the last
    /// material node seen before it in the stream.
    ///
    /// Every polygon in the archives is preceded either by another polygon or
    /// by a material, so "the most recent material" is always defined once the
    /// first one has appeared. A run is 4 polygons long at the median and 168
    /// at the longest.
    pub material: Option<usize>,
}

impl Polygon {
    /// Whether `cross(v1 - v0, v2 - v1)` points along the stored normal.
    ///
    /// `None` when the first three corners are collinear, so the cross product
    /// is zero and there is nothing to compare - 87 polygons in the shipped
    /// models are like that. Of the 33,641 that are not, 33,639 agree and two
    /// disagree, both in `KBOUT.BIN` and `KBOUT2.BIN` and both so far from
    /// planar that the normal is barely meaningful.
    pub fn winding_matches(&self, vertices: &[Vertex]) -> Option<bool> {
        let p: Vec<&Vertex> = self
            .corners
            .iter()
            .map(|c| vertices.get(c.vertex as usize))
            .collect::<Option<Vec<_>>>()?;
        let a = [p[1].x - p[0].x, p[1].y - p[0].y, p[1].z - p[0].z];
        let b = [p[2].x - p[1].x, p[2].y - p[1].y, p[2].z - p[1].z];
        let cross = [
            a[1] as i64 * b[2] as i64 - a[2] as i64 * b[1] as i64,
            a[2] as i64 * b[0] as i64 - a[0] as i64 * b[2] as i64,
            a[0] as i64 * b[1] as i64 - a[1] as i64 * b[0] as i64,
        ];
        if cross == [0, 0, 0] {
            return None;
        }
        let dot: i64 = cross
            .iter()
            .zip(self.normal)
            .map(|(c, n)| c * n as i64)
            .sum();
        Some(dot > 0)
    }
}

/// A [`FLIPBOOK`] material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flipbook {
    /// Which of [`Model::materials`] it stands for; that entry is the first
    /// frame.
    pub material: usize,
    pub frames: Vec<String>,
    /// Seconds a frame, 16.16.
    pub period: i32,
    /// A sound for each frame, where it has one.
    pub sounds: Vec<Option<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct Model {
    /// The mesh-start node's second word: model units to a world unit. The
    /// engine's bounds routine scales vertices by `2 * 0x7fffffff / unit`
    /// in 16.16 (`0x473ff4`), which is `vertex / unit` world units. A
    /// placed actor is drawn at its type's radius instead; powerups, which
    /// have no type, are drawn and picked up at this.
    pub unit: Option<i32>,
    pub vertices: Vec<Vertex>,
    /// Texels given per vertex by [`VERTEX_TEXELS`], 16.16; zero where none.
    pub vertex_texels: Vec<(i32, i32)>,
    pub materials: Vec<String>,
    pub flipbooks: Vec<Flipbook>,
    pub polygons: Vec<Polygon>,
    /// Child model filenames, for a group node.
    pub children: Vec<String>,
    /// Every node in the stream, understood or not.
    pub nodes: Vec<Node>,
}

impl Model {
    pub fn parse(data: &[u8]) -> Result<Model> {
        let mut model = Model::default();
        let mut material: Option<usize> = None;
        let mut colour: Option<u16> = None;
        let mut shade: Option<i32> = None;
        for node in Walk::new(data) {
            let node = node?;
            model.nodes.push(node);
            let at = node.offset;
            match node.kind {
                MESH_START if model.unit.is_none() => {
                    model.unit = Some(i32_at(data, at + 4));
                }
                VERTEX_LIST => {
                    // `+4` is where the list starts in the vertex numbering
                    // that the polygons index, the same way the texel list
                    // works. It is zero in every shipped model but
                    // `target.bin`, the reticle, whose nine vertices are
                    // numbered from 100.
                    let first = i32_at(data, at + 4).max(0) as usize;
                    let count = i32_at(data, at + 8).max(0) as usize;
                    if model.vertices.len() < first + count {
                        model.vertices.resize(first + count, Vertex::default());
                    }
                    for i in 0..count {
                        let v = at + 12 + i * 12;
                        model.vertices[first + i] = Vertex {
                            x: i32_at(data, v),
                            y: i32_at(data, v + 4),
                            z: i32_at(data, v + 8),
                        };
                    }
                }
                MATERIAL => {
                    material = Some(model.materials.len());
                    model.materials.push(cstr(&data[at + 8..at + 24]));
                }
                VERTEX_TEXELS => {
                    let first = i32_at(data, at + 4).max(0) as usize;
                    let count = i32_at(data, at + 8).max(0) as usize;
                    if model.vertex_texels.len() < first + count {
                        model.vertex_texels.resize(first + count, (0, 0));
                    }
                    for i in 0..count {
                        let t = at + 12 + i * 8;
                        model.vertex_texels[first + i] = (i32_at(data, t), i32_at(data, t + 4));
                    }
                }
                FLIPBOOK => {
                    let count = i32_at(data, at + 8).max(0) as usize;
                    let frame = |i: usize| at + 0x1c + i * 32;
                    let frames: Vec<String> = (0..count).map(|i| cstr(&data[frame(i)..frame(i) + 16])).collect();
                    let sounds = (0..count)
                        .map(|i| Some(cstr(&data[frame(i) + 16..frame(i) + 32])).filter(|n| !n.is_empty()))
                        .collect();
                    if let Some(first) = frames.first() {
                        material = Some(model.materials.len());
                        model.flipbooks.push(Flipbook {
                            material: model.materials.len(),
                            frames: frames.clone(),
                            period: i32_at(data, at + 0x10),
                            sounds,
                        });
                        model.materials.push(first.clone());
                    }
                }
                INDEXED_POLYGON | FLAT_POLYGON | SOLID_POLYGON | SHADED_POLYGON => {
                    let count = i32_at(data, at + 4).max(0) as usize;
                    let corners = (0..count)
                        .map(|i| {
                            let vertex = u32_at(data, at + 0x18 + i * 4);
                            let (u, v) = model.vertex_texels.get(vertex as usize).copied().unwrap_or((0, 0));
                            Corner { vertex, u, v }
                        })
                        .collect();
                    model.polygons.push(Polygon {
                        material,
                        colour,
                        shade,
                        kind: node.kind,
                        normal: [i32_at(data, at + 8), i32_at(data, at + 12), i32_at(data, at + 16)],
                        plane: i32_at(data, at + 0x14),
                        corners,
                    });
                }
                FLAT_COLOUR => colour = Some(i32_at(data, at + 8) as u16),
                SHADE_COLOUR => shade = Some(i32_at(data, at + 4)),
                POLYGON | POLYGON_ALT | 0x11 | 0x1E | 0x22 => {
                    let count = i32_at(data, at + 4).max(0) as usize;
                    let mut corners = Vec::with_capacity(count);
                    for i in 0..count {
                        let c = at + 24 + i * 12;
                        corners.push(Corner {
                            vertex: u32_at(data, c),
                            u: i32_at(data, c + 4),
                            v: i32_at(data, c + 8),
                        });
                    }
                    model.polygons.push(Polygon {
                        material,
                        colour,
                        shade,
                        kind: node.kind,
                        normal: [
                            i32_at(data, at + 8),
                            i32_at(data, at + 12),
                            i32_at(data, at + 16),
                        ],
                        plane: i32_at(data, at + 20),
                        corners,
                    });
                }
                GROUP => {
                    let count =
                        i32_at(data, at + 8).max(0).min(GROUP_CHILDREN as i32) as usize;
                    for i in 0..count {
                        let n = at + GROUP_NAMES + i * 16;
                        model.children.push(cstr(&data[n..n + 16]));
                    }
                }
                _ => {}
            }
        }
        Ok(model)
    }

    /// The stream is well formed when the last node ends exactly at end of
    /// file. True of all 342 shipped models.
    pub fn walks_exactly(data: &[u8]) -> bool {
        let mut end = 0;
        for node in Walk::new(data) {
            match node {
                Ok(n) => end = n.offset + n.size,
                Err(_) => return false,
            }
        }
        end == data.len()
    }
}
