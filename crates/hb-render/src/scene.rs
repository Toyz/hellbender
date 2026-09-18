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
    pub sky: Option<&'a Image>,
    pub sky_remap: Option<&'a [u8; 256]>,
    /// The level's placed objects, and a mesh per kind.
    pub placements: &'a [hb_formats::text::Placement],
    pub meshes: &'a [Option<hb_formats::mrgl::Model>],
    /// Which texture each mesh material names, resolved once.
    pub mesh_textures: &'a [Vec<Option<Image>>],
}

/// Draws the ground and both box sets from the camera's position.
///
/// The traversal is this renderer's own: the engine's visibility scheme has not
/// been read. This walks every cell within `camera.far` of the eye, back to
/// front, and lets the depth buffer settle the rest.
pub fn draw_world(target: &mut Target, scene: &Scene, camera: &Camera) -> Drawn {
    let mut drawn = Drawn::default();
    draw_sky(target, scene, camera);
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
        draw_chamber(target, scene, camera, cell, origin, &mut drawn);
    }
    draw_objects(target, scene, camera, &mut drawn);
    drawn
}

/// The sky, as a cylinder around the eye.
///
/// The engine's own projection is not known - it has a `skyTextureFlag` and a
/// `"Sky clip overflow!"` diagnostic and nothing else legible - so this wraps
/// the 64 x 64 texture once around the horizon and once from the horizon to
/// the zenith, which is the simplest thing that turns with the camera.
///
/// The sky is drawn before anything else and writes no depth, so everything
/// else covers it.
fn draw_sky(target: &mut Target, scene: &Scene, camera: &Camera) {
    let (Some(sky), Some(remap)) = (scene.sky, scene.sky_remap) else {
        return;
    };
    let (w, h) = (sky.shape.width, sky.shape.height);
    if w == 0 || h == 0 {
        return;
    }
    let half_fov = camera.fov.to_radians() / 2.0;
    let scale = (target.width as f32 / 2.0) / half_fov.tan();
    let yaw = camera.yaw.to_radians();
    let pitch = camera.pitch.to_radians();

    for y in 0..target.height {
        // Elevation of this scanline, from the screen offset and the focal
        // length, with the camera's pitch added.
        let dy = target.height as f32 / 2.0 - (y as f32 + 0.5);
        let elevation = (dy / scale).atan() - pitch;
        if elevation <= 0.0 {
            continue;
        }
        let v = ((elevation / (std::f32::consts::PI / 2.0)) * h as f32) as isize;
        let v = v.clamp(0, h as isize - 1) as usize;
        for x in 0..target.width {
            let dx = x as f32 + 0.5 - target.width as f32 / 2.0;
            let azimuth = (dx / scale).atan() + yaw;
            let turns = azimuth / std::f32::consts::TAU;
            let u = ((turns.rem_euclid(1.0)) * w as f32) as usize % w;
            let at = y * target.width + x;
            target.colour[at] = remap[sky.pixels[v * w + u] as usize];
        }
    }
}

/// The level's placed objects, back to front.
///
/// A placement's position is in the world's signed coordinates, so it has to be
/// rebased onto the same unwrapped frame the terrain walk uses - otherwise an
/// object at -300 units lands 1024 units away from the ground it stands on.
fn draw_objects(target: &mut Target, scene: &Scene, camera: &Camera, drawn: &mut Drawn) {
    let eye = Cell::containing(camera.x, camera.z);
    let mut order: Vec<(i64, usize)> = Vec::new();
    for (i, p) in scene.placements.iter().enumerate() {
        let (wx, wz) = rebase(eye, p.x, p.z);
        let [_, _, depth] = camera.to_view(wx, p.y, wz);
        if depth <= 0.0 || depth > (camera.far >> 16) as f32 {
            continue;
        }
        order.push((-(depth * 256.0) as i64, i));
    }
    order.sort();

    for (_, i) in order {
        let p = &scene.placements[i];
        let Some(Some(mesh)) = scene.meshes.get(p.kind) else {
            continue;
        };
        let textures = &scene.mesh_textures[p.kind];
        let (wx, wz) = rebase(eye, p.x, p.z);
        if draw_mesh(target, scene, camera, mesh, textures, wx, p.y, wz, p.scale, p.heading) {
            drawn.models += 1;
        }
    }
}

/// Move a signed world coordinate into the unwrapped frame the terrain walk
/// uses, choosing the copy of the wrapping world nearest the eye.
fn rebase(eye: Cell, x: i32, z: i32) -> (i32, i32) {
    let one = |eye_cell: i32, coord: i32| {
        let cell = (coord >> 19) & (SIDE as i32 - 1);
        // The nearest equivalent cell index to the eye's, modulo 128.
        let mut delta = cell - (eye_cell & (SIDE as i32 - 1));
        if delta > SIDE as i32 / 2 {
            delta -= SIDE as i32;
        } else if delta < -(SIDE as i32) / 2 {
            delta += SIDE as i32;
        }
        ((eye_cell + delta) << 19) | (coord & (CELL_SIZE - 1))
    };
    (one(eye.x, x), one(eye.z, z))
}

/// One MRGL mesh, at a position, scale and heading.
#[allow(clippy::too_many_arguments)]
fn draw_mesh(
    target: &mut Target,
    scene: &Scene,
    camera: &Camera,
    mesh: &hb_formats::mrgl::Model,
    textures: &[Option<Image>],
    x: i32,
    y: i32,
    z: i32,
    scale: i32,
    heading: u16,
) -> bool {
    let (sy, cy) = hb_formats::Angle(heading).to_radians().sin_cos();
    // Model space is 2.14 and spans -1.0 to +1.0, so `Vertex::world` turns a
    // vertex into a 16.16 world offset at the placement's scale.
    let (width, height) = (target.width, target.height);
    let place = |v: &hb_formats::mrgl::Vertex| -> Option<(f32, f32, f32)> {
        let [mx, my, mz] = v.world(scale);
        let (mx, mz) = (mx as f32, mz as f32);
        let rx = mx * cy + mz * sy;
        let rz = -mx * sy + mz * cy;
        project_onto(camera, width, height, x + rx as i32, y + my, z + rz as i32)
    };

    let shade = Shade {
        light: scene.light,
        fog: scene.fog,
        intensity: 255,
        fog_distance: (camera.far / 65536) as f32,
        index_zero_is_clear: false,
    };
    let mut any = false;
    for poly in &mesh.polygons {
        // Each polygon records the material node that preceded it, or a flat
        // colour when the mesh is untextured.
        let texture = poly.material.and_then(|m| textures.get(m)).and_then(Option::as_ref);
        if texture.is_none() && poly.colour.is_none() {
            continue;
        }
        let corners: Option<Vec<Vertex>> = poly
            .corners
            .iter()
            .map(|c| {
                let v = mesh.vertices.get(c.vertex as usize)?;
                let (sx, sy, depth) = place(v)?;
                Some(Vertex {
                    x: sx,
                    y: sy,
                    depth,
                    u: (c.u >> 16) as f32,
                    v: (c.v >> 16) as f32,
                })
            })
            .collect();
        let Some(corners) = corners else { continue };
        for i in 1..corners.len().saturating_sub(1) {
            let tri = [corners[0], corners[i], corners[i + 1]];
            match texture {
                Some(texture) => target.triangle(tri, texture, &shade),
                None => {
                    // The low byte is the palette index; bit 8 is set in three
                    // of the 270 flat-colour nodes and is not understood.
                    let index = (poly.colour.unwrap_or(0) & 0xff) as u8;
                    target.flat_triangle(tri, index, &shade);
                }
            }
        }
        any = true;
    }
    any
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Drawn {
    pub ground: usize,
    pub boxes: usize,
    pub chambers: usize,
    pub models: usize,
    pub clipped: usize,
}

fn corner_world(origin: (i32, i32), corner: Corner) -> (i32, i32) {
    let (dx, dz) = corner.offset();
    (origin.0 + dx * CELL_SIZE, origin.1 + dz * CELL_SIZE)
}

fn project(camera: &Camera, target: &Target, x: i32, y: i32, z: i32) -> Option<(f32, f32, f32)> {
    project_onto(camera, target.width, target.height, x, y, z)
}

fn project_onto(
    camera: &Camera,
    width: usize,
    height: usize,
    x: i32,
    y: i32,
    z: i32,
) -> Option<(f32, f32, f32)> {
    let [vx, vy, vz] = camera.to_view(x, y, z);
    // Near plane at a tenth of a cell.
    if vz <= 0.8 {
        return None;
    }
    let half_fov = camera.fov.to_radians() / 2.0;
    let scale = (width as f32 / 2.0) / half_fov.tan();
    Some((
        width as f32 / 2.0 + vx * scale / vz,
        height as f32 / 2.0 - vy * scale / vz,
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

/// A chamber's floor and ceiling, triangulated exactly like the ground.
///
/// Both are height fields that `heightAtGrid` accepts as layers 2 and 3, so
/// the same split and the same corner heights apply. The two textures in the
/// cell's `.CL1` entry are the floor's and the ceiling's, in that order -
/// which is the order the loader reads them and is not otherwise confirmed.
fn draw_chamber(
    target: &mut Target,
    scene: &Scene,
    camera: &Camera,
    cell: Cell,
    origin: (i32, i32),
    drawn: &mut Drawn,
) {
    if !scene.grid.has_chamber(cell) {
        return;
    }
    let intensity = scene
        .grid
        .terrain
        .shading
        .as_ref()
        .map(|s| s.chambers[cell.index()][0])
        .unwrap_or(255);
    let shade = shade_for(scene, camera, intensity);

    for (slot, layer) in [(0usize, Layer::ChamberFloor), (1, Layer::ChamberCeiling)] {
        let word = scene.grid.terrain.chambers.textures.texture_at(cell.x, cell.z, slot);
        let Some(Some(texture)) = scene.textures.get(word.index() as usize) else {
            continue;
        };
        for half in [Half::First, Half::Second] {
            let tri = triangle(cell, half);
            let mut points = [Vertex { x: 0.0, y: 0.0, depth: 0.0, u: 0.0, v: 0.0 }; 3];
            let mut visible = true;
            for (point, corner) in points.iter_mut().zip(tri.corners) {
                let (dx, dz) = corner.offset();
                let (wx, wz) = corner_world(origin, corner);
                let height = scene
                    .grid
                    .height_at_grid(layer, cell.x + dx, cell.z + dz)
                    .unwrap_or(0);
                match project(camera, target, wx, height, wz) {
                    Some((x, y, depth)) => {
                        *point = Vertex {
                            x,
                            y,
                            depth,
                            u: (dx * 255) as f32,
                            v: (dz * 255) as f32,
                        }
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
            drawn.chambers += 1;
        }
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
