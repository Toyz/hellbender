//! The target: an 8-bit indexed framebuffer with a depth buffer, and a
//! textured triangle filler that shades and fogs through the level's ramps.

use hb_formats::act::Palette;
use hb_formats::colour::Ramp;
use hb_formats::raw::Image;

/// A vertex as the rasteriser wants it: screen position, reciprocal depth for
/// the perspective divide, and texture coordinates in texels.
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    /// View-space depth in world units. Used for fog and for the depth test.
    pub depth: f32,
    pub u: f32,
    pub v: f32,
}

pub struct Target {
    pub width: usize,
    pub height: usize,
    /// Palette indices, the same 8-bit space the original renders in.
    pub colour: Vec<u8>,
    pub depth: Vec<f32>,
}

impl Target {
    /// The three modes the game ships art for. `HELLBEND.INI` picks one with
    /// `gamePIXX` and `gamePIXY`.
    pub const MODE_200: (usize, usize) = (320, 200);
    pub const MODE_400: (usize, usize) = (320, 400);
    pub const MODE_480: (usize, usize) = (640, 480);

    pub fn new(width: usize, height: usize) -> Target {
        Target {
            width,
            height,
            colour: vec![0; width * height],
            depth: vec![f32::INFINITY; width * height],
        }
    }

    pub fn clear(&mut self, index: u8) {
        self.colour.fill(index);
        self.depth.fill(f32::INFINITY);
    }

    pub fn to_rgb(&self, palette: &Palette) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.colour.len() * 3);
        for &index in &self.colour {
            out.extend_from_slice(&palette.rgb(index));
        }
        out
    }

    /// One textured triangle, affinely mapped.
    ///
    /// The original has a `perspectiveFlag` in its settings with three values,
    /// so it chooses between affine and perspective correction per some
    /// threshold. Which threshold is not known, and at a cell's size the
    /// difference is small, so this always interpolates affinely - a choice of
    /// this renderer, not a reading of the engine.
    pub fn triangle(&mut self, tri: [Vertex; 3], texture: &Image, shade: &Shade) {
        let top = tri
            .iter()
            .map(|v| v.y)
            .fold(f32::MAX, f32::min)
            .floor()
            .max(0.0) as usize;
        let bottom = tri
            .iter()
            .map(|v| v.y)
            .fold(f32::MIN, f32::max)
            .ceil()
            .min(self.height as f32) as usize;

        // Barycentric setup. A degenerate triangle has zero area and is skipped
        // rather than divided by.
        let area = edge(tri[0], tri[1], tri[2].x, tri[2].y);
        if area.abs() < 1e-6 {
            return;
        }

        for y in top..bottom {
            let scan = y as f32 + 0.5;
            let (mut left, mut right) = (f32::MAX, f32::MIN);
            for i in 0..3 {
                let (a, b) = (tri[i], tri[(i + 1) % 3]);
                if (a.y <= scan) == (b.y <= scan) {
                    continue;
                }
                let x = a.x + (scan - a.y) / (b.y - a.y) * (b.x - a.x);
                left = left.min(x);
                right = right.max(x);
            }
            if left > right {
                continue;
            }
            let from = left.ceil().max(0.0) as usize;
            let to = (right.floor() as isize).min(self.width as isize - 1);
            for x in from..=(to.max(0) as usize) {
                if x >= self.width {
                    break;
                }
                let px = x as f32 + 0.5;
                let w0 = edge(tri[1], tri[2], px, scan) / area;
                let w1 = edge(tri[2], tri[0], px, scan) / area;
                let w2 = 1.0 - w0 - w1;
                let depth = w0 * tri[0].depth + w1 * tri[1].depth + w2 * tri[2].depth;
                let at = y * self.width + x;
                if depth >= self.depth[at] {
                    continue;
                }
                let u = w0 * tri[0].u + w1 * tri[1].u + w2 * tri[2].u;
                let v = w0 * tri[0].v + w1 * tri[1].v + w2 * tri[2].v;
                let index = sample(texture, u, v);
                if index == 0 && shade.index_zero_is_clear {
                    continue;
                }
                self.colour[at] = shade.apply(index, depth);
                self.depth[at] = depth;
            }
        }
    }
}

/// How a sampled index becomes a written index: the level's light ramp at the
/// surface's own shade, then the fog ramp by distance.
pub struct Shade<'a> {
    pub light: Option<&'a Ramp>,
    pub fog: Option<&'a Ramp>,
    /// 0 is dark, 255 is full, as the shading database stores it.
    pub intensity: u8,
    /// Where the fog ramp reaches its last row.
    pub fog_distance: f32,
    /// Terrain is opaque; sprites and cockpit overlays are not.
    pub index_zero_is_clear: bool,
}

impl Shade<'_> {
    pub fn apply(&self, index: u8, depth: f32) -> u8 {
        let mut out = index;
        if let Some(ramp) = self.light {
            out = ramp.shade(((255 - self.intensity as usize) >> 4).min(15), out);
        }
        if let Some(ramp) = self.fog {
            let level = (depth / self.fog_distance * 16.0) as isize;
            out = ramp.shade(level.clamp(0, 15) as usize, out);
        }
        out
    }
}

fn edge(a: Vertex, b: Vertex, x: f32, y: f32) -> f32 {
    (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x)
}

/// Texture space is 256 units wide whatever the texture's real size is, so a
/// coordinate scales by `size / 256` rather than indexing a texel directly.
///
/// The alternative reading - that the coordinate is already a texel and a
/// 64 x 64 texture therefore repeats four times across a face - is what this
/// renderer tried first, and it puts a fine grid over everything. Scaling is
/// also what makes a model face consistent: the polygon nodes map their
/// textures corner to corner with 1 and 255, which is one tile under scaling
/// and four under wrapping.
pub const TEXTURE_SPACE: f32 = 256.0;

fn sample(texture: &Image, u: f32, v: f32) -> u8 {
    let w = texture.shape.width;
    let h = texture.shape.height;
    if w == 0 || h == 0 {
        return 0;
    }
    let tx = ((u * w as f32 / TEXTURE_SPACE) as isize).rem_euclid(w as isize) as usize;
    let ty = ((v * h as f32 / TEXTURE_SPACE) as isize).rem_euclid(h as isize) as usize;
    texture.pixels[ty * w + tx]
}
