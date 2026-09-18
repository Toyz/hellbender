//! Walking the world and handing triangles to the rasteriser.

use hb_formats::act::Palette;
use hb_formats::colour::Ramp;
use hb_formats::raw::Image;
use hb_formats::terrain::{BoxFace, Layer, TextureRef, CELL_SIZE, SIDE};
use hb_world::{triangle, Cell, Corner, Grid, Half};

use crate::camera::Camera;
use crate::raster::{Shade, Target, Vertex};

pub struct Scene<'a> {
    pub grid: Grid<'a>,
    pub textures: &'a [Option<Image>],
    pub palette: &'a Palette,
    pub light: Option<&'a Ramp>,
    pub fog: Option<&'a Ramp>,
    /// Per-slot texture substitution for this instant, from
    /// [`crate::Level::texture_frames`]. `None` leaves every texture static.
    pub frames: Option<&'a [u16]>,
    pub sky: Option<&'a Image>,
    pub sky_remap: Option<&'a [u8; 256]>,
    /// The level's placed objects, and a mesh per kind.
    pub placements: &'a [hb_formats::text::Placement],
    pub meshes: &'a [Option<hb_formats::mrgl::Model>],
    /// Which texture each mesh material names, resolved once.
    pub mesh_textures: &'a [Vec<Option<Image>>],
    /// The 16.16 radius each mesh is drawn at. Models are normalised to
    /// +/-1.0, so this is the object's half-extent in the world.
    pub mesh_radius: &'a [i32],
    /// The level's ambient light, 0-255: `.LVL` line 19 over 256. A ground
    /// vertex whose shade word has bit 8 set takes this.
    pub ambient: u8,
}

/// Draws the ground and both box sets from the camera's position.
///
/// The traversal is this renderer's own: the engine's visibility scheme has not
/// been read. This walks every cell within `camera.far` of the eye, back to
/// front, and lets the depth buffer settle the rest.
pub fn draw_world(target: &mut Target, scene: &Scene, camera: &Camera) -> Drawn {
    let mut drawn = Drawn::default();
    // Everything past the draw distance is fully fogged, and the fog ramp's
    // last row sends every colour to one index - 255 in FLOAT, JURASIC and
    // KREASH, 0 in HOTH and ROID. Filling the frame with it first means the
    // band between the last cell drawn and the horizon reads as distance
    // rather than as a hole.
    if let Some(fog) = scene.fog {
        target.colour.fill(fog.shade(15, 1));
    }
    draw_sky(target, scene, camera);
    let reach = (camera.far / CELL_SIZE).max(1);
    // The eye's cell before wrapping. The camera's coordinates are signed and
    // unbounded, so the walk has to start from the same frame the camera is
    // in: a camera at x = -10 units is in cell -2, whose data is in grid
    // column 126 but whose ground is drawn at -16 units, not +1008.
    let eye = (camera.x >> 19, camera.z >> 19);

    // Back to front by cell distance, so the painter's order is roughly right
    // before the depth buffer even runs.
    let mut cells: Vec<(i32, i32, i32)> = Vec::new();
    for dz in -reach..=reach {
        for dx in -reach..=reach {
            let d = dx * dx + dz * dz;
            if d > reach * reach {
                continue;
            }
            cells.push((d, eye.0 + dx, eye.1 + dz));
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
    // Signed: a nose-up pitch is a small negative angle, not nearly a turn.
    let pitch = camera.pitch.to_signed_radians();

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
    let eye = (camera.x >> 19, camera.z >> 19);
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
        let radius = scene.mesh_radius.get(p.kind).copied().unwrap_or(1 << 16);
        let (wx, wz) = rebase(eye, p.x, p.z);
        if draw_mesh(target, scene, camera, mesh, textures, wx, p.y, wz, radius, p.heading) {
            drawn.models += 1;
        }
    }
}

/// Move a signed world coordinate into the unwrapped frame the terrain walk
/// uses, choosing the copy of the wrapping world nearest the eye. `eye` is the
/// eye's cell before wrapping.
fn rebase(eye: (i32, i32), x: i32, z: i32) -> (i32, i32) {
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
    (one(eye.0, x), one(eye.1, z))
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
    // vertex into a 16.16 world offset at the type's radius.
    let (width, height) = (target.width, target.height);
    let place = |v: &hb_formats::mrgl::Vertex| -> Option<(f32, f32, f32)> {
        let [mx, my, mz] = v.world(scale);
        let (mx, mz) = (mx as f32, mz as f32);
        let rx = mx * cy + mz * sy;
        let rz = -mx * sy + mz * cy;
        project_onto(camera, width, height, x + rx as i32, y + my, z + rz as i32)
    };

    let shade = shade_for(scene, camera);
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
                    light: 255.0,
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

/// A texture slot after animation.
fn slot(scene: &Scene, index: u16) -> usize {
    match scene.frames {
        Some(frames) => *frames.get(index as usize).unwrap_or(&index) as usize,
        None => index as usize,
    }
}

fn shade_for<'a>(scene: &'a Scene, camera: &Camera) -> Shade<'a> {
    Shade {
        light: scene.light,
        fog: scene.fog,
        fog_distance: (camera.far / 65536) as f32,
        index_zero_is_clear: false,
    }
}

/// A texture's corner coordinates, half a texel in from each edge of a
/// 64-texel texture in the 256-unit space: `0x413c20` and `0x414f60` write
/// `0x20000` and `0xfe0000`, 2.0 and 254.0.
const UV_LO: f32 = 2.0;
const UV_HI: f32 = 254.0;

/// Which of a quad's corners `a b c d` a cell corner is, in the engine's
/// order: (x, z), (x+1, z), (x+1, z+1), (x, z+1).
fn quad_corner(corner: Corner) -> usize {
    match corner.offset() {
        (0, 0) => 0,
        (1, 0) => 1,
        (1, 1) => 2,
        _ => 3,
    }
}

/// The light at one grid point of the ground.
///
/// The engine gives each of a cell's four vertices the shade of the cell whose
/// origin that vertex is (`0x414e05` to `0x414ea7`): the low byte of the
/// shading word, or, when bit 8 is set, the level's ambient (`[0x525c5c]`,
/// the `.LVL`'s line 19). So the shading database is a value per grid point,
/// and the ground is Gouraud shaded between them.
fn ground_light(scene: &Scene, x: i32, z: i32) -> f32 {
    match scene.grid.terrain.shading.as_ref() {
        Some(s) if s.ground_flag(x, z) => scene.ambient as f32,
        Some(s) => s.ground_intensity(x, z) as f32,
        None => 255.0,
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
    let Some(Some(texture)) = scene.textures.get(slot(scene, word.index())) else {
        return;
    };
    let height = |dx: i32, dz: i32| {
        scene.grid.height_at_grid(Layer::Ground, cell.x + dx, cell.z + dz).unwrap_or(0)
    };
    // Ground that a box of set A encloses is never seen, and the engine does
    // not draw it: a cell whose four corners all lie within its box's span is
    // skipped (`0x414d43`).
    if let Some((bottom, top)) = scene.grid.box_span(Layer::BoxA, cell) {
        let corners = [height(0, 0), height(1, 0), height(1, 1), height(0, 1)];
        if bottom != top && corners.iter().all(|&h| h >= bottom && h <= top) {
            return;
        }
    }
    let uvs = word.corner_uvs(UV_LO, UV_HI);
    let shade = shade_for(scene, camera);

    for half in [Half::First, Half::Second] {
        let tri = triangle(cell, half);
        let mut points = [Vertex::default(); 3];
        let mut visible = true;
        for (slot, corner) in points.iter_mut().zip(tri.corners) {
            let (dx, dz) = corner.offset();
            let (wx, wz) = corner_world(origin, corner);
            match project(camera, target, wx, height(dx, dz), wz) {
                Some((x, y, depth)) => {
                    let (u, v) = uvs[quad_corner(corner)];
                    let light = ground_light(scene, cell.x + dx, cell.z + dz);
                    *slot = Vertex { x, y, depth, u, v, light };
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
/// the same split and the same corner heights apply. The floor is the `.CL1`
/// entry's first word and the ceiling its second: the engine draws them
/// through the ground's own cell routine, `0x4144b0`, from `0x41911a` with the
/// word at chamber `+4` and from `0x41957a` with the word at `+6` and its
/// flip flag set. So their texture coordinates follow the ground's rule too.
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
    // The chamber's 24-bit shading value is not decomposed; its low byte
    // stands in, flat across the cell.
    let light = scene
        .grid
        .terrain
        .shading
        .as_ref()
        .map(|s| s.chambers[cell.index()][0] as f32)
        .unwrap_or(255.0);
    let shade = shade_for(scene, camera);

    for (face, layer) in [(0usize, Layer::ChamberFloor), (1, Layer::ChamberCeiling)] {
        let word = scene.grid.terrain.chambers.textures.texture_at(cell.x, cell.z, face);
        let Some(Some(texture)) = scene.textures.get(slot(scene, word.index())) else {
            continue;
        };
        let uvs = word.corner_uvs(UV_LO, UV_HI);
        for half in [Half::First, Half::Second] {
            let tri = triangle(cell, half);
            let mut points = [Vertex::default(); 3];
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
                        let (u, v) = uvs[quad_corner(corner)];
                        *point = Vertex { x, y, depth, u, v, light };
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

/// A box: four sides and a top, each from its own slot, with the engine's
/// corners and texture coordinates (see [`BoxFace`]).
///
/// The bottom is not drawn: it cannot be seen from outside. A side is skipped
/// when the neighbouring box on that side covers it top to bottom, as the
/// engine does before each face.
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
    // A box's `.LTE` byte is not a shade but eight shadow bits, one per
    // corner: the computation at `0x41c8cd` clears it, then for each corner
    // casts 48 units toward the light (`0x413580`) and sets the corner's bit
    // if something is in the way. The drawer gives a shadowed corner the
    // level's ambient and a lit one full light (`0x415e45`). Bit n is taken to
    // be box vertex n - bottom corners 0-3 then top 4-7, in the ground's
    // order - which is the order the first two tests run in.
    let shadows = scene
        .grid
        .terrain
        .shading
        .as_ref()
        .map(|s| match layer {
            Layer::BoxB => s.box_b[cell.index()],
            _ => s.box_a[cell.index()],
        })
        .unwrap_or(0);
    let corner_light = |dx: i32, dz: i32, up: bool| {
        let n = match (dx, dz) {
            (0, 0) => 0,
            (1, 0) => 1,
            (1, 1) => 2,
            _ => 3,
        } + if up { 4 } else { 0 };
        if shadows & (1 << n) != 0 {
            scene.ambient as f32
        } else {
            255.0
        }
    };
    let shade = shade_for(scene, camera);

    for face in [BoxFace::NegZ, BoxFace::PosZ, BoxFace::PosX, BoxFace::NegX, BoxFace::Top] {
        let neighbour = match face {
            BoxFace::NegZ => Some((0, -1)),
            BoxFace::PosZ => Some((0, 1)),
            BoxFace::PosX => Some((1, 0)),
            BoxFace::NegX => Some((-1, 0)),
            _ => None,
        };
        if let Some((dx, dz)) = neighbour {
            let next = Cell::new(cell.x + dx, cell.z + dz);
            if let Some((b, t)) = scene.grid.box_span(layer, next) {
                if b != t && b <= bottom && t >= top {
                    continue;
                }
            }
        }
        let word = scene
            .grid
            .box_texture(layer, cell, face)
            .map(TextureRef)
            .unwrap_or_default();
        let Some(Some(texture)) = scene.textures.get(slot(scene, word.index())) else {
            continue;
        };
        let uvs = word.corner_uvs(UV_LO, UV_HI);
        let mut quad = [Vertex::default(); 4];
        let mut visible = true;
        for (i, (dx, dz, up)) in face.corners().into_iter().enumerate() {
            let (wx, wz) = (origin.0 + dx * CELL_SIZE, origin.1 + dz * CELL_SIZE);
            let y = if up { top } else { bottom };
            match project(camera, target, wx, y, wz) {
                Some((x, sy, depth)) => {
                    let (u, v) = uvs[i];
                    let light = corner_light(dx, dz, up);
                    quad[i] = Vertex { x, y: sy, depth, u, v, light };
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

/// A bright point in the world - a shot in flight - drawn as a small square
/// that shrinks with distance and respects the depth buffer.
///
/// The engine's own shot is a model (`bullet.bin` is in STARTUP.POD) and this
/// is a stand-in until weapons are read properly.
pub fn draw_spark(target: &mut Target, camera: &Camera, position: [f32; 3], index: u8) {
    let at = [
        (position[0] * 65536.0) as i32,
        (position[1] * 65536.0) as i32,
        (position[2] * 65536.0) as i32,
    ];
    let Some((sx, sy, depth)) = project_onto(camera, target.width, target.height, at[0], at[1], at[2])
    else {
        return;
    };
    let size = (target.width as f32 * 0.9 / depth).clamp(1.0, 5.0) as isize;
    let (cx, cy) = (sx as isize, sy as isize);
    for y in cy - size / 2..=cy + size / 2 {
        for x in cx - size / 2..=cx + size / 2 {
            if x < 0 || y < 0 || x as usize >= target.width || y as usize >= target.height {
                continue;
            }
            let i = y as usize * target.width + x as usize;
            if depth < target.depth[i] {
                target.colour[i] = index;
                target.depth[i] = depth;
            }
        }
    }
}
