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

#[derive(Debug, Clone, Default)]
pub struct Model {
    pub vertices: Vec<Vertex>,
    pub materials: Vec<String>,
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
        for node in Walk::new(data) {
            let node = node?;
            model.nodes.push(node);
            let at = node.offset;
            match node.kind {
                VERTEX_LIST => {
                    let count = i32_at(data, at + 8).max(0) as usize;
                    for i in 0..count {
                        let v = at + 12 + i * 12;
                        model.vertices.push(Vertex {
                            x: i32_at(data, v),
                            y: i32_at(data, v + 4),
                            z: i32_at(data, v + 8),
                        });
                    }
                }
                MATERIAL => {
                    material = Some(model.materials.len());
                    model.materials.push(cstr(&data[at + 8..at + 24]));
                }
                FLAT_COLOUR => colour = Some(i32_at(data, at + 8) as u16),
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
