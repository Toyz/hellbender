//! Walking the world and handing triangles to the rasteriser.

use hb_formats::act::Palette;
use hb_formats::colour::Ramp;
use hb_formats::raw::Image;
use hb_formats::terrain::{Layer, TextureRef, CELL_SIZE, SIDE};
use hb_world::{triangle, Cell, Corner, Grid, Half};

use crate::camera::Camera;
use crate::raster::{Shade, Target, Vertex};

pub struct Scene<'a> {
    pub grid: Grid<'a>,
    pub textures: &'a [Option<Image>],
    pub palette: &'a Palette,
    pub light: Option<&'a Ramp>,
    pub fog: Option<&'a Ramp>,
}

/// Draws the ground and both box sets from the camera's position.
///
/// The traversal is this renderer's own: the engine's visibility scheme has not
/// been read. This walks every cell within `camera.far` of the eye, back to
/// front, and lets the depth buffer settle the rest.
pub fn draw_world(target: &mut Target, scene: &Scene, camera: &Camera) -> Drawn {
    let mut drawn = Drawn::default();
    let reach = (camera.far / CELL_SIZE).max(1);
    let eye = Cell::containing(camera.x, camera.z);

    // Back to front by cell distance, so the painter's order is roughly right
    // before the depth buffer even runs.
    let mut cells: Vec<(i32, i32, i32)> = Vec::new();
    for dz in -reach..=reach {
        for dx in -reach..=reach {
            let d = dx * dx + dz * dz;
            if d > reach * reach {
                continue;
            }
            cells.push((d, eye.x + dx, eye.z + dz));
        }
    }
    cells.sort_by_key(|&(d, _, _)| std::cmp::Reverse(d));

    for (_, cx, cz) in cells {
        let cell = Cell::new(cx, cz);
        // The grid wraps, but the world does not repeat visually here: the
        // unwrapped indices give the position, the wrapped ones the data.
        let origin = (cx << 19, cz << 19);
        draw_ground(target, scene, camera, cell, origin, &mut drawn);
        for layer in [Layer::BoxA, Layer::BoxB] {
            draw_box(target, scene, camera, cell, origin, layer, &mut drawn);
        }
    }
    drawn
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Drawn {
    pub ground: usize,
    pub boxes: usize,
    pub clipped: usize,
}

fn corner_world(origin: (i32, i32), corner: Corner) -> (i32, i32) {
    let (dx, dz) = corner.offset();
    (origin.0 + dx * CELL_SIZE, origin.1 + dz * CELL_SIZE)
}

fn project(camera: &Camera, target: &Target, x: i32, y: i32, z: i32) -> Option<(f32, f32, f32)> {
    let [vx, vy, vz] = camera.to_view(x, y, z);
    // Near plane at a tenth of a cell.
    if vz <= 0.8 {
        return None;
    }
    let half_fov = camera.fov.to_radians() / 2.0;
    let scale = (target.width as f32 / 2.0) / half_fov.tan();
    Some((
        target.width as f32 / 2.0 + vx * scale / vz,
        target.height as f32 / 2.0 - vy * scale / vz,
        vz,
    ))
}

fn shade_for<'a>(scene: &'a Scene, camera: &Camera, intensity: u8) -> Shade<'a> {
    Shade {
        light: scene.light,
        fog: scene.fog,
        intensity,
        fog_distance: (camera.far / 65536) as f32,
        index_zero_is_clear: false,
    }
}

fn draw_ground(
    target: &mut Target,
    scene: &Scene,
    camera: &Camera,
    cell: Cell,
    origin: (i32, i32),
    drawn: &mut Drawn,
) {
    let word = TextureRef(scene.grid.terrain.colour.at(cell.x, cell.z, 0));
    let Some(Some(texture)) = scene.textures.get(word.index() as usize) else {
        return;
    };
    let intensity = scene
        .grid
        .terrain
        .shading
        .as_ref()
        .map(|s| s.ground_intensity(cell.x, cell.z))
        .unwrap_or(255);
    let shade = shade_for(scene, camera, intensity);

    for half in [Half::First, Half::Second] {
        let tri = triangle(cell, half);
        let mut points = [Vertex { x: 0.0, y: 0.0, depth: 0.0, u: 0.0, v: 0.0 }; 3];
        let mut visible = true;
        for (slot, corner) in points.iter_mut().zip(tri.corners) {
            let (wx, wz) = corner_world(origin, corner);
            let height = scene
                .grid
                .height_at_grid(Layer::Ground, cell.x + corner.offset().0, cell.z + corner.offset().1)
                .unwrap_or(0);
            match project(camera, target, wx, height, wz) {
                Some((x, y, depth)) => {
                    let (dx, dz) = corner.offset();
                    *slot = Vertex {
                        x,
                        y,
                        depth,
                        // One cell spans the full 256-unit texture space.
                        u: (dx * 255) as f32,
                        v: (dz * 255) as f32,
                    };
                }
                None => {
                    visible = false;
                    break;
                }
            }
        }
        if !visible {
            drawn.clipped += 1;
            continue;
        }
        target.triangle(points, texture, &shade);
        drawn.ground += 1;
    }
}

fn draw_box(
    target: &mut Target,
    scene: &Scene,
    camera: &Camera,
    cell: Cell,
    origin: (i32, i32),
    layer: Layer,
    drawn: &mut Drawn,
) {
    let Some((bottom, top)) = scene.grid.box_span(layer, cell) else {
        return;
    };
    if bottom == top {
        return;
    }
    let intensity = 255;
    let shade = shade_for(scene, camera, intensity);

    // Four sides and a top. The bottom is never seen from the air, and which
    // of slots 0-3 faces which way is not settled, so the sides are drawn with
    // the pair that matches their axis and the first of the pair.
    let faces: [( [Corner; 4], usize ); 5] = [
        ([Corner::Origin, Corner::X, Corner::X, Corner::Origin], 0),
        ([Corner::Z, Corner::Far, Corner::Far, Corner::Z], 1),
        ([Corner::X, Corner::Far, Corner::Far, Corner::X], 2),
        ([Corner::Origin, Corner::Z, Corner::Z, Corner::Origin], 3),
        ([Corner::Origin, Corner::X, Corner::Far, Corner::Z], 4),
    ];

    for (corners, slot) in faces {
        let word = scene
            .grid
            .box_texture(layer, cell, hb_formats::terrain::BoxFace::ALL[slot])
            .map(TextureRef)
            .unwrap_or_default();
        let Some(Some(texture)) = scene.textures.get(word.index() as usize) else {
            continue;
        };
        // A side face uses bottom, bottom, top, top; the top face is flat.
        let heights = if slot == 4 {
            [top, top, top, top]
        } else {
            [bottom, bottom, top, top]
        };
        let mut quad = [Vertex { x: 0.0, y: 0.0, depth: 0.0, u: 0.0, v: 0.0 }; 4];
        let mut visible = true;
        for (i, corner) in corners.into_iter().enumerate() {
            let (wx, wz) = corner_world(origin, corner);
            match project(camera, target, wx, heights[i], wz) {
                Some((x, y, depth)) => {
                    let (u, v) = match slot {
                        4 => {
                            let (dx, dz) = corner.offset();
                            ((dx * 255) as f32, (dz * 255) as f32)
                        }
                        _ => (
                            if i == 0 || i == 3 { 0.0 } else { 255.0 },
                            if i < 2 { 255.0 } else { 0.0 },
                        ),
                    };
                    quad[i] = Vertex { x, y, depth, u, v };
                }
                None => {
                    visible = false;
                    break;
                }
            }
        }
        if !visible {
            drawn.clipped += 1;
            continue;
        }
        target.triangle([quad[0], quad[1], quad[2]], texture, &shade);
        target.triangle([quad[0], quad[2], quad[3]], texture, &shade);
        drawn.boxes += 2;
    }
}

/// The height of the ground under a world position, for placing the camera.
pub fn ground_height(grid: &Grid, x: i32, z: i32) -> i32 {
    let cell = Cell::containing(x, z);
    grid.height_at_grid(Layer::Ground, cell.x, cell.z).unwrap_or(0)
}

/// The number of cells on a side, re-exported so callers need not reach past
/// this crate for it.
pub const GRID_SIDE: usize = SIDE;
