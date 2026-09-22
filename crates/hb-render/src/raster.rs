//! The target: an 8-bit indexed framebuffer with a depth buffer, and a
//! textured triangle filler that shades and fogs through the level's ramps.

use hb_formats::act::Palette;
use hb_formats::colour::Ramp;
use hb_formats::raw::{Image, Shape};

/// A vertex as the rasteriser wants it: screen position, view depth,
/// texture coordinates in the 256-unit texture space, and a light level.
#[derive(Debug, Clone, Copy, Default)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    /// View-space depth in world units. Used for fog and for the depth test.
    pub depth: f32,
    pub u: f32,
    pub v: f32,
    /// 0 is dark, 255 is full, as the shading database stores it. Interpolated
    /// across the triangle: the engine gives each ground vertex its own shade
    /// (`0x414e05`), so the ground is Gouraud shaded.
    pub light: f32,
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

    /// One pixel, if it is in the frame.
    pub fn plot(&mut self, x: isize, y: isize, index: u8) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.colour[y as usize * self.width + x as usize] = index;
        }
    }

    /// A line one pixel wide, clipped to the frame (the engine's `0x4871c0`
    /// clips it to the view instead).
    pub fn line(&mut self, (x0, y0): (isize, isize), (x1, y1): (isize, isize), index: u8) {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for i in 0..=steps {
            self.plot(x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps, index);
        }
    }

    pub fn to_rgb(&self, palette: &Palette) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.colour.len() * 3);
        for &index in &self.colour {
            out.extend_from_slice(&palette.rgb(index));
        }
        out
    }

    /// Blit an image over the frame, treating index 0 as transparent.
    ///
    /// That index 0 is the clear colour is measured rather than declared: the
    /// whole viewport region of `ART\CKPT200.RAW` is index 0 and nothing else,
    /// 32,051 of its 64,000 pixels. See `docs/formats/raw.md`.
    ///
    /// The cockpit art is drawn in `VGA.ACT`, and a level's own palette agrees
    /// with `VGA.ACT` on 240 of its 256 entries, so it can be blitted into a
    /// frame that is in the level's palette without a remap.
    pub fn overlay(&mut self, image: &Image) {
        self.overlay_at(image, 0, 0);
    }

    /// The same, with the image's top left corner at `(at_x, at_y)`.
    pub fn overlay_at(&mut self, image: &Image, at_x: usize, at_y: usize) {
        let w = image.shape.width.min(self.width.saturating_sub(at_x));
        let h = image.shape.height.min(self.height.saturating_sub(at_y));
        for y in 0..h {
            for x in 0..w {
                let index = image.pixels[y * image.shape.width + x];
                if index != 0 {
                    self.colour[(at_y + y) * self.width + at_x + x] = index;
                }
            }
        }
    }

    /// One triangle in a single colour, shaded and fogged like a textured one.
    pub fn flat_triangle(&mut self, tri: [Vertex; 3], index: u8, shade: &Shade) {
        let one = Image { shape: Shape::new(1, 1), pixels: vec![index] };
        self.triangle(tri, &one, shade);
    }

    /// One textured triangle, perspective correct.
    ///
    /// The original has a `perspectiveFlag` in its settings with three values,
    /// so it chooses between affine and perspective correction per some
    /// threshold, which is not known. This always corrects: texture, light and
    /// depth are interpolated as `a / z` and `1 / z`. Affine interpolation was
    /// this renderer's first choice and bent every texture on the large ground
    /// triangles near the eye.
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
        // Everything that varies is carried divided by depth.
        let inv = tri.map(|v| 1.0 / v.depth.max(1e-3));
        let over = |f: fn(&Vertex) -> f32| [f(&tri[0]) * inv[0], f(&tri[1]) * inv[1], f(&tri[2]) * inv[2]];
        let (us, vs, ls) = (over(|v| v.u), over(|v| v.v), over(|v| v.light));

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
                let one_over = w0 * inv[0] + w1 * inv[1] + w2 * inv[2];
                if one_over <= 0.0 {
                    continue;
                }
                let depth = 1.0 / one_over;
                let at = y * self.width + x;
                if depth >= self.depth[at] {
                    continue;
                }
                let u = (w0 * us[0] + w1 * us[1] + w2 * us[2]) * depth;
                let v = (w0 * vs[0] + w1 * vs[1] + w2 * vs[2]) * depth;
                let light = (w0 * ls[0] + w1 * ls[1] + w2 * ls[2]) * depth;
                let index = sample(texture, u, v);
                if index == 0 && shade.index_zero_is_clear {
                    continue;
                }
                self.colour[at] = shade.apply(index, depth, light);
                self.depth[at] = depth;
            }
        }
    }
}

/// How a sampled index becomes a written index: the level's light ramp at the
/// pixel's light level, then the fog ramp by distance.
pub struct Shade<'a> {
    pub light: Option<&'a Ramp>,
    pub fog: Option<&'a Ramp>,
    /// The depth where fog begins, and how far past it the fog ramp reaches
    /// its last row.
    pub fog_start: f32,
    pub fog_range: f32,
    /// Terrain is opaque; sprites and cockpit overlays are not.
    pub index_zero_is_clear: bool,
}

impl Shade<'_> {
    /// `light` is 0 (dark) to 255 (full).
    pub fn apply(&self, index: u8, depth: f32, light: f32) -> u8 {
        let mut out = index;
        if let Some(ramp) = self.light {
            let level = (255.0 - light.clamp(0.0, 255.0)) as usize >> 4;
            out = ramp.shade(level.min(15), out);
        }
        if let Some(ramp) = self.fog {
            let f = ((depth - self.fog_start) / self.fog_range).clamp(0.0, 1.0);
            out = ramp.shade((f * 15.0).round() as usize, out);
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
