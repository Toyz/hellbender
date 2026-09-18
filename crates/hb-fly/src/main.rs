//! Fly around a Hellbender level.
//!
//! A window, a keyboard and the port's renderer. The flight model here is not
//! the game's - `hb-sim` will be, when the original's is read - it is enough to
//! move an eye through the world and see whether the world is right.

use std::path::PathBuf;
use std::time::Instant;

use hb_formats::terrain::CELL_SIZE;
use hb_formats::Angle;
mod battle;
mod sound;

use hb_formats::font::Font;
use hb_formats::raw::Image;
use hb_pod::Pod;
use hb_render::{Camera, Level, Target};
use minifb::{Key, Window, WindowOptions};

const USAGE: &str = "\
hb-fly - fly around a Hellbender level

  hb-fly [level] [--mode 200|400|480] [--scale N] [--demo 1|2|3]

  level    a level stem, default `hoth`
  --mode   the game's three screen sizes, default 200 (320x200)
  --scale  integer upscale of the window, default 3
  --demo   replay one of the game's recorded attract-mode flights

  arrows        up dives, down climbs, left and right turn (and bank)
  x / z  w / s  throttle up and down  a / d  home / pgup   roll
  shift         afterburner           tab     cycle the level
  c             collision on/off      k       cockpit on/off
  m             music on/off          h       hud on/off
  space         fire                  esc     quit
";

fn game_dir() -> PathBuf {
    std::env::var_os("HB_GAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("original"))
}

/// How far above the surface the eye is held when collision is on.
const CLEARANCE: i32 = 2 << 16;

/// The player's ship and the eye that rides in it.
struct Flight {
    ship: hb_sim::flight::Ship,
    camera: Camera,
    collide: bool,
    /// Set on the frame the ship was pushed up out of the ground.
    grounded: bool,
}

impl Flight {
    fn new(camera: Camera) -> Flight {
        let at = |v: i32| v as f32 / 65536.0;
        let mut ship = hb_sim::flight::Ship::new([at(camera.x), at(camera.y), at(camera.z)], camera.yaw.0 as f32);
        // Start under way at half throttle rather than parked in the air.
        ship.throttle = 0.5;
        let mut flight = Flight { ship, camera, collide: true, grounded: false };
        flight.sync_camera();
        flight
    }

    /// The engine's flight model, `hb_sim::flight`, with the keys the shipped
    /// `HELLBEND.INI` binds - arrows to steer, Z and X for the throttle, Home
    /// and PgUp to roll - and some friendlier ones alongside.
    fn step(&mut self, window: &Window, dt: f32) {
        let down = |keys: &[Key]| keys.iter().any(|&k| window.is_key_down(k));
        let controls = hb_sim::flight::Controls {
            up: down(&[Key::Up]),
            down: down(&[Key::Down]),
            left: down(&[Key::Left]),
            right: down(&[Key::Right]),
            roll_left: down(&[Key::Home, Key::A]),
            roll_right: down(&[Key::PageUp, Key::D]),
            throttle_up: down(&[Key::X, Key::W]),
            throttle_down: down(&[Key::Z, Key::S]),
            afterburner: down(&[Key::LeftShift, Key::RightShift]),
        };
        self.ship.step(&controls, dt);
        // The engine keeps every position in the signed world.
        let wrap = |v: f32| (v + 512.0).rem_euclid(1024.0) - 512.0;
        self.ship.position[0] = wrap(self.ship.position[0]);
        self.ship.position[2] = wrap(self.ship.position[2]);
        self.sync_camera();
    }

    fn sync_camera(&mut self) {
        let [x, y, z] = self.ship.position;
        let fixed = |v: f32| (v * 65536.0) as i32;
        let [pitch, roll, heading] = self.ship.angles();
        self.camera.x = fixed(x);
        self.camera.y = fixed(y);
        self.camera.z = fixed(z);
        self.camera.pitch = Angle(pitch as i32 as u16);
        self.camera.roll = Angle(roll as i32 as u16);
        self.camera.yaw = Angle(heading as i32 as u16);
    }

    /// Keep the ship above the ground and above anything standing on it.
    ///
    /// The engine has a real collision system - `intersectingBoxSurface` and
    /// the ground triangle queries are part of it - and this is not it. It is
    /// the height query used honestly: find the top of whatever is under the
    /// ship and refuse to go below it.
    fn settle(&mut self, grid: &hb_world::Grid) {
        self.grounded = false;
        if !self.collide {
            return;
        }
        let floor = grid.ceiling_of_solid(self.camera.x, self.camera.z) + CLEARANCE;
        if self.camera.y < floor {
            self.ship.position[1] = floor as f32 / 65536.0;
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
    let mut demo_number: Option<u32> = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--mode" => mode = it.next().and_then(|v| v.parse().ok()).unwrap_or(200),
            "--scale" => scale = it.next().and_then(|v| v.parse().ok()).unwrap_or(3),
            "--demo" => demo_number = it.next().and_then(|v| v.parse().ok()),
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

    // A demo names its own level. Two of the three name one that did not ship.
    let demo = match demo_number {
        Some(n) => {
            let bytes = startup
                .read("demo", &format!("demo{n}.dmo"))
                .map_err(|e| e.to_string())?;
            let demo = hb_formats::demo::Demo::parse(bytes).map_err(|e| e.to_string())?;
            let stem = demo.level.trim_end_matches(".lvl").to_string();
            if !LEVELS.contains(&stem.as_str()) {
                return Err(format!(
                    "demo {n} was recorded on {}, which is not in GAME.POD - it is one of \
                     the levels that did not ship",
                    demo.level
                ));
            }
            println!(
                "demo {n}: {:.1} s on {}, {} poses, {} key presses",
                demo.seconds(),
                demo.level,
                demo.poses().count(),
                demo.keys().count()
            );
            level_name = stem;
            Some(demo)
        }
        None => None,
    };

    let mut index = LEVELS.iter().position(|l| *l == level_name).unwrap_or(2);
    let mut level = Level::load(&game, Some(&startup), LEVELS[index])?;
    let describe = |level: &Level| {
        println!(
            "{}: {} terrain textures plus {} animation frames, {} cycles, \
             {} objects, {}x{} screen",
            level.stem,
            level.texture_names.len(),
            level.textures.len() - level.texture_names.len(),
            level.animations.len(),
            level.placements.len(),
            w,
            h
        );
    };
    describe(&level);

    // The cockpit is drawn for each of the three modes, in VGA.ACT, which a
    // level's own palette agrees with on 240 of 256 entries.
    let cockpit: Option<Image> = startup
        .read("art", &format!("ckpt{mode}.raw"))
        .ok()
        .and_then(|b| Image::parse_guessed(b).ok().flatten());
    let mut show_cockpit = cockpit.is_some();
    println!(
        "cockpit: {}",
        if show_cockpit { "ckpt art loaded" } else { "not found" }
    );

    // The level's music, if a device will take it.
    let music = match sound::Music::open() {
        Ok(music) => {
            println!("audio: {} Hz", music.rate);
            Some(music)
        }
        Err(why) => {
            println!("audio: {why}");
            None
        }
    };
    let play_music = |level: &Level| {
        let (Some(music), Some((dir, file))) = (music.as_ref(), level.manifest.slot("music"))
        else {
            return;
        };
        match game
            .read(dir, file)
            .or_else(|_| startup.read(dir, file))
            .ok()
            .and_then(|b| hb_audio::Module::parse(b).ok())
        {
            Some(module) => {
                println!("music: {file} - {:?}", module.title);
                music.play(module);
            }
            None => println!("music: {file} could not be loaded"),
        }
    };
    play_music(&level);

    // The front end's typeface, for the readout.
    let hud_font = (|| {
        let index = startup.read("startup", "font.ndx").ok()?;
        let bitmap = startup.read("startup", "font.bin").ok()?;
        Font::parse(index, bitmap).ok()
    })();
    let mut show_hud = hud_font.is_some();

    let mut target = Target::new(w, h);
    // The game drew 320x200 and 320x400 for a 4:3 monitor, so its pixels were
    // taller than wide; the window is 4:3 and minifb stretches the frame to
    // fill it, which gives the picture its original proportions. 640x480 is
    // already 4:3.
    let mut window = Window::new(
        "Hellbender",
        w * scale,
        w * scale * 3 / 4,
        WindowOptions { resize: true, ..WindowOptions::default() },
    )
    .map_err(|e| e.to_string())?;
    window.set_target_fps(60);

    let mut flight = Flight::new(start_of(&level));
    let mut buffer = vec![0u32; w * h];
    // Placements that follow a course move; the rest stand still. `live` is
    // the copy the renderer draws, rewritten from the followers each frame.
    let mut followers = followers_for(&level);
    let mut live = level.placements.clone();
    println!("sim: {} of {} objects follow a course", followers.len(), live.len());

    // Combat. The laser sound is the one the engine names; a destroyed
    // object plays its type's own destroy sound, falling back to a blast.
    let mut sounds: std::collections::HashMap<String, Option<std::sync::Arc<hb_audio::Wav>>> =
        std::collections::HashMap::new();
    let mut sound = |name: &str| -> Option<std::sync::Arc<hb_audio::Wav>> {
        sounds
            .entry(name.to_string())
            .or_insert_with(|| {
                let bytes = startup.read("sound", name).or_else(|_| game.read("sound", name)).ok()?;
                hb_audio::Wav::parse(bytes).ok().map(std::sync::Arc::new)
            })
            .clone()
    };
    let mut battle = battle::Battle::new(&level);
    let mut cooldown = 0.0f32;
    let mut colours = ShotColours::for_palette(&level.palette);
    // The ship's velocity, from how far the eye moved last frame; turrets
    // lead with it and the laser adds its magnitude.
    let mut last_eye = eye_of(&flight.camera);
    println!("sim: {} turrets", battle.turret_count());

    let started = Instant::now();
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
            describe(&level);
            play_music(&level);
            followers = followers_for(&level);
            live = level.placements.clone();
            battle = battle::Battle::new(&level);
            colours = ShotColours::for_palette(&level.palette);
            last_eye = eye_of(&flight.camera);
            println!(
                "sim: {} of {} objects follow a course, {} turrets",
                followers.len(),
                live.len(),
                battle.turret_count()
            );
        }
        tab_was_down = tab;

        if window.is_key_pressed(Key::H, minifb::KeyRepeat::No) {
            show_hud = !show_hud && hud_font.is_some();
        }
        if window.is_key_pressed(Key::M, minifb::KeyRepeat::No) {
            match music.as_ref() {
                Some(m) if m.playing() => m.stop(),
                Some(_) => play_music(&level),
                None => {}
            }
        }
        if window.is_key_pressed(Key::K, minifb::KeyRepeat::No) {
            show_cockpit = !show_cockpit && cockpit.is_some();
        }
        if window.is_key_pressed(Key::C, minifb::KeyRepeat::No) {
            flight.collide = !flight.collide;
            println!("collision {}", if flight.collide { "on" } else { "off" });
        }
        match &demo {
            // Replaying: the camera is wherever the original game recorded it,
            // looping. The recorded angles are pitch, roll and heading in the
            // engine's 16-bit circle.
            Some(demo) => {
                let length = demo.seconds().max(0.1);
                let t = started.elapsed().as_secs_f32() % length;
                if let Some(pose) = demo.pose_at(t) {
                    flight.camera.x = pose.x;
                    flight.camera.y = pose.y;
                    flight.camera.z = pose.z;
                    flight.camera.pitch = Angle(pose.angles[0] as u16);
                    flight.camera.roll = Angle(pose.angles[1] as u16);
                    flight.camera.yaw = Angle(pose.angles[2] as u16);
                }
            }
            None => {
                flight.step(&window, dt);
                flight.settle(&hb_world::Grid::new(&level.terrain));
            }
        }
        target.clear(0);
        let eye = eye_of(&flight.camera);
        let velocity = if dt > 0.0 {
            [
                hb_sim::combat::wrapped(eye[0] - last_eye[0]) / dt,
                (eye[1] - last_eye[1]) / dt,
                hb_sim::combat::wrapped(eye[2] - last_eye[2]) / dt,
            ]
        } else {
            [0.0; 3]
        };
        last_eye = eye;

        // Fire: a shot every tenth of a second while the button is held. The
        // rate is this port's choice; speed and damage are the engine's.
        cooldown -= dt;
        if window.is_key_down(Key::Space) && cooldown <= 0.0 && demo.is_none() && battle.pilot.alive() {
            cooldown = 0.1;
            let from = [eye[0], eye[1] - 0.5, eye[2]];
            battle.fire(hb_sim::combat::Shot::player_laser(
                from,
                flight.camera.yaw.0,
                flight.camera.pitch.0,
                flight.ship.speed(),
            ));
            if let (Some(music), Some(s)) = (music.as_ref(), sound("laser.wav")) {
                music.effect(&s, 0.5);
            }
        }

        let grid = hb_world::Grid::new(&level.terrain);
        let solid = |p: [f32; 3]| {
            let (x, z) = ((p[0] * 65536.0) as i32, (p[2] * 65536.0) as i32);
            (p[1] * 65536.0) as i32 <= grid.ceiling_of_solid(x, z)
        };
        // In a demo the recorded flight cannot dodge, so nothing shoots back.
        let noises = if demo.is_none() {
            battle.step(&level, &mut live, eye, velocity, dt, &solid)
        } else {
            battle.step(&level, &mut live, [0.0, 1.0e6, 0.0], [0.0; 3], dt, &|_| false)
        };
        for noise in noises {
            let (name, volume) = match noise {
                battle::Noise::Destroyed(kind) => {
                    let own = level.destroy_sound[kind]
                        .as_ref()
                        .and_then(|b| hb_audio::Wav::parse(b).ok())
                        .map(std::sync::Arc::new);
                    if let (Some(music), Some(s)) = (music.as_ref(), own.or_else(|| sound("blast4.wav"))) {
                        music.effect(&s, 0.8);
                    }
                    continue;
                }
                battle::Noise::PlayerHit(n) => (format!("exp{}.wav", n + 1), 0.8),
                battle::Noise::NearMiss(kind) => (hb_sim::combat::near_miss_sound(kind).to_string(), 0.6),
                battle::Noise::Died => {
                    println!("shot down ({} so far) - back to the start", battle.deaths);
                    ("blast7.wav".to_string(), 1.0)
                }
            };
            if let (Some(music), Some(s)) = (music.as_ref(), sound(&name)) {
                music.effect(&s, volume);
            }
        }
        if !battle.pilot.alive() {
            // The port's own: start again where the level starts, whole. The
            // engine's death - an explosion, a wreck, the mission's end - has
            // not been read.
            flight = Flight::new(start_of(&level));
            battle.pilot = hb_sim::combat::Pilot::default();
            last_eye = eye_of(&flight.camera);
        }

        for (i, follower) in &mut followers {
            if battle.health[*i].destroyed {
                continue;
            }
            // Only actors within the engine's 80-unit box think.
            if !hb_sim::combat::in_range(eye, hb_sim::combat::position_of(&live[*i])) {
                continue;
            }
            follower.step(dt);
            let [x, y, z] = follower.position_fixed();
            let placed = &mut live[*i];
            placed.x = x;
            placed.y = y;
            placed.z = z;
            placed.heading = follower.heading();
        }

        // Animated textures advance on the wall clock.
        let frames_now = level.texture_frames(started.elapsed().as_secs_f32());
        let mut scene = level.scene();
        scene.frames = Some(&frames_now);
        scene.sky_scroll = level.sky_scroll(started.elapsed().as_secs_f32());
        scene.placements = &live;
        hb_render::draw_world(&mut target, &scene, &flight.camera);
        // Shots are drawn at the copy of their position nearest the eye.
        let near = |p: [f32; 3]| {
            [
                eye[0] + hb_sim::combat::wrapped(p[0] - eye[0]),
                p[1],
                eye[2] + hb_sim::combat::wrapped(p[2] - eye[2]),
            ]
        };
        for flying in &battle.shots {
            let colour = match flying.shot.side {
                hb_sim::combat::Side::Player => colours.player,
                hb_sim::combat::Side::Enemy => colours.enemy,
            };
            hb_render::scene::draw_spark(&mut target, &flight.camera, near(flying.shot.position), colour);
        }
        for missile in &battle.missiles {
            hb_render::scene::draw_spark(&mut target, &flight.camera, near(missile.position), colours.missile);
        }
        if show_cockpit {
            if let Some(art) = &cockpit {
                target.overlay(art);
            }
        }
        if show_hud {
            if let Some(font) = &hud_font {
                let readout = format!(
                    "{}  ALT {:.0}  SPD {:.0}  HP {:.0}  KILLS {}",
                    level.stem.to_uppercase(),
                    flight.camera.y as f32 / 65536.0,
                    flight.ship.speed(),
                    battle.pilot.health * 100.0,
                    battle.destroyed
                );
                // The font is drawn at its authored size, which is 23 pixels
                // tall - more than a tenth of a 200-line screen, so it sits in
                // the top corner and is meant to be read, not admired.
                font.draw(&mut target.colour, w, h, 4, 3, &readout, Some(255));
            }
        }

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
                flight.ship.speed(),
                if flight.grounded { "  [on the deck]" } else { "" }
            );
            frames = 0;
            since = Instant::now();
        }
    }
    Ok(())
}

/// Every placement whose type names a course that exists, paired with a
/// follower for it. A dangling course id - six levels have them - is skipped,
/// as the engine's "Bad course ID for enemy" diagnostic implies it copes.
fn followers_for(level: &Level) -> Vec<(usize, hb_sim::Follower)> {
    level
        .placements
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let c = level.kinds.get(p.kind)?.course;
            let course = level.courses.get(usize::try_from(c).ok()?)?;
            Some((i, hb_sim::Follower::new(course, [p.x, p.y, p.z])?))
        })
        .collect()
}

fn eye_of(camera: &Camera) -> [f32; 3] {
    [camera.x as f32 / 65536.0, camera.y as f32 / 65536.0, camera.z as f32 / 65536.0]
}

/// Which palette entries draw shots. All three come from the reserved range,
/// 240-255, which the renderer's light and fog ramps leave alone, so a shot is
/// equally bright near and far. The engine draws shots as models; these points
/// stand in for them.
struct ShotColours {
    player: u8,
    enemy: u8,
    missile: u8,
}

impl ShotColours {
    fn for_palette(palette: &hb_formats::act::Palette) -> ShotColours {
        let nearest = |want: [i32; 3]| -> u8 {
            (240u8..=255)
                .min_by_key(|&i| {
                    let [r, g, b] = palette.rgb(i);
                    let d = [r as i32 - want[0], g as i32 - want[1], b as i32 - want[2]];
                    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
                })
                .unwrap_or(255)
        };
        ShotColours {
            player: 255,
            enemy: nearest([255, 40, 20]),
            missile: nearest([255, 200, 40]),
        }
    }
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
