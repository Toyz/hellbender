//! Fly around a Hellbender level.
//!
//! A window, a keyboard and the port's renderer. The flight model here is not
//! the game's - `hb-sim` will be, when the original's is read - it is enough to
//! move an eye through the world and see whether the world is right.

use std::path::PathBuf;
use std::time::Instant;

use hb_formats::terrain::CELL_SIZE;
use hb_formats::Angle;
use hb_pod::Pod;
use hb_render::{Camera, Level, Target};
use minifb::{Key, Window, WindowOptions};

const USAGE: &str = "\
hb-fly - fly around a Hellbender level

  hb-fly [level] [--mode 200|400|480] [--scale N]

  level    a level stem, default `hoth`
  --mode   the game's three screen sizes, default 200 (320x200)
  --scale  integer upscale of the window, default 3

  arrows        pitch and turn        w / s   throttle
  a / d         strafe                r / f   climb and dive
  space         stop                  tab     cycle the level
  c             collision on/off      esc     quit
";

fn game_dir() -> PathBuf {
    std::env::var_os("HB_GAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("original"))
}

/// How far above the surface the eye is held when collision is on.
const CLEARANCE: i32 = 2 << 16;

/// The eye, with just enough motion to steer it.
struct Flight {
    camera: Camera,
    /// World units per second along the view axis.
    speed: f32,
    collide: bool,
    /// Set on the frame the eye was pushed up out of the ground.
    grounded: bool,
}

impl Flight {
    fn new(camera: Camera) -> Flight {
        Flight { camera, speed: 0.0, collide: true, grounded: false }
    }

    fn step(&mut self, window: &Window, dt: f32) {
        let turn = |k: Key, amount: f32| if window.is_key_down(k) { amount } else { 0.0 };
        // A 16-bit circle, so a full turn is 65,536 and this is about a third
        // of a turn a second held down.
        let yaw = turn(Key::Right, 20_000.0) - turn(Key::Left, 20_000.0);
        let pitch = turn(Key::Down, 12_000.0) - turn(Key::Up, 12_000.0);
        self.camera.yaw = Angle(self.camera.yaw.0.wrapping_add((yaw * dt) as i32 as u16));
        self.camera.pitch =
            Angle(self.camera.pitch.0.wrapping_add((pitch * dt) as i32 as u16));

        self.speed += (turn(Key::W, 60.0) - turn(Key::S, 60.0)) * dt;
        if window.is_key_down(Key::Space) {
            self.speed = 0.0;
        }
        self.speed = self.speed.clamp(-90.0, 90.0);

        // Forward is the view axis; yaw 0 looks along +z.
        let (sy, cy) = self.camera.yaw.to_radians().sin_cos();
        let (sp, cp) = self.camera.pitch.to_radians().sin_cos();
        let forward = [sy * cp, -sp, cy * cp];
        let strafe = turn(Key::D, 40.0) - turn(Key::A, 40.0);
        let climb = turn(Key::R, 40.0) - turn(Key::F, 40.0);

        let units = |v: f32| (v * 65536.0) as i32;
        self.camera.x = self
            .camera
            .x
            .wrapping_add(units((forward[0] * self.speed + cy * strafe) * dt));
        self.camera.y = self
            .camera
            .y
            .wrapping_add(units((forward[1] * self.speed + climb) * dt));
        self.camera.z = self
            .camera
            .z
            .wrapping_add(units((forward[2] * self.speed - sy * strafe) * dt));
    }

    /// Keep the eye above the ground and above anything standing on it.
    ///
    /// The engine has a real collision system - `intersectingBoxSurface` and
    /// the ground triangle queries are part of it - and this is not it. It is
    /// the height query used honestly: find the top of whatever is under the
    /// eye and refuse to go below it.
    fn settle(&mut self, grid: &hb_world::Grid) {
        self.grounded = false;
        if !self.collide {
            return;
        }
        let floor = grid.ceiling_of_solid(self.camera.x, self.camera.z) + CLEARANCE;
        if self.camera.y < floor {
            self.camera.y = floor;
            self.grounded = true;
        }
    }
}

const LEVELS: [&str; 26] = [
    "float", "float2", "hoth", "hoth2", "hoth3", "iowah", "iowah2", "iowah3", "jurasic",
    "jurasic2", "jurasic3", "kreash", "kreash2", "kreash3", "morbos", "morbos2", "morbos3",
    "netlvl1", "netlvl2", "netlvl3", "roid", "roid2", "roid3", "roid4", "ship", "ship2",
];

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return Ok(());
    }
    let mut level_name = "hoth".to_string();
    let mut mode = 200usize;
    let mut scale = 3usize;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--mode" => mode = it.next().and_then(|v| v.parse().ok()).unwrap_or(200),
            "--scale" => scale = it.next().and_then(|v| v.parse().ok()).unwrap_or(3),
            other if !other.starts_with('-') => level_name = other.to_string(),
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }

    let game = Pod::open(game_dir().join("system/GAME.POD")).map_err(|e| e.to_string())?;
    let startup =
        Pod::open(game_dir().join("system/STARTUP.POD")).map_err(|e| e.to_string())?;

    let (w, h) = match mode {
        400 => Target::MODE_400,
        480 => Target::MODE_480,
        _ => Target::MODE_200,
    };

    let mut index = LEVELS.iter().position(|l| *l == level_name).unwrap_or(2);
    let mut level = Level::load(&game, Some(&startup), LEVELS[index])?;
    println!(
        "{}: {}/{} textures, {} screen",
        level.stem,
        level.resolved_textures(),
        level.texture_names.len(),
        format_args!("{w}x{h}")
    );

    let mut target = Target::new(w, h);
    let mut window = Window::new(
        "Hellbender",
        w * scale,
        h * scale,
        WindowOptions { resize: true, ..WindowOptions::default() },
    )
    .map_err(|e| e.to_string())?;
    window.set_target_fps(60);

    let mut flight = Flight::new(start_of(&level));
    let mut buffer = vec![0u32; w * h];
    let mut last = Instant::now();
    let mut tab_was_down = false;
    // The original's own floor was 8 frames a second - `autoMinFrameRate` in
    // HELLBEND.INI - so it is worth knowing where this sits.
    let (mut frames, mut since) = (0u32, Instant::now());

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let now = Instant::now();
        let dt = (now - last).as_secs_f32().min(0.1);
        last = now;

        let tab = window.is_key_down(Key::Tab);
        if tab && !tab_was_down {
            index = (index + 1) % LEVELS.len();
            level = Level::load(&game, Some(&startup), LEVELS[index])?;
            flight = Flight::new(start_of(&level));
            println!("{}", level.stem);
        }
        tab_was_down = tab;

        if window.is_key_pressed(Key::C, minifb::KeyRepeat::No) {
            flight.collide = !flight.collide;
            println!("collision {}", if flight.collide { "on" } else { "off" });
        }
        flight.step(&window, dt);
        flight.settle(&hb_world::Grid::new(&level.terrain));
        target.clear(0);
        let scene = level.scene();
        hb_render::draw_world(&mut target, &scene, &flight.camera);

        // The framebuffer is palette indices; minifb wants 0x00RRGGBB.
        for (slot, &index) in buffer.iter_mut().zip(&target.colour) {
            let [r, g, b] = level.palette.rgb(index);
            *slot = ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
        }
        window.update_with_buffer(&buffer, w, h).map_err(|e| e.to_string())?;

        frames += 1;
        if since.elapsed().as_secs_f32() >= 2.0 {
            let grid = hb_world::Grid::new(&level.terrain);
            let cell = hb_world::Cell::containing(flight.camera.x, flight.camera.z);
            let ground = grid
                .height_at(hb_formats::terrain::Layer::Ground, flight.camera.x, flight.camera.z)
                .unwrap_or(0);
            println!(
                "{:.0} fps  cell ({:3},{:3})  y {:7.1}  ground {:7.1}  speed {:6.1}{}",
                frames as f32 / since.elapsed().as_secs_f32(),
                cell.x,
                cell.z,
                flight.camera.y as f32 / 65536.0,
                ground as f32 / 65536.0,
                flight.speed,
                if flight.grounded { "  [on the deck]" } else { "" }
            );
            frames = 0;
            since = Instant::now();
        }
    }
    Ok(())
}

/// Start in the middle of the map, above whatever is there.
fn start_of(level: &Level) -> Camera {
    let grid = hb_world::Grid::new(&level.terrain);
    let (x, z) = (64 * CELL_SIZE, 64 * CELL_SIZE);
    let ground = grid.ceiling_of_solid(x, z);
    let mut camera = Camera::looking_at(x, ground + (25 << 16), z, Angle(0x2000));
    camera.pitch = Angle(2_000);
    camera
}
