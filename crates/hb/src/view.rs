//! A debug view of a model: perspective projection, back-face culling by the
//! stored normal, painter's algorithm, flat shading.
//!
//! This is not the port's renderer. It exists to answer "is the geometry
//! right", which a table of numbers cannot.

use hb_formats::mrgl::Model;

pub struct View {
    pub width: usize,
    pub height: usize,
    /// Rotation about y, then about x, in radians.
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for View {
    fn default() -> Self {
        View { width: 640, height: 640, yaw: 0.7, pitch: 0.45 }
    }
}

/// Renders to an RGB buffer. Returns `(pixels, drawn, culled)`.
pub fn render(model: &Model, view: &View) -> (Vec<u8>, usize, usize) {
    let w = view.width;
    let h = view.height;
    let mut pixels = vec![0x12u8; w * h * 3];

    if model.vertices.is_empty() || model.polygons.is_empty() {
        return (pixels, 0, 0);
    }

    // Fit the model to the frame from its own bounds.
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for v in &model.vertices {
        for (i, c) in [v.x, v.y, v.z].into_iter().enumerate() {
            lo[i] = lo[i].min(c);
            hi[i] = hi[i].max(c);
        }
    }
    let centre = [
        (lo[0] + hi[0]) as f32 / 2.0,
        (lo[1] + hi[1]) as f32 / 2.0,
        (lo[2] + hi[2]) as f32 / 2.0,
    ];
    let radius = (0..3)
        .map(|i| (hi[i] - lo[i]) as f32 / 2.0)
        .fold(1.0f32, f32::max);

    let (sy, cy) = view.yaw.sin_cos();
    let (sp, cp) = view.pitch.sin_cos();
    let rotate = |p: [f32; 3]| -> [f32; 3] {
        let x = p[0] * cy + p[2] * sy;
        let z = -p[0] * sy + p[2] * cy;
        let y = p[1] * cp - z * sp;
        let z = p[1] * sp + z * cp;
        [x, y, z]
    };

    // `world` divides by `radius`, so everything below is in units of the
    // model's own half-extent and the camera distance is unitless too.
    const DISTANCE: f32 = 3.2;
    let scale = w.min(h) as f32 * 0.42;
    let project = |p: [f32; 3]| -> Option<(f32, f32, f32)> {
        let depth = p[2] + DISTANCE;
        if depth <= 0.05 {
            return None;
        }
        let k = scale * DISTANCE / depth;
        Some((
            w as f32 / 2.0 + p[0] * k,
            // Screen y grows downward; model y is treated as up.
            h as f32 / 2.0 - p[1] * k,
            depth,
        ))
    };

    let world = |i: u32| -> [f32; 3] {
        let v = &model.vertices[i as usize];
        rotate([
            (v.x as f32 - centre[0]) / radius,
            (v.y as f32 - centre[1]) / radius,
            (v.z as f32 - centre[2]) / radius,
        ])
    };

    // Sort back to front by the mean depth of the corners.
    let mut order: Vec<(usize, f32)> = Vec::with_capacity(model.polygons.len());
    for (i, poly) in model.polygons.iter().enumerate() {
        if poly.corners.iter().any(|c| c.vertex as usize >= model.vertices.len()) {
            continue;
        }
        let depth = poly
            .corners
            .iter()
            .map(|c| world(c.vertex)[2])
            .sum::<f32>()
            / poly.corners.len() as f32;
        order.push((i, depth));
    }
    order.sort_by(|a, b| b.1.total_cmp(&a.1));

    let light = normalise([0.35, 0.75, -0.55]);
    let (mut drawn, mut culled) = (0usize, 0usize);

    for (index, _) in order {
        let poly = &model.polygons[index];
        let normal = rotate(normalise([
            poly.normal[0] as f32,
            poly.normal[1] as f32,
            poly.normal[2] as f32,
        ]));
        // The normal points out of the face, so a face turned away from the
        // camera has a normal pointing away down the view axis.
        if normal[2] > 0.0 {
            culled += 1;
            continue;
        }
        let Some(points) = poly
            .corners
            .iter()
            .map(|c| project(world(c.vertex)))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };

        let lambert = (-(normal[0] * light[0] + normal[1] * light[1] + normal[2] * light[2]))
            .max(0.0);
        let shade = 0.30 + 0.70 * lambert;
        // Tint by material so adjacent faces are told apart.
        let tint = [
            0.80 + 0.20 * ((index % 5) as f32 / 4.0),
            0.84 + 0.16 * ((index % 3) as f32 / 2.0),
            0.92,
        ];
        let colour = [
            (255.0 * shade * tint[0]) as u8,
            (255.0 * shade * tint[1]) as u8,
            (255.0 * shade * tint[2]) as u8,
        ];
        fill(&mut pixels, w, h, &points, colour);
        drawn += 1;
    }

    (pixels, drawn, culled)
}

fn normalise(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len == 0.0 {
        v
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

/// Scanline fill of a convex polygon, which every face here is - they all have
/// three or four corners.
fn fill(pixels: &mut [u8], w: usize, h: usize, points: &[(f32, f32, f32)], colour: [u8; 3]) {
    let top = points.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
    let bottom =
        (points.iter().map(|p| p.1).fold(f32::MIN, f32::max).ceil() as usize).min(h.saturating_sub(1));
    for y in top..=bottom {
        let scan = y as f32 + 0.5;
        let mut crossings: Vec<f32> = Vec::with_capacity(4);
        for i in 0..points.len() {
            let (x0, y0, _) = points[i];
            let (x1, y1, _) = points[(i + 1) % points.len()];
            if (y0 <= scan) == (y1 <= scan) {
                continue;
            }
            crossings.push(x0 + (scan - y0) / (y1 - y0) * (x1 - x0));
        }
        if crossings.len() < 2 {
            continue;
        }
        crossings.sort_by(f32::total_cmp);
        let left = crossings[0].ceil().max(0.0) as usize;
        let right = (crossings[crossings.len() - 1].floor() as isize).min(w as isize - 1);
        for x in left..=(right.max(0) as usize) {
            if x >= w {
                break;
            }
            let at = (y * w + x) * 3;
            pixels[at..at + 3].copy_from_slice(&colour);
        }
    }
}

/// A top-down shaded view of a level's ground, lit by the per-triangle normal
/// the engine computes. Boxes and chambers are marked rather than drawn.
///
/// Like [`render`], this is a debug view. It exists to show that the height
/// query, the parity-dependent triangle split and the normal all line up.
pub fn heightmap(grid: &hb_world::Grid, scale: usize) -> (Vec<u8>, usize, usize) {
    use hb_formats::terrain::{Layer, SIDE};
    use hb_world::grid::triangle;
    use hb_world::{Cell, Half};

    let side = SIDE * scale;
    let mut pixels = vec![0u8; side * side * 3];
    let light = [0.45f32, 0.80, -0.40];

    let (mut lo, mut hi) = (i32::MAX, i32::MIN);
    for z in 0..SIDE as i32 {
        for x in 0..SIDE as i32 {
            if let Some(h) = grid.height_at_grid(Layer::Ground, x, z) {
                lo = lo.min(h);
                hi = hi.max(h);
            }
        }
    }
    let range = (hi - lo).max(1) as f32;

    let (mut boxes, mut chambers) = (0usize, 0usize);
    for z in 0..SIDE as i32 {
        for x in 0..SIDE as i32 {
            let cell = Cell::new(x, z);
            let height = grid.height_at_grid(Layer::Ground, x, z).unwrap_or(0);
            let tint = (height - lo) as f32 / range;

            let has_box = grid.has_box_a(cell) || grid.has_box_b(cell);
            if grid.has_box_a(cell) {
                boxes += 1;
            }
            // A chamber whose ceiling is not at the default is a real chamber.
            let roofed = grid
                .height_at_grid(Layer::ChamberCeiling, x, z)
                .is_some_and(|c| c != 0);
            if roofed {
                chambers += 1;
            }

            for sy in 0..scale {
                for sx in 0..scale {
                    // Which half of the cell this pixel is in, so the
                    // checkerboard split shows up in the shading. Same tests
                    // the engine uses, at sub-cell resolution.
                    let half = match cell.diagonal() {
                        hb_world::Diagonal::Main if sy < sx => Half::Second,
                        hb_world::Diagonal::Main => Half::First,
                        hb_world::Diagonal::Anti if sx + sy < scale => Half::Second,
                        hb_world::Diagonal::Anti => Half::First,
                    };
                    let normal = grid
                        .triangle_normal(Layer::Ground, triangle(cell, half))
                        .unwrap_or([0, 1, 0]);
                    let n = normalise([
                        normal[0] as f32,
                        normal[1] as f32,
                        normal[2] as f32,
                    ]);
                    let lambert =
                        (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0);
                    let shade = 0.25 + 0.75 * lambert;

                    let mut rgb = [
                        (255.0 * shade * (0.30 + 0.70 * tint)) as u8,
                        (255.0 * shade * (0.45 + 0.55 * tint)) as u8,
                        (255.0 * shade * (0.35 + 0.35 * tint)) as u8,
                    ];
                    if has_box {
                        rgb[0] = rgb[0].saturating_add(70);
                        rgb[2] = rgb[2].saturating_add(30);
                    }
                    if roofed {
                        rgb[2] = rgb[2].saturating_add(60);
                    }
                    let px = (x as usize * scale + sx, z as usize * scale + sy);
                    let at = (px.1 * side + px.0) * 3;
                    pixels[at..at + 3].copy_from_slice(&rgb);
                }
            }
        }
    }
    (pixels, boxes, chambers)
}

/// A top-down textured view: each cell sampled from the texture its `.CLR`
/// word names, through the level's palette, lit by the shading database.
///
/// This is the first drawing that uses the texture index, the palette and the
/// per-cell shade together, so it is the check on all three at once.
pub fn textured(
    grid: &hb_world::Grid,
    textures: &[Option<hb_formats::raw::Image>],
    palette: &hb_formats::act::Palette,
    ramp: Option<&hb_formats::colour::Ramp>,
    scale: usize,
) -> (Vec<u8>, usize) {
    use hb_formats::terrain::{TextureRef, SIDE};

    let side = SIDE * scale;
    let mut pixels = vec![0u8; side * side * 3];
    let mut missing = 0usize;

    for z in 0..SIDE as i32 {
        for x in 0..SIDE as i32 {
            let word = TextureRef(grid.terrain.colour.at(x, z, 0));
            let image = textures.get(word.index() as usize).and_then(Option::as_ref);
            if image.is_none() {
                missing += 1;
            }
            // The ground's two shading bytes are one little-endian value in
            // 0..=511: a 0-255 intensity in the low byte and one flag in bit 8.
            let shade = grid
                .terrain
                .shading
                .as_ref()
                .map(|s| s.ground_intensity(x, z))
                .unwrap_or(255);

            for sy in 0..scale {
                for sx in 0..scale {
                    let half = match hb_world::Cell::new(x, z).diagonal() {
                        hb_world::Diagonal::Main if sy < sx => 1,
                        hb_world::Diagonal::Main => 0,
                        hb_world::Diagonal::Anti if sx + sy < scale => 1,
                        hb_world::Diagonal::Anti => 0,
                    };
                    let index = match image {
                        Some(img) => {
                            let u = sx * img.shape.width / scale;
                            let v = sy * img.shape.height / scale;
                            img.pixels[v * img.shape.width + u]
                        }
                        None => 0,
                    };
                    // 255 is full brightness and the ramp's row 0 is the
                    // identity, so the level is the complement.
                    let _ = half;
                    let index = match ramp {
                        Some(r) => r.shade(((255 - shade as usize) >> 4).min(15), index),
                        None => index,
                    };
                    let rgb = palette.rgb(index);
                    let at = ((z as usize * scale + sy) * side + x as usize * scale + sx) * 3;
                    pixels[at..at + 3].copy_from_slice(&rgb);
                }
            }
        }
    }
    (pixels, missing)
}
