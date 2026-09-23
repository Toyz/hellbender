//! `MODELS\*.TXT`: the animated model format.
//!
//! A skeleton of parts, each with its own mesh and a keyframe per frame. The
//! engine parses it at `HELLBEND.EXE:0x0046cd60` and writes it back at
//! `0x0046c240`, and the writer's `fprintf` calls are where the field names
//! here come from - they are the game's own.
//!
//! Not to be confused with `DATA\*.TXT`, which is a mission briefing, or with
//! the binary [MRGL](crate::mrgl) models. A `.TXT` model becomes an MRGL node
//! of type 0x26 once parsed.

use crate::fixed::to_units;
use crate::mrgl::{Corner, Polygon, Vertex};
use crate::text::lines;
use crate::{Error, Result};

/// The material index that means "no texture on this face".
pub const NO_MATERIAL: i32 = 255;

/// Animated models normalise to +/-32,767, where the binary
/// [MRGL](crate::mrgl) models normalise to +/-16,384 - a factor of two. Every
/// one of the 18 shipped `.TXT` models has a maximum absolute vertex component
/// of exactly 32,767 with no part offsets applied.
pub const MODEL_ONE: i32 = 32_767;

#[derive(Debug, Clone, Default)]
pub struct Animated {
    /// Keyframes per part. 21 to 125 across the shipped models.
    pub frames: usize,
    /// 16.16 seconds per frame. 1,310 is 0.02 s and 16,384 is 0.25 s.
    pub time_per_frame: i32,
    /// Texture names, indexed by a face's material field.
    pub materials: Vec<String>,
    pub angle: [i32; 3],
    pub centre: [i32; 3],
    /// The exponent the exporter normalised the vertices by.
    pub mag_power: i32,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Default)]
pub struct Part {
    pub name: String,
    pub pivot: [i32; 3],
    /// One per frame, in the engine's 16-bit circle per axis.
    pub angles: Vec<[i32; 3]>,
    /// One per frame.
    pub centres: Vec<[i32; 3]>,
    /// Index of the part this one hangs off, or -1 for the root.
    pub parent: i32,
    pub hit_points: i32,
    pub leg_flag: i32,
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub vertices: Vec<Vertex>,
    /// Faces in the same shape as a binary polygon node: a normal, a plane
    /// constant and `(vertex, u, v)` per corner.
    pub polygons: Vec<Polygon>,
}

struct Reader<'a> {
    lines: &'a [String],
    at: usize,
}

impl Reader<'_> {
    fn keyword(&mut self, want: &'static str) -> Result<()> {
        let saw = self.lines.get(self.at).map(String::as_str).unwrap_or("");
        if saw.trim() != want {
            return Err(Error::BadLine {
                what: "animated model keyword",
                line: self.at + 1,
                saw: format!("{saw:?}, wanted {want:?}"),
            });
        }
        self.at += 1;
        Ok(())
    }

    fn name(&mut self) -> Result<String> {
        let line = self.lines.get(self.at).ok_or(Error::BadLine {
            what: "animated model name",
            line: self.at + 1,
            saw: String::new(),
        })?;
        self.at += 1;
        Ok(line.trim().to_string())
    }

    fn row(&mut self) -> Result<Vec<i32>> {
        let line = self.lines.get(self.at).ok_or(Error::BadLine {
            what: "animated model row",
            line: self.at + 1,
            saw: String::new(),
        })?;
        let out: Vec<i32> = line
            .split(',')
            .map(|p| p.trim().parse::<i32>())
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| Error::BadLine {
                what: "animated model row",
                line: self.at + 1,
                saw: line.clone(),
            })?;
        self.at += 1;
        Ok(out)
    }

    fn triple(&mut self) -> Result<[i32; 3]> {
        let v = self.row()?;
        if v.len() != 3 {
            return Err(Error::BadLine {
                what: "animated model triple",
                line: self.at,
                saw: format!("{v:?}"),
            });
        }
        Ok([v[0], v[1], v[2]])
    }
}

pub fn parse(data: &[u8]) -> Result<Animated> {
    let lines = lines(data);
    let mut r = Reader { lines: &lines, at: 0 };
    let mut model = Animated::default();

    r.keyword("frameCount,timePerFrame")?;
    let head = r.row()?;
    model.frames = head.first().copied().unwrap_or(0).max(0) as usize;
    model.time_per_frame = head.get(1).copied().unwrap_or(0);

    r.keyword("materialCount")?;
    let count = r.row()?[0].max(0) as usize;
    r.keyword("materialList")?;
    for _ in 0..count {
        model.materials.push(r.name()?);
    }

    r.keyword("angle")?;
    model.angle = r.triple()?;
    r.keyword("center")?;
    model.centre = r.triple()?;
    r.keyword("magPower")?;
    model.mag_power = r.row()?[0];
    r.keyword("partCount")?;
    let parts = r.row()?[0].max(0) as usize;

    for _ in 0..parts {
        let mut part = Part::default();
        r.keyword("partName")?;
        part.name = r.name()?;
        r.keyword("pivot")?;
        part.pivot = r.triple()?;
        r.keyword("angleList")?;
        for _ in 0..model.frames {
            part.angles.push(r.triple()?);
        }
        r.keyword("centerList")?;
        for _ in 0..model.frames {
            part.centres.push(r.triple()?);
        }
        r.keyword("parent,partHP,legFlag")?;
        let v = r.row()?;
        part.parent = v[0];
        part.hit_points = *v.get(1).unwrap_or(&0);
        part.leg_flag = *v.get(2).unwrap_or(&0);
        r.keyword("min")?;
        part.min = r.triple()?;
        r.keyword("max")?;
        part.max = r.triple()?;
        r.keyword("vertexCount,faceCount")?;
        let v = r.row()?;
        let (nv, nf) = (v[0].max(0) as usize, v[1].max(0) as usize);
        r.keyword("vertexList")?;
        for _ in 0..nv {
            let [x, y, z] = r.triple()?;
            part.vertices.push(Vertex { x, y, z });
        }
        r.keyword("material and faceList")?;
        for _ in 0..nf {
            // 255 is the "no material" sentinel: twelve faces across
            // `KRACKEN`, `GMRADAR` and `PROCES2` use it, and no model has
            // anything like 255 materials.
            let raw = r.row()?[0];
            let material = if raw == NO_MATERIAL { None } else { Some(raw.max(0) as usize) };
            let f = r.row()?;
            let corners = f.first().copied().unwrap_or(0).max(0) as usize;
            if f.len() != 5 + corners * 3 {
                return Err(Error::BadLine {
                    what: "animated model face",
                    line: r.at,
                    saw: format!("{corners} corners but {} values", f.len()),
                });
            }
            part.polygons.push(Polygon {
                kind: crate::mrgl::POLYGON,
                normal: [f[1], f[2], f[3]],
                plane: f[4],
                material,
                colour: None,
                shade: None,
                corners: (0..corners)
                    .map(|i| Corner {
                        vertex: f[5 + i * 3] as u32,
                        u: f[6 + i * 3],
                        v: f[7 + i * 3],
                    })
                    .collect(),
            });
        }
        model.parts.push(part);
    }
    Ok(model)
}

impl Animated {
    pub fn vertex_count(&self) -> usize {
        self.parts.iter().map(|p| p.vertices.len()).sum()
    }

    pub fn polygon_count(&self) -> usize {
        self.parts.iter().map(|p| p.polygons.len()).sum()
    }

    /// The model's rest pose as one mesh, in the same normalisation the
    /// binary models use so that one draw path serves both.
    ///
    /// This is [`Animated::pose`] at time zero: the first keyframe, with no
    /// interpolation.
    pub fn rest_pose(&self) -> (Vec<Vertex>, Vec<Polygon>) {
        self.pose(0.0)
    }

    /// Where the model is `seconds` into its animation.
    ///
    /// The engine keeps a clock per actor, turns it into a frame index and a
    /// fraction between 0 and 0xffff (`0x4684c0`), and interpolates each
    /// part's angle and centre between that keyframe and the next - the
    /// angles by the difference wrapped into sixteen bits, so they take the
    /// short way round. The last frame interpolates back to the first.
    ///
    /// Each part is then drawn with a transform built from its own
    /// interpolated angle and centre (`0x467a10` hands both to `0x42aa30`).
    /// **The `pivot` and `parent` fields take no part in it**: a part's
    /// centre is where it goes in model space, not an offset from its
    /// parent, which is why summing the chain put `TREX`'s head between its
    /// shoulders. Centres and vertices are in one space as authored: the
    /// rescale that would scale both (`0x4664e0`) belongs to the `.ASC`
    /// importers, which the game never calls.
    pub fn pose(&self, seconds: f32) -> (Vec<Vertex>, Vec<Polygon>) {
        let mut vertices = Vec::with_capacity(self.vertex_count());
        let mut polygons = Vec::with_capacity(self.polygon_count());
        let (frame, fraction) = self.frame_at(seconds);
        let next = if frame + 1 < self.frames { frame + 1 } else { 0 };
        // The model's own angle and centre sit under every part: the engine
        // builds a matrix from the angle into the header at `+0x634`
        // (`0x4681d0`) and the parts are drawn inside it.
        let model = rotation(self.angle.map(|a| a as f32));
        let offset = self.centre.map(|c| c as f32);
        for part in &self.parts {
            let base = vertices.len() as u32;
            let angle = tween_angles(part.angles.get(frame), part.angles.get(next), fraction);
            let centre = tween(part.centres.get(frame), part.centres.get(next), fraction);
            let rotate = rotation(angle);
            for v in &part.vertices {
                let p = [v.x as f32, v.y as f32, v.z as f32];
                let turned: [f32; 3] =
                    std::array::from_fn(|k| (0..3).map(|j| rotate[k][j] * p[j]).sum());
                let placed: [f32; 3] = std::array::from_fn(|k| turned[k] + centre[k]);
                let out: [f32; 3] = std::array::from_fn(|k| {
                    (0..3).map(|j| model[k][j] * placed[j]).sum::<f32>() + offset[k]
                });
                // Halve, so the result is in the binary models' +/-16,384.
                vertices.push(Vertex { x: out[0] as i32 / 2, y: out[1] as i32 / 2, z: out[2] as i32 / 2 });
            }
            for poly in &part.polygons {
                let mut moved = poly.clone();
                for corner in &mut moved.corners {
                    corner.vertex += base;
                }
                polygons.push(moved);
            }
        }
        (vertices, polygons)
    }

    /// Where one part sits at `seconds`, in the same space [`Animated::pose`]
    /// puts its vertices. This is what the engine asks for when a type names
    /// a part rather than a vertex as its muzzle: `0x46edd0` takes the part's
    /// interpolated centre through the model's matrix.
    pub fn part_origin(&self, part: usize, seconds: f32) -> Option<[i32; 3]> {
        let p = self.parts.get(part)?;
        let (frame, fraction) = self.frame_at(seconds);
        let next = if frame + 1 < self.frames { frame + 1 } else { 0 };
        let centre = tween(p.centres.get(frame), p.centres.get(next), fraction);
        let model = rotation(self.angle.map(|a| a as f32));
        let offset = self.centre.map(|c| c as f32);
        Some(std::array::from_fn(|k| {
            ((0..3).map(|j| model[k][j] * centre[j]).sum::<f32>() + offset[k]) as i32 / 2
        }))
    }

    /// Which keyframe `seconds` lands on, and how far past it, 0.0 to 1.0.
    /// The clock wraps at the last frame.
    pub fn frame_at(&self, seconds: f32) -> (usize, f32) {
        let per_frame = to_units(self.time_per_frame);
        if self.frames == 0 || per_frame <= 0.0 {
            return (0, 0.0);
        }
        let ticks = (seconds / per_frame).rem_euclid(self.frames as f32);
        (ticks as usize % self.frames, ticks.fract())
    }
}

/// Between two keyframe angles, the short way round: the engine takes the
/// difference, sign-extends it from sixteen bits and scales it by the
/// fraction (`0x468585`).
fn tween_angles(a: Option<&[i32; 3]>, b: Option<&[i32; 3]>, t: f32) -> [f32; 3] {
    let (a, b) = (a.copied().unwrap_or_default(), b.copied().unwrap_or(a.copied().unwrap_or_default()));
    std::array::from_fn(|k| {
        let d = (((b[k] - a[k]) << 16) >> 16) as f32;
        a[k] as f32 + d * t
    })
}

/// Between two keyframe centres, straight.
fn tween(a: Option<&[i32; 3]>, b: Option<&[i32; 3]>, t: f32) -> [f32; 3] {
    let (a, b) = (a.copied().unwrap_or_default(), b.copied().unwrap_or(a.copied().unwrap_or_default()));
    std::array::from_fn(|k| a[k] as f32 + (b[k] - a[k]) as f32 * t)
}

/// The rotation a part's three angles make, in the engine's convention: the
/// first is about x, the second about z and the third about y, which is the
/// order the actor draw hands its pitch, roll and heading to `0x42aa30`.
fn rotation(angle: [f32; 3]) -> [[f32; 3]; 3] {
    // The same right, up and forward the renderer builds for a placement.
    let [right, up, forward] = crate::vector::axes(angle[2], angle[0], angle[1]);
    [
        [right[0], up[0], forward[0]],
        [right[1], up[1], forward[1]],
        [right[2], up[2], forward[2]],
    ]
}
