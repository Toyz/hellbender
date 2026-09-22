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
mod keys;
mod stick;

use hb_formats::act;
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

  The keys are the game's own, from `[Control]` of system/hellbend.ini if
  there is one and the engine's defaults if not:

  arrows        up dives, down climbs, left and right turn (and bank)
  home / pgup   roll left and right    x / z   throttle up and down
  space         fire                   shift   afterburner
  ` 1 2 3       Vulcan cannon, dispersion cannon, SKL, RFL20
  4 5 6 7 8 9   Dead-On, cruise, Viper, cluster, MIRV, guided MIRV
  0             mine                   - / =   previous / next weapon
  v             lock the next target   b       drop a beacon
  , .           main energy to the weapons, to the shield
  t             crosshair on/off       /       label the cockpit
  esc           quit

  And the port's own switches, on keys the game does not bind:

  h  hud on/off     k  cockpit on/off    y  music on/off
  g  collision      p  cycle the level

  A chapter opens with its briefing screen; any key flies on. The level's
  mission runs from its .NAV file: the HUD shows the current
  objective's code, how far it is, and an arrow on the radar points at it.
  Flying into the jump zone, or finishing every objective, moves on to the
  next level; failing starts the level again.
";

fn game_dir() -> PathBuf {
    std::env::var_os("HB_GAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("original"))
}

/// The player's ship and the eye that rides in it.
struct Flight {
    ship: hb_sim::flight::Ship,
    camera: Camera,
    collide: bool,
    /// Set on the frame the ship was pushed up out of the ground.
    grounded: bool,
    /// Where it was before this frame, for backing out of rock.
    was: [f32; 3],
}

impl Flight {
    fn new(camera: Camera) -> Flight {
        let at = |v: i32| v as f32 / 65536.0;
        let mut ship = hb_sim::flight::Ship::new([at(camera.x), at(camera.y), at(camera.z)], camera.yaw.0 as f32);
        // Start under way at half throttle rather than parked in the air.
        ship.throttle = 0.5;
        let was = ship.position;
        let mut flight = Flight { ship, camera, collide: true, grounded: false, was };
        flight.sync_camera();
        flight
    }

    /// The engine's flight model, `hb_sim::flight`, with the keys the shipped
    /// `HELLBEND.INI` binds - arrows to steer, Z and X for the throttle, Home
    /// and PgUp to roll - and some friendlier ones alongside.
    /// `fuel` says whether the afterburner has anything to burn.
    fn step(
        &mut self,
        window: &Window,
        binds: &keys::Bindings,
        joystick: Option<&stick::Stick>,
        dt: f32,
        fuel: bool,
    ) {
        let held = |key: Option<Key>| key.is_some_and(|k| window.is_key_down(k));
        let down = |keys: &[Key]| keys.iter().any(|&k| window.is_key_down(k));
        let deflection = joystick.map(|s| s.stick()).filter(|d| d.iter().any(|v| *v != 0.0));
        let lever = joystick.filter(|s| s.has_lever()).and_then(|s| s.lever());
        let controls = hb_sim::flight::Controls {
            up: held(binds.up),
            down: held(binds.down),
            left: held(binds.left),
            right: held(binds.right),
            roll_left: held(binds.roll_left),
            roll_right: held(binds.roll_right),
            throttle_up: held(binds.throttle_up)
                || joystick.is_some_and(|s| s.button(stick::THROTTLE_UP)),
            throttle_down: held(binds.throttle_down)
                || joystick.is_some_and(|s| s.button(stick::THROTTLE_DOWN)),
            afterburner: fuel
                && (down(&[Key::LeftShift, Key::RightShift])
                    || joystick.is_some_and(|s| s.button(stick::AFTERBURNER))),
            stick: deflection,
            lever,
        };
        self.was = self.ship.position;
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

    /// Keep the ship out of the ground, the boxes and the tunnel walls.
    ///
    /// The engine tests the box that covers where the ship was and where it
    /// is going (`0x427280`), so nothing is crossed in one frame. This walks
    /// the step in pieces no longer than half the ship's unit and resolves
    /// each, which comes to the same thing for anything it can fly into.
    fn settle(&mut self, grid: &hb_world::Grid) {
        self.grounded = false;
        if !self.collide {
            return;
        }
        let target = self.ship.position;
        let travel: [f32; 3] = std::array::from_fn(|k| {
            if k == 1 {
                target[k] - self.was[k]
            } else {
                hb_sim::combat::wrapped(target[k] - self.was[k])
            }
        });
        let length = (travel[0] * travel[0] + travel[1] * travel[1] + travel[2] * travel[2]).sqrt();
        let pieces = (length / (hb_sim::collide::SHIP / 2.0)).ceil().max(1.0) as usize;
        let mut at = target;
        for piece in 1..=pieces {
            let t = piece as f32 / pieces as f32;
            let along: [f32; 3] = std::array::from_fn(|k| self.was[k] + travel[k] * t);
            let (resolved, held) = self.resolve(grid, along);
            at = resolved;
            if held {
                break;
            }
        }
        self.ship.position = at;
        self.sync_camera();
    }

    /// One position against the world: out of the boxes, above the ground, or
    /// inside the tunnel. Returns where it belongs and whether it was held
    /// back, which ends the sweep.
    ///
    /// The boxes are `hb_sim::collide`, which pushes the ship out of the face
    /// it is least far through, as the engine's response does (`0x4277c0`).
    /// The ground and the chamber are height queries, sampled across the
    /// ship's own unit rather than under its middle.
    fn resolve(&mut self, grid: &hb_world::Grid, position: [f32; 3]) -> ([f32; 3], bool) {
        let fixed = |v: f32| (v * 65536.0) as i32;
        let unit = hb_sim::collide::SHIP;
        let solids: Vec<hb_sim::collide::Solid> = grid
            .boxes_near(fixed(position[0]), fixed(position[2]), (unit * 65536.0) as i32)
            .into_iter()
            .map(hb_sim::collide::Solid::of)
            .collect();
        let (mut at, push) = hb_sim::collide::push_out(position, unit, &solids);
        let mut held = push.is_some();
        if push == Some(hb_sim::collide::Push::Up) {
            self.grounded = true;
        }
        let corners = [(0.0, 0.0), (-unit, -unit), (unit, -unit), (-unit, unit), (unit, unit)];
        let sample = |layer, at: [f32; 3], dx: f32, dz: f32| {
            grid.height_at(layer, fixed(at[0] + dx), fixed(at[2] + dz)).map(|h| h as f32 / 65536.0)
        };
        // Underground the chamber holds the ship, not the ground: two
        // heightfields with rock where they meet (`0x4290f0`).
        let cell = hb_world::Cell::containing(fixed(at[0]), fixed(at[2]));
        if at[1] < 0.0 && grid.has_chamber(cell) {
            let roof = corners
                .into_iter()
                .filter_map(|(dx, dz)| sample(hb_formats::terrain::Layer::ChamberCeiling, at, dx, dz))
                .fold(f32::MAX, f32::min);
            let bed = corners
                .into_iter()
                .filter_map(|(dx, dz)| sample(hb_formats::terrain::Layer::ChamberFloor, at, dx, dz))
                .fold(f32::MIN, f32::max);
            if roof - bed < unit * 2.0 {
                // Rock: it goes no further this frame.
                self.grounded = true;
                return (self.was, true);
            }
            if at[1] < bed + unit {
                at[1] = bed + unit;
                self.grounded = true;
                held = true;
            }
            if at[1] > roof - unit {
                at[1] = roof - unit;
                held = true;
            }
            return (at, held);
        }
        let ground = corners
            .into_iter()
            .filter_map(|(dx, dz)| sample(hb_formats::terrain::Layer::Ground, at, dx, dz))
            .fold(f32::MIN, f32::max);
        if at[1] < ground + unit {
            at[1] = ground + unit;
            self.grounded = true;
            held = true;
        }
        (at, held)
    }
}

/// Every level in `GAME.POD`, for the demo check - the campaign's own order
/// is [`hb_formats::campaign::CAMPAIGN`], which is what flying through them
/// follows.
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
    // Where the campaign starts (`0x482720`), not where the archive does.
    let mut level_name = hb_formats::campaign::FIRST.to_string();
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

    let mut index = hb_formats::campaign::index_of(&level_name).unwrap_or(0);
    let mut level = Level::load(&game, Some(&startup), &level_name)?;
    let describe = |level: &Level| {
        if let Some(at) = hb_formats::campaign::index_of(&level.stem) {
            let m = hb_formats::campaign::CAMPAIGN[at];
            println!("mission {}-{} of the campaign ({} of 23)", m.chapter, m.mission, at + 1);
        }
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
    // The game's own key bindings. Without an .INI they are the engine's
    // defaults, which is what a fresh install plays on.
    let (binds, from_ini) = keys::Bindings::load(&game_dir());
    println!(
        "keys: {}",
        if from_ini { "system/hellbend.ini" } else { "the engine's defaults" }
    );

    // A joystick, if the kernel has one to give. Without it nothing changes.
    let mut joystick = stick::Stick::open();
    let mut stick_woke = false;
    println!(
        "joystick: {}",
        match joystick {
            // Whatever the kernel numbered first may not be a joystick at
            // all, so nothing is read from it until something moves.
            Some(_) => "open, waiting for an axis to move",
            None => "none - keyboard only",
        }
    );

    // The briefing screen: the frame every chapter's briefing is drawn on,
    // in its own palette, and the small font the prose goes in.
    let briefing_screen: Option<(Image, act::Palette)> = startup
        .read("art", hb_formats::brief::BACKDROP)
        .ok()
        .and_then(|b| Image::parse_guessed(b).ok().flatten())
        .zip(startup.read("art", "brief.act").ok().and_then(|b| act::Palette::parse(b).ok()));
    // Which level's briefing is up, if one is, and how long it has been
    // typing. Cleared by any key, and set whenever a level that starts a
    // chapter is loaded.
    let mut briefing: Option<String>;
    let mut typing: f32;

    // The twelve weapon pictures, which the icon box shows one of.
    let icons: Vec<Option<Image>> = hb_render::hud::ICONS
        .iter()
        .map(|name| {
            startup
                .read("art", &format!("{name}.raw"))
                .ok()
                .and_then(|b| Image::parse_guessed(b).ok().flatten())
        })
        .collect();
    println!("weapon icons: {} of 12 loaded", icons.iter().filter(|i| i.is_some()).count());

    // The reticle, which is a model rather than art (`target.bin`), and the
    // palette index it pulses through, 32 to 63 and back a step a frame.
    let reticle = startup
        .read("models", "target.bin")
        .ok()
        .and_then(|b| hb_formats::mrgl::Model::parse(&b).ok());
    let mut show_reticle = reticle.is_some();
    let mut pulse: i32 = 32;
    let mut pulse_step: i32 = 1;
    println!(
        "cockpit: {}",
        if show_cockpit { "ckpt art loaded" } else { "not found" }
    );

    // The level's music, if a device will take it.
    let music = match sound::Music::open() {
        Ok(mut music) => {
            // The engine's two volumes, 16.16 in the `.INI` and 1.0 by
            // default. HB_VOLUME scales both on top, for a machine where
            // 1.0 is too much.
            let ini = hb_formats::ini::Ini::read(&game_dir().join("system").join("hellbend.ini"))
                .unwrap_or_default();
            let setting = |name: &str| {
                ini.int("Sound", name).map_or(1.0, |v| v as f32 / 65536.0).clamp(0.0, 1.0)
            };
            let extra: f32 = std::env::var("HB_VOLUME")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0f32)
                .clamp(0.0, 1.0);
            let (m, e) = (setting("musicVolume") * extra, setting("soundVolume") * extra);
            music.set_volumes(m, e);
            println!("audio: {} Hz, music {m:.2}, effects {e:.2}", music.rate);
            Some(music)
        }
        Err(why) => {
            println!("audio: {why}");
            None
        }
    };
    let play_module = |dir: &str, file: &str| {
        let Some(music) = music.as_ref() else {
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
    let play_music = |level: &Level| {
        if let Some((dir, file)) = level.manifest.slot("music") {
            play_module(dir, file);
        }
    };
    play_music(&level);

    // The HUD's own font, which lives in the executable rather than in an
    // archive: five pixels tall, which is what every line the game draws
    // over the cockpit is written in. The front end's `FONT.BIN` is the
    // 23-pixel one and is not this.
    let hud_font = std::fs::read(game_dir().join("HELLBEND.EXE"))
        .ok()
        .and_then(|exe| hb_formats::hud_font::HudFont::read(&exe).ok());
    if hud_font.is_none() {
        println!("hud: HELLBEND.EXE is not beside the archives, so no HUD font");
    }
    let mut show_hud = hud_font.is_some();
    // The engine's keyCockpitLabel, which names every element on the screen.
    let mut show_labels = false;

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

    let mut buffer = vec![0u32; w * h];
    // Everything that starts again with a level: the ship, the mission, the
    // placements that follow a course (`live` is the copy the renderer draws,
    // rewritten from them each frame), and the fight.
    let (mut flight, mut mission, mut followers, mut live, mut battle, mut colours) = begin(&level);
    let mut doors = doors_of(&level);
    let mut hoverers = hoverers_for(&level);
    // The actors that have left the level: class 17 ships, once they are
    // high enough. Not destroyed - simply no longer here.
    let mut gone = vec![false; live.len()];
    // The scatter an asteroid starts with, and anything else the loop needs
    // a number for.
    let mut rng = hb_sim::turret::Rng::new(0x5eed);
    // The missile warning and the afterburner's note, while they are held.
    let mut warning: Option<u64> = None;
    let (mut burner, mut burning) = (None, false);
    println!(
        "sim: {} of {} objects follow a course, {} doors, {} moving patches of ground",
        followers.len(),
        live.len(),
        doors.doors.len(),
        doors.patches.len()
    );

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
    briefing = has_briefing(&game, &level.stem).then(|| level.stem.clone());
    typing = 0.0;

    // A line the mission flashes on the HUD, and for how much longer.
    let mut flash: Option<(String, f32)> = None;
    // What is in the top-left panel and for how much longer. The engine
    // writes its lines there (`0x420080`) and hides the weapon readout while
    // one is up, which is the same strip of screen.
    let mut panel: Vec<String> = Vec::new();
    let mut saying = 0.0f32;
    // Seconds since the mission ended, before the next level (or this one
    // again) begins.
    let mut ended = 0.0f32;
    // The ship's last few seconds, while it is being one.
    let mut dying: Option<hb_sim::death::Wreck> = None;
    // The ship's velocity, from how far the eye moved last frame; turrets
    // lead with it and the laser adds its magnitude.
    let mut last_eye = eye_of(&flight.camera);
    println!("sim: {} turrets, {} flyers", battle.turret_count(), battle.flyer_count());

    let started = Instant::now();
    let mut last = Instant::now();
    let mut tab_was_down = false;
    // The original's own floor was 8 frames a second - `autoMinFrameRate` in
    // HELLBEND.INI - so it is worth knowing where this sits.
    let (mut frames, mut since) = (0u32, Instant::now());

    let quit = binds.end_game.unwrap_or(Key::Escape);
    while window.is_open() && !window.is_key_down(quit) {
        let now = Instant::now();
        let mut dt = (now - last).as_secs_f32().min(0.1);
        last = now;

        // A briefing holds everything until a key is pressed, the way a
        // briefing screen does. Nothing moves behind it.
        if briefing.is_some() {
            typing += dt;
            dt = 0.0;
            if window.get_keys_pressed(minifb::KeyRepeat::No).iter().any(|k| *k != quit) {
                briefing = None;
                last = Instant::now();
            }
        }

        // The doors, before anything borrows the terrain to read it: a moved
        // box is written straight back into the grid, so the renderer and the
        // collision both see it where it now is.
        for moved in doors.step(dt) {
            move_box(&mut level.terrain, &moved);
        }
        // A switch that has just gone on or off wears a different texture on
        // all four of its sides.
        for swap in std::mem::take(&mut doors.swaps) {
            let Some(name) = swap.texture else { continue };
            let wanted = name.to_ascii_lowercase();
            let Some(index) =
                level.texture_names.iter().position(|n| n.to_ascii_lowercase() == wanted)
            else {
                continue;
            };
            let layer = match swap.layer {
                hb_sim::quake::Layer::BoxA => &mut level.terrain.boxes_a,
                _ => &mut level.terrain.boxes_b,
            };
            let at = (swap.cell.1 as usize & 127) * 128 + (swap.cell.0 as usize & 127);
            for face in 0..4 {
                layer.textures.values[at * layer.textures.per_cell + face] = index as u16;
            }
        }
        for played in std::mem::take(&mut doors.sounds) {
            if let (Some(music), Some(s)) = (music.as_ref(), sound(&played.name.to_ascii_lowercase())) {
                // A door at the far end of the level is a door you can hardly
                // hear.
                let at = [
                    (played.cell.0 * 8) as f32 - 512.0,
                    0.0,
                    (played.cell.1 * 8) as f32 - 512.0,
                ];
                music.effect(&s, 0.6 * hb_sim::combat::falloff(eye_of(&flight.camera), at));
            }
        }

        if let Some(joystick) = joystick.as_mut() {
            joystick.poll();
            // Say so the once, so it is clear which device answered.
            if joystick.woken() && !stick_woke {
                stick_woke = true;
                println!("joystick: an axis moved - flying with it");
            }
        }
        let tab = window.is_key_down(binds.next_level);
        // The port's own: after a mission ends, three seconds with the result
        // on the HUD, then the next level if it was won and this one again
        // if not. What the engine does next - the debriefing, the story - is
        // not read.
        let next = match mission.outcome {
            Some(outcome) if demo.is_none() => {
                if ended == 0.0 {
                    println!("mission {}", match outcome {
                        hb_sim::mission::Outcome::Jumped => "over: jumped out",
                        hb_sim::mission::Outcome::Complete => "complete",
                        hb_sim::mission::Outcome::Failed => "failed",
                    });
                }
                ended += dt;
                (ended > 3.0).then_some(outcome != hb_sim::mission::Outcome::Failed)
            }
            _ => None,
        };
        if (tab && !tab_was_down) || next.is_some() {
            if next != Some(false) {
                index = (index + 1) % hb_formats::campaign::CAMPAIGN.len();
            }
            level = Level::load(&game, Some(&startup), hb_formats::campaign::CAMPAIGN[index].stem)?;
            briefing = has_briefing(&game, &level.stem).then(|| level.stem.clone());
            typing = 0.0;
            describe(&level);
            play_music(&level);
            (flight, mission, followers, live, battle, colours) = begin(&level);
            doors = doors_of(&level);
            hoverers = hoverers_for(&level);
            gone = vec![false; live.len()];
            last_eye = eye_of(&flight.camera);
            ended = 0.0;
            dying = None;
            flash = None;
            println!(
                "sim: {} of {} objects follow a course, {} turrets, {} flyers",
                followers.len(),
                live.len(),
                battle.turret_count(),
                battle.flyer_count()
            );
        }
        tab_was_down = tab;

        let pressed = |key: Option<Key>| {
            key.is_some_and(|k| window.is_key_pressed(k, minifb::KeyRepeat::No))
        };
        if window.is_key_pressed(binds.hud, minifb::KeyRepeat::No) {
            show_hud = !show_hud && hud_font.is_some();
        }
        if window.is_key_pressed(binds.music, minifb::KeyRepeat::No) {
            match music.as_ref() {
                Some(m) if m.playing() => m.stop(),
                Some(_) => play_music(&level),
                None => {}
            }
        }
        if window.is_key_pressed(binds.cockpit, minifb::KeyRepeat::No) {
            show_cockpit = !show_cockpit && cockpit.is_some();
        }
        if pressed(binds.cockpit_label) {
            show_labels = !show_labels;
        }
        if pressed(binds.crosshair) {
            show_reticle = !show_reticle && reticle.is_some();
        }
        if pressed(binds.beacon) && demo.is_none() {
            for event in mission.drop_beacon(eye_of(&flight.camera)) {
                if let hb_sim::mission::Event::Voice(v) = event {
                    say(&mut panel, &mut saying, v.text);
                    if let (Some(music), Some(s)) = (music.as_ref(), sound(v.sound)) {
                        music.effect(&s, 1.0);
                    }
                } else if let hb_sim::mission::Event::Message(m) = event {
                    flash = Some((m.to_string(), 3.0));
                }
            }
        }
        if window.is_key_pressed(binds.collide, minifb::KeyRepeat::No) {
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
                flight.step(&window, &binds, joystick.as_ref(), dt, battle.stores.fuel > 0.0);
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

        // The guns, the afterburner's fuel, and the energy that feeds both:
        // `hb_sim::weapons`, from the trigger at `0x47db11` on.
        if demo.is_none() && battle.pilot.alive() {
            let v = flight.ship.world_velocity();
            let pose = hb_sim::weapons::Pose {
                position: eye,
                right: flight.ship.right,
                up: flight.ship.up,
                forward: flight.ship.forward,
                pitch: flight.camera.pitch.0 as i16 as f32,
                heading: flight.camera.yaw.0 as f32,
                speed: (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(),
            };
            // The missile lock (`keyMissileLock`, V): what the selected weapon
            // can lock on, as `0x47bbd0` judges it.
            let selected = battle.guns.selected;
            let candidates: Vec<hb_sim::weapons::Candidate> = live
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let at = hb_sim::combat::position_of(p);
                    let fixed = |v: f32| (v * 65536.0) as i32;
                    let near = [
                        eye[0] + hb_sim::combat::wrapped(at[0] - eye[0]),
                        at[1],
                        eye[2] + hb_sim::combat::wrapped(at[2] - eye[2]),
                    ];
                    let kind = &level.kinds[level.placements[i].kind];
                    hb_sim::weapons::Candidate {
                        class: kind.class(),
                        friendly: kind.friendly,
                        alive: !battle.health[i].destroyed && battle.health[i].hit_points > 0.0,
                        near: hb_sim::combat::in_range(eye, at),
                        view: flight.camera.to_view(fixed(near[0]), fixed(near[1]), fixed(near[2])),
                    }
                })
                .collect();
            battle.guns.track(pressed(binds.missile_lock), candidates.len(), |i| {
                candidates[i].lockable(selected)
            });
            let burn = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
            let firing = binds.fire.is_some_and(|k| window.is_key_down(k))
                || joystick.as_ref().is_some_and(|s| s.button(stick::FIRE));
            let (volleys, mut voices) = battle.trigger(firing, burn, dt, &pose);
            for (key, weapon) in binds.weapons.iter().zip(keys::WEAPON_ROWS) {
                if pressed(*key) {
                    voices.extend(battle.guns.select(weapon, &battle.stores));
                }
            }
            if pressed(binds.next_weapon)
                || joystick.as_ref().is_some_and(|s| s.pressed(stick::WEAPON))
            {
                battle.guns.next(&battle.stores);
            }
            if pressed(binds.previous_weapon) {
                battle.guns.previous(&battle.stores);
            }
            if pressed(binds.transfer_weapon) {
                voices.extend(hb_sim::weapons::transfer_to_weapons(&mut battle.stores));
            }
            if pressed(binds.transfer_shields) {
                voices.extend(hb_sim::weapons::transfer_to_shields(&mut battle.stores, &mut battle.pilot.shield));
            }
            voices.extend(battle.tick(dt));
            for volley in volleys {
                if let (Some(music), Some(s)) = (music.as_ref(), sound(volley.sound)) {
                    music.effect(&s, 0.5);
                }
            }
            for v in voices {
                say(&mut panel, &mut saying, v.text);
                if let (Some(music), Some(s)) = (music.as_ref(), sound(v.sound)) {
                    music.effect(&s, 1.0);
                }
            }
        }

        let grid = hb_world::Grid::new(&level.terrain);
        // What stops a shot. Above ground that is the top of whatever is
        // solid; inside a tunnel it is the chamber's own floor and ceiling,
        // because the ground overhead is not what a shot fired down there
        // meets first.
        let solid = |p: [f32; 3]| {
            let (x, z) = ((p[0] * 65536.0) as i32, (p[2] * 65536.0) as i32);
            let y = (p[1] * 65536.0) as i32;
            let cell = hb_world::Cell::containing(x, z);
            if p[1] < 0.0 && grid.has_chamber(cell) {
                let layer = hb_formats::terrain::Layer::ChamberFloor;
                let floor = grid.height_at(layer, x, z).unwrap_or(i32::MIN);
                let layer = hb_formats::terrain::Layer::ChamberCeiling;
                let roof = grid.height_at(layer, x, z).unwrap_or(i32::MAX);
                return y <= floor || y >= roof;
            }
            y <= grid.ceiling_of_solid(x, z)
        };
        // In a demo the recorded flight cannot dodge, so nothing shoots back.
        let ground = |x: f32, z: f32| grid.ceiling_of_solid((x * 65536.0) as i32, (z * 65536.0) as i32) as f32 / 65536.0;
        let axes = [flight.ship.right, flight.ship.up, flight.ship.forward];
        let noises = if demo.is_none() {
            battle.step(&level, &mut live, eye, velocity, Some((&axes, flight.ship.speed())), dt, &solid, &ground)
        } else {
            battle.step(&level, &mut live, [0.0, 1.0e6, 0.0], [0.0; 3], None, dt, &|_| false, &ground)
        };
        // The afterburner: a kick and then a held engine note for as long as
        // it burns (`0x47d6a3` plays `blast7.wav`, then `engine4.wav` with
        // the voice's loop flag set).
        if battle.burning != burning {
            burning = battle.burning;
            match (burning, burner.take()) {
                (true, _) => {
                    if let Some(music) = music.as_ref() {
                        if let Some(s) = sound("blast7.wav") {
                            music.effect(&s, 0.7);
                        }
                        burner = sound("engine4.wav").and_then(|s| music.hold(&s, 0.6));
                    }
                }
                (false, Some(handle)) => {
                    if let Some(music) = music.as_ref() {
                        music.let_go(handle);
                    }
                }
                (false, None) => {}
            }
        }

        // The missile warning: the engine walks the missile pool for one
        // that names the local player and holds `m-lock7.wav` while there is
        // one, stopping it when there is not (`0x44ea21`, `0x47f6d0`).
        let chased = battle
            .missiles
            .iter()
            .any(|m| m.side == hb_sim::combat::Side::Enemy && m.alive())
            && battle.pilot.alive();
        match (chased, warning) {
            (true, None) => {
                if let (Some(music), Some(s)) = (music.as_ref(), sound("m-lock7.wav")) {
                    warning = music.hold(&s, 0.5);
                }
            }
            (false, Some(handle)) => {
                if let Some(music) = music.as_ref() {
                    music.let_go(handle);
                }
                warning = None;
            }
            _ => {}
        }

        // A shot that hit the world opens any door it landed in
        // (`0x410b80`): the cell it hit, and its altitude in the terrain's
        // own words.
        for hit in std::mem::take(&mut battle.ground_hits) {
            let cell = hb_world::grid::Cell::containing(
                (hit[0] * 65536.0) as i32,
                (hit[2] * 65536.0) as i32,
            );
            doors.shot((cell.x, cell.z), hit[1] * hb_sim::quake::WORD);
        }

        for noise in noises {
            let (name, volume) = match noise {
                battle::Noise::Destroyed(kind, at) => {
                    if level.kinds[kind].friendly {
                        mission.friendly_lost();
                        if level.kinds[kind].class() == 50 {
                            mission.escort_lost();
                        }
                    }
                    let own = level.destroy_sound[kind]
                        .as_ref()
                        .and_then(|b| hb_audio::Wav::parse(b).ok())
                        .map(std::sync::Arc::new);
                    if let (Some(music), Some(s)) = (music.as_ref(), own.or_else(|| sound("blast4.wav"))) {
                        music.effect(&s, 0.8 * hb_sim::combat::falloff(eye, at));
                    }
                    // And the words that go with it, which the engine finds
                    // by the sound's own file name (`0x4548a9`).
                    if let Some(said) = level.destroy_sound_name[kind]
                        .as_deref()
                        .and_then(hb_sim::phrases::by_sound)
                    {
                        say(&mut panel, &mut saying, said.text);
                    }
                    continue;
                }
                battle::Noise::PlayerHit(n) => (format!("exp{}.wav", n + 1), 0.8),
                battle::Noise::NearMiss(kind, at) => (
                    hb_sim::combat::near_miss_sound(kind).to_string(),
                    0.6 * hb_sim::combat::falloff(eye, at),
                ),
                battle::Noise::Died => {
                    println!("shot down ({} so far) - back to the start", battle.deaths);
                    ("blast7.wav".to_string(), 1.0)
                }
            };
            if let (Some(music), Some(s)) = (music.as_ref(), sound(&name)) {
                music.effect(&s, volume);
            }
        }
        // Dying: the ship tumbles on, blows up near the ground, and the
        // level is over five seconds later (`hb_sim::death`).
        if !battle.pilot.alive() && demo.is_none() {
            let [pitch, roll, heading] = flight.ship.angles();
            let wreck = dying.get_or_insert_with(|| hb_sim::death::Wreck::new(pitch, roll, heading));
            let grid = hb_world::Grid::new(&level.terrain);
            let under = |p: [f32; 3]| {
                grid.ceiling_of_solid((p[0] * 65536.0) as i32, (p[2] * 65536.0) as i32) as f32 / 65536.0
            };
            let mut at = flight.ship.position;
            let ground = under(at);
            let wants = wreck.step(&mut at, dt, ground);
            flight.ship.position = at;
            flight.camera.x = (at[0] * 65536.0) as i32;
            flight.camera.y = (at[1] * 65536.0) as i32;
            flight.camera.z = (at[2] * 65536.0) as i32;
            flight.camera.pitch = Angle(wreck.pitch as i32 as u16);
            flight.camera.roll = Angle(wreck.roll as i32 as u16);
            last_eye = eye_of(&flight.camera);
            match wants {
                Some(hb_sim::death::Wants::Explode) => {
                    battle.blow_up(at);
                    if let (Some(music), Some(s)) = (music.as_ref(), sound("blast7.wav")) {
                        music.effect(&s, 1.0);
                    }
                    flash = Some(("SHOT DOWN".to_string(), 5.0));
                }
                // The engine ends the level here and the mission is failed;
                // this starts it again, which is the port's own choice.
                Some(hb_sim::death::Wants::Over) => {
                    println!("shot down ({} so far) - starting again", battle.deaths);
                    (flight, mission, followers, live, battle, colours) = begin(&level);
                    doors = doors_of(&level);
                    hoverers = hoverers_for(&level);
                    gone = vec![false; live.len()];
                    last_eye = eye_of(&flight.camera);
                    dying = None;
                    ended = 0.0;
                }
                None => {}
            }
        }

        // The mission.
        if demo.is_none() {
            let mut standing = battle::Standing {
                hull: battle.pilot.health,
                health: &mut battle.health,
                live: &live,
                placed: &level.placements,
            };
            let events = mission.step(&mut standing, eye, flight.camera.yaw.0, dt, floor_of(&level));
            for event in events {
                use hb_sim::mission::Event;
                let heard = match event {
                    Event::Sound(name) => {
                        // A sound the mission file names carries its words
                        // in the engine's phrase table, found by that name.
                        if let Some(said) = hb_sim::phrases::by_sound(&name) {
                            say(&mut panel, &mut saying, said.text);
                        }
                        Some(name)
                    }
                    Event::Voice(v) => {
                        say(&mut panel, &mut saying, v.text);
                        Some(v.sound.to_string())
                    }
                    Event::Message(m) => {
                        flash = Some((m.to_string(), 3.0));
                        None
                    }
                    Event::Music(name) => {
                        play_module("music", &name);
                        None
                    }
                    Event::MusicBack => {
                        play_music(&level);
                        None
                    }
                    Event::Warp(at) => {
                        flight.ship.position = at;
                        flight.sync_camera();
                        last_eye = eye_of(&flight.camera);
                        None
                    }
                };
                if let (Some(music), Some(s)) = (music.as_ref(), heard.and_then(|n| sound(&n))) {
                    music.effect(&s, 1.0);
                }
            }
        }
        // Powerups.
        if demo.is_none() {
            let (said, taken) = battle.pick_up(eye);
            for i in taken {
                let kind = battle.field.items[i].kind;
                // Every powerup case that takes one plays the same sound
                // beside its own message (`0x40e42f` and six more).
                if let (Some(music), Some(s)) = (music.as_ref(), sound("power-1.wav")) {
                    music.effect(&s, 0.8);
                }
                // The eighth Bion piece makes the super weapon and selects
                // it (`0x426c39`).
                if (23..=30).contains(&kind) && battle.stores.weapon == hb_sim::weapons::SUPER {
                    battle.guns.select(hb_sim::weapons::SUPER, &battle.stores);
                }
                println!("picked up {}", hb_sim::powerup::KINDS[kind].0);
                if kind == hb_sim::powerup::MESSAGE_POD {
                    mission.pod_taken(i);
                }
            }
            for event in said {
                use hb_sim::powerup::Event;
                let heard = match event {
                    Event::Voice(v) => {
                        say(&mut panel, &mut saying, v.text);
                        Some(v.sound)
                    }
                    Event::Message(m) => {
                        flash = Some((m.to_string(), 3.0));
                        None
                    }
                    Event::Sound(name) => Some(name),
                };
                if let (Some(music), Some(s)) = (music.as_ref(), heard.and_then(|n| sound(n))) {
                    music.effect(&s, 1.0);
                }
            }
        }

        if saying > 0.0 {
            saying -= dt;
            if saying <= 0.0 {
                panel.clear();
            }
        }
        if let Some((_, left)) = &mut flash {
            *left -= dt;
            if *left <= 0.0 {
                flash = None;
            }
        }

        for (i, motion, radius) in &mut hoverers {
            if battle.health[*i].destroyed
                || gone[*i]
                || !hb_sim::combat::in_range(eye, hb_sim::combat::position_of(&live[*i]))
            {
                continue;
            }
            let here = hb_sim::combat::position_of(&live[*i]);
            let around = hb_sim::behaviour::Around {
                radius: *radius,
                sky: level.sky_height(),
                ground: floor_of(&level)(here),
            };
            let moved = motion.step(dt, around, &mut rng);
            let placed = &mut live[*i];
            [placed.x, placed.y, placed.z] = moved.at.map(|v| (v * 65536.0) as i32);
            placed.pitch = moved.angles[0] as i32;
            placed.roll = moved.angles[1] as i32;
            placed.heading = moved.angles[2] as u16;
            if let Some(at) = moved.blast {
                battle.burst(at, *radius);
            }
            if moved.gone {
                // The engine clears the actor's `+0x1c`, which takes it out
                // of the loop that thinks and the one that draws. It is not
                // a kill, so nothing here counts it as one.
                gone[*i] = true;
            }
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
        // The animated models, before the scene borrows the level.
        level.animate(started.elapsed().as_secs_f32());
        let frames_now = level.texture_frames(started.elapsed().as_secs_f32());
        let mut scene = level.scene();
        scene.frames = Some(&frames_now);
        scene.sky_scroll = level.sky_scroll(started.elapsed().as_secs_f32());
        scene.seconds = started.elapsed().as_secs_f32();
        // The powerups are drawn as more placements, turned to face the eye.
        // The engine turns them by two of the view's angles (`0x4266c0`
        // passes `0x5b37c8` and `0x5b37c4` to `0x42aa30`); which two is not
        // settled, so this turns them by the heading alone.
        let mut drawn: Vec<hb_formats::text::Placement> = live
            .iter()
            .enumerate()
            .filter(|(i, _)| !gone.get(*i).copied().unwrap_or(false))
            .map(|(_, p)| p.clone())
            .collect();
        for item in battle.field.items.iter().filter(|p| !p.taken) {
            if let Some(Some(mesh)) = level.powerup_mesh.get(item.kind) {
                let fixed = |v: f32| (v * 65536.0) as i32;
                drawn.push(hb_formats::text::Placement {
                    kind: *mesh,
                    hit_points: 0,
                    x: fixed(item.position[0]),
                    y: fixed(item.position[1]),
                    z: fixed(item.position[2]),
                    pitch: 0,
                    roll: 0,
                    heading: flight.camera.yaw.0,
                });
            }
        }
        // Shots and missiles are models too (`0x4769cf`): along their own
        // flight, except the kinds the engine holds facing the eye, and the
        // Valkyrie's three muzzle flashes in turn. A kind whose model did
        // not load stays a spark.
        let near = |p: [f32; 3]| {
            [
                eye[0] + hb_sim::combat::wrapped(p[0] - eye[0]),
                p[1],
                eye[2] + hb_sim::combat::wrapped(p[2] - eye[2]),
            ]
        };
        let fixed = |v: f32| (v * 65536.0) as i32;
        let shot_at = |drawn: &mut Vec<hb_formats::text::Placement>, mesh, at: [f32; 3], angles: (u16, u16)| {
            drawn.push(hb_formats::text::Placement {
                kind: mesh,
                hit_points: 0,
                x: fixed(at[0]),
                y: fixed(at[1]),
                z: fixed(at[2]),
                pitch: angles.1 as i32,
                roll: 0,
                heading: angles.0,
            });
        };
        // Which way a velocity points, in the engine's circle.
        let along = |v: [f32; 3]| {
            let turn = 65536.0 / std::f32::consts::TAU;
            let flat = (v[0] * v[0] + v[2] * v[2]).sqrt();
            ((v[0].atan2(v[2]) * turn) as i32 as u16, (-v[1].atan2(flat) * turn) as i32 as u16)
        };
        let mut sparks: Vec<([f32; 3], u8)> = Vec::new();
        let mut muzzle_frame = 0usize;
        for flying in &battle.shots {
            let s = &flying.shot;
            let at = near(s.position);
            let mesh = if s.kind == hb_sim::weapons::VALKYRIE as i32 {
                muzzle_frame += 1;
                level.muzzle_mesh[(muzzle_frame - 1) % 3]
            } else {
                level.shot_mesh.get(s.kind as usize).copied().flatten()
            };
            match mesh {
                Some(mesh) => {
                    let angles = if hb_sim::weapons::FACES_THE_EYE.contains(&s.kind) {
                        (flight.camera.yaw.0, flight.camera.pitch.0)
                    } else {
                        along(s.velocity)
                    };
                    shot_at(&mut drawn, mesh, at, angles);
                }
                None => sparks.push((
                    at,
                    match s.side {
                        hb_sim::combat::Side::Player => colours.player,
                        hb_sim::combat::Side::Enemy => colours.enemy,
                    },
                )),
            }
        }
        for missile in &battle.missiles {
            let at = near(missile.position);
            match level.shot_mesh.get(missile.kind as usize).copied().flatten() {
                Some(mesh) => shot_at(
                    &mut drawn,
                    mesh,
                    at,
                    (missile.heading as i32 as u16, missile.pitch as i32 as u16),
                ),
                None => sparks.push((at, colours.missile)),
            }
        }
        // The mines lying about, each on its own two angles.
        for mine in battle.mines.live() {
            let at = near(mine.at);
            match level.shot_mesh.get(hb_sim::weapons::MINE).copied().flatten() {
                Some(mesh) => {
                    shot_at(&mut drawn, mesh, at, (mine.angles[1] as u16, mine.angles[0] as u16))
                }
                None => sparks.push((at, colours.missile)),
            }
        }
        // And the enemy's, which have no angles of their own - they are
        // dropped and they sit there.
        for mine in battle.laid.slots.iter().flatten() {
            let at = near(mine.at);
            match level.shot_mesh.get(hb_sim::weapons::MINE).copied().flatten() {
                Some(mesh) => shot_at(&mut drawn, mesh, at, (0, 0)),
                None => sparks.push((at, colours.missile)),
            }
        }
        scene.placements = &drawn;
        hb_render::draw_world(&mut target, &scene, &flight.camera);
        for (at, colour) in sparks {
            hb_render::scene::draw_spark(&mut target, &flight.camera, at, colour);
        }
        // The explosions, each a square facing the eye on its own frame.
        for puff in &battle.blasts.puffs {
            let Some(frame) = puff.frame() else { continue };
            if let Some(Some(texture)) = level.blast.get(frame) {
                hb_render::scene::draw_sprite(
                    &mut target,
                    &scene,
                    &flight.camera,
                    near(puff.position),
                    puff.size,
                    texture,
                );
            }
        }
        // Brackets on the locked target. The engine's lock display is not
        // read; this only shows what is locked.
        if let Some(i) = battle.guns.lock {
            if let Some(p) = live.get(i) {
                let at = hb_sim::combat::position_of(p);
                let fixed = |v: f32| (v * 65536.0) as i32;
                let v = flight.camera.to_view(
                    fixed(eye[0] + hb_sim::combat::wrapped(at[0] - eye[0])),
                    fixed(at[1]),
                    fixed(eye[2] + hb_sim::combat::wrapped(at[2] - eye[2])),
                );
                if v[2] > 0.5 {
                    let ([kx, ky], [cx, cy]) = Camera::screen(w, h);
                    let (sx, sy) = (cx + v[0] / v[2] * kx, cy - v[1] / v[2] * ky);
                    draw_brackets(&mut target.colour, w, h, sx, sy, colours.enemy);
                }
            }
        }
        if show_cockpit {
            if let Some(art) = &cockpit {
                target.overlay(art);
            }
        }
        if show_reticle {
            if let Some(model) = &reticle {
                pulse += pulse_step;
                if pulse >= 63 || pulse <= 32 {
                    pulse_step = -pulse_step;
                }
                hb_render::hud::reticle(&mut target, model, pulse as u8);
            }
        }
        if show_hud {
            if let Some(font) = &hud_font {
                // The radar, which the engine fills from every object that is
                // alive and not hidden, turned so the nose points up
                // (`0x437148`). Blips beyond the box are dropped by the draw.
                let heading = flight.camera.yaw.0 as f32 * std::f32::consts::TAU / 65536.0;
                let (sin, cos) = heading.sin_cos();
                let blips: Vec<hb_render::hud::Blip> = live
                    .iter()
                    .enumerate()
                    .filter(|&(i, _)| {
                        let class = level.kinds[level.placements[i].kind].class();
                        !gone[i]
                            && !battle.health[i].destroyed
                            && battle.health[i].hit_points > 0.0
                            && class != 18
                            && class != 33
                    })
                    .map(|(i, p)| {
                        let at = hb_sim::combat::position_of(p);
                        let dx = hb_sim::combat::wrapped(at[0] - eye[0]);
                        let dz = hb_sim::combat::wrapped(at[2] - eye[2]);
                        let class = level.kinds[level.placements[i].kind].class();
                        hb_render::hud::Blip {
                            right: dx * cos - dz * sin,
                            forward: dx * sin + dz * cos,
                            colour: if hb_render::hud::is_target(class) {
                                hb_render::hud::BLIP_TARGET
                            } else {
                                hb_render::hud::BLIP_OTHER
                            },
                            below: at[1] < eye[1],
                        }
                    })
                    .collect();
                let weapon = battle.guns.selected;
                // The objective box takes the three-letter code, not the
                // long line: the box is 80 pixels in 640, and "Destroy
                // Target" is twice that.
                let objective = (demo.is_none() && !mission.code.is_empty())
                    .then_some(mission.code);
                let readout = hb_render::hud::Readout {
                    weapon: hb_sim::weapons::ROWS[weapon].code,
                    ammo: match battle.stores.ammo[weapon] {
                        -1 => None,
                        n => Some(n as i64),
                    },
                    objective,
                    distance: (demo.is_none()).then_some(mission.distance as i32),
                    gauges: [
                        flight.ship.throttle,
                        battle.pilot.health,
                        battle.stores.fuel,
                        battle.stores.weapon_energy,
                        battle.stores.energy,
                        battle.pilot.shield,
                    ],
                    countdown: None,
                    labels: show_labels,
                    blips: &blips,
                };
                hb_render::hud::draw(&mut target, font, &readout);
                // The panel, and while it is up the weapon lines and the
                // picture stand aside - they are the same strip of screen.
                if saying > 0.0 && !panel.is_empty() {
                    hb_render::hud::panel(&mut target, font, &panel);
                }
                // The weapon's picture, which the message panel covers.
                if flash.is_none() && !(saying > 0.0 && !panel.is_empty()) {
                    if let Some(Some(art)) =
                        hb_render::hud::WEAPON_ICONS.get(weapon).map(|&i| &icons[i])
                    {
                        hb_render::hud::icon(&mut target, art);
                    }
                }
                if let Some((text, _)) = &flash {
                    hb_render::hud::message(&mut target, font, text);
                }
                if mission.outcome.is_none() && demo.is_none() {
                    // On the radar, which is where the engine puts it.
                    hb_render::hud::arrow(&mut target, mission.arrow);
                }
            }
        }

        // The framebuffer is palette indices; minifb wants 0x00RRGGBB. A
        // briefing is already in that form and stands in front of it.
        let typed = briefing.as_ref().and_then(|stem| {
            briefing_for(
                &game,
                briefing_screen.as_ref(),
                hud_font.as_ref(),
                stem,
                w,
                h,
                (typing * hb_formats::brief::TYPED_A_SECOND) as usize,
            )
        });
        match &typed {
            Some(screen) => buffer.copy_from_slice(screen),
            None => {
                for (slot, &index) in buffer.iter_mut().zip(&target.colour) {
                    let [r, g, b] = level.palette.rgb(index);
                    *slot = OPAQUE | ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
                }
            }
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
            let kind = level.kinds.get(p.kind)?;
            // An actor runs one routine, and the flyers' and the hovering
            // class's do not read the course.
            if battle::FLYING.contains(&kind.class()) || kind.class() == 26 {
                return None;
            }
            let c = kind.course;
            let course = level.courses.get(usize::try_from(c).ok()?)?;
            Some((i, hb_sim::Follower::new(course, [p.x, p.y, p.z])?))
        })
        .collect()
}

/// The classes that only move - 26 hovers, 17 leaves, 18 falls - with each
/// one's radius, which is how far a hovering one bobs.
fn hoverers_for(level: &Level) -> Vec<(usize, hb_sim::behaviour::Motion, f32)> {
    level
        .placements
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let kind = level.kinds.get(p.kind)?;
            let at = [p.x, p.y, p.z].map(|v| v as f32 / 65536.0);
            let angles = [p.pitch as f32, p.roll as f32, p.heading as f32];
            let motion = hb_sim::behaviour::Motion::of(kind.class(), at, angles)?;
            Some((i, motion, kind.radius() as f32 / 65536.0))
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

/// A level's opening state: the ship at the mission's start, the mission,
/// the course followers, the placements as drawn, the fight, and the shot
/// colours for its palette.
fn begin(
    level: &Level,
) -> (Flight, hb_sim::mission::Mission, Vec<(usize, hb_sim::Follower)>, Vec<hb_formats::text::Placement>, battle::Battle, ShotColours) {
    let mut rng = hb_sim::turret::Rng::new(0x1996);
    let mut mission =
        hb_sim::mission::Mission::new(level.navs.clone(), &level.placements, floor_of(level), &mut rng);
    if !mission.is_empty() {
        println!("mission: {} points - {}", mission.len(), mission.objective());
    }
    // The `.PUP`'s powerups go down first, so their indices are the ones the
    // mission's message pods name.
    let mut battle = battle::Battle::new(level);
    let floor = floor_of(level);
    for p in &level.powerups {
        let at = p.position.map(|v| v as f32 / 65536.0);
        let size = level.powerup_size.get(p.kind).copied().unwrap_or(0.0);
        battle.field.place(at, p.kind, size, floor(at));
    }
    let pods: Vec<[f32; 3]> = battle.field.items.iter().map(|p| p.position).collect();
    mission.place_pods(&pods);
    (
        Flight::new(start_camera(level, &mission)),
        mission,
        followers_for(level),
        level.placements.clone(),
        battle,
        ShotColours::for_palette(&level.palette),
    )
}

/// The level's doors and moving ground, with every cell's altitude as the
/// terrain has it.
fn doors_of(level: &Level) -> hb_sim::quake::Quakes {
    use hb_sim::quake::Layer;
    hb_sim::quake::Quakes::new(&level.quake, |layer, (x, z)| match layer {
        Layer::BoxA => (level.terrain.boxes_a.bottom.at(x, z), level.terrain.boxes_a.top.at(x, z)),
        Layer::BoxB => (level.terrain.boxes_b.bottom.at(x, z), level.terrain.boxes_b.top.at(x, z)),
        Layer::Ground => {
            let h = level.terrain.ground.at(x, z);
            (h, h)
        }
        Layer::ChamberFloor => {
            let h = level.terrain.chambers.floor.at(x, z);
            (h, h)
        }
        Layer::ChamberCeiling => {
            let h = level.terrain.chambers.ceiling.at(x, z);
            (h, h)
        }
    })
}

/// Write a moved cell back into the terrain, which is where the renderer and
/// the collision both read it from.
fn move_box(terrain: &mut hb_formats::terrain::Terrain, m: &hb_sim::quake::Moved) {
    use hb_sim::quake::Layer;
    let at = (m.cell.1 as usize & 127) * 128 + (m.cell.0 as usize & 127);
    match m.layer {
        Layer::BoxA => {
            terrain.boxes_a.bottom.values[at] = m.bottom;
            terrain.boxes_a.top.values[at] = m.top;
        }
        Layer::BoxB => {
            terrain.boxes_b.bottom.values[at] = m.bottom;
            terrain.boxes_b.top.values[at] = m.top;
        }
        Layer::Ground => terrain.ground.values[at] = m.top,
        Layer::ChamberFloor => terrain.chambers.floor.values[at] = m.top,
        Layer::ChamberCeiling => terrain.chambers.ceiling.values[at] = m.top,
    }
}



/// The surface under a point, for the mission (`0x41c300` finds the floor
/// under a point, tunnels included). This has only the top of the solid, so a
/// point below zero - the engine's own test for underground - is left where
/// it is.
fn floor_of(level: &Level) -> impl Fn([f32; 3]) -> f32 + '_ {
    move |p: [f32; 3]| {
        if p[1] < 0.0 {
            return p[1];
        }
        let grid = hb_world::Grid::new(&level.terrain);
        grid.ceiling_of_solid((p[0] * 65536.0) as i32, (p[2] * 65536.0) as i32) as f32 / 65536.0
    }
}

/// Where the mission starts the player, or the middle of the map when it has
/// no start.
fn start_camera(level: &Level, mission: &hb_sim::mission::Mission) -> Camera {
    match mission.start {
        Some(start) => {
            let fixed = |v: f32| (v * 65536.0) as i32;
            let [x, y, z] = start.position.map(fixed);
            let mut camera = Camera::looking_at(x, y, z, Angle(start.angles[2]));
            camera.pitch = Angle(start.angles[0]);
            camera
        }
        None => start_of(level),
    }
}

/// Four corner brackets around a point on the screen.
fn draw_brackets(pixels: &mut [u8], w: usize, h: usize, x: f32, y: f32, colour: u8) {
    let (x, y) = (x.round() as isize, y.round() as isize);
    let mut plot = |px: isize, py: isize| {
        if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
            pixels[py as usize * w + px as usize] = colour;
        }
    };
    let (r, arm) = (8isize, 3isize);
    for (sx, sy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
        for k in 0..=arm {
            plot(x + sx * r - sx * k, y + sy * r);
            plot(x + sx * r, y + sy * r - sy * k);
        }
    }
}


/// Put a line in the top-left panel, keeping the last few and starting the
/// clock again. A line with a break in it is two lines, as the panel has
/// eight of them.
fn say(panel: &mut Vec<String>, saying: &mut f32, text: &str) {
    for line in text.split('\n') {
        panel.push(line.trim().to_string());
    }
    while panel.len() > 8 {
        panel.remove(0);
    }
    // The phrase table's own durations are two to five seconds; four is the
    // middle of them and what this uses until a line carries its own.
    *saying = 4.0;
}

/// Whether a level has a briefing at all - only the first of each chapter
/// does.
fn has_briefing(game: &Pod, stem: &str) -> bool {
    game.read("data", &format!("{stem}.txt")).is_ok_and(|b| {
        hb_formats::brief::Brief::parse(b).is_ok_and(|s| !s.lines.is_empty())
    })
}

/// Break lines to a width in pixels, keeping the blank ones - they are the
/// paragraph breaks and the briefing reads as prose without them.
fn wrap(lines: &[String], font: &hb_formats::hud_font::HudFont, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            out.push(String::new());
            continue;
        }
        let mut row = String::new();
        for word in line.split_whitespace() {
            let candidate =
                if row.is_empty() { word.to_string() } else { format!("{row} {word}") };
            if font.width(&candidate) > width && !row.is_empty() {
                out.push(std::mem::take(&mut row));
                row = word.to_string();
            } else {
                row = candidate;
            }
        }
        if !row.is_empty() {
            out.push(row);
        }
    }
    out
}

/// The alpha a window's pixel needs. minifb hands the buffer to a 32-bit
/// visual, and a compositing window manager reads the top byte: leave it
/// zero and the dark parts of the picture are a hole through to the desktop.
pub const OPAQUE: u32 = 0xff00_0000;

/// The briefing for a level, as a window-sized buffer of 0xffRRGGBB - or
/// nothing when the level has none, which is every level that is not the
/// first of its chapter.
fn briefing_for(
    game: &Pod,
    screen: Option<&(Image, act::Palette)>,
    font: Option<&hb_formats::hud_font::HudFont>,
    stem: &str,
    w: usize,
    h: usize,
    revealed: usize,
) -> Option<Vec<u32>> {
    use hb_formats::hud_font::LINE;
    let (image, palette) = screen?;
    let font = font?;
    let brief =
        hb_formats::brief::Brief::parse(game.read("data", &format!("{stem}.txt")).ok()?).ok()?;
    if brief.lines.is_empty() {
        return None;
    }
    let mut pixels = image.pixels.clone();
    let (iw, ih) = (image.shape.width, image.shape.height);
    let ink = (0..=255u8)
        .max_by_key(|&i| palette.rgb(i).iter().map(|&c| c as u32).sum::<u32>())
        .unwrap_or(255);
    // Inside the panel, wrapped to it, and scrolled when there is more than
    // it holds - the engine types the whole thing out and this is where it
    // would have to go.
    let [px, py, pw, ph] = hb_formats::brief::PANEL;
    let wrapped = wrap(&brief.lines, font, pw);
    // Only as much as has been typed so far, and once there is more than the
    // panel holds it scrolls, so the last thing typed is always in view.
    let mut left = revealed;
    let mut shown: Vec<String> = Vec::new();
    for line in &wrapped {
        if left == 0 {
            break;
        }
        let take = left.min(line.chars().count());
        shown.push(line.chars().take(take).collect());
        left -= take;
        // A line break costs a character, so a blank line takes time too.
        left = left.saturating_sub(1);
    }
    let rows = ph / LINE;
    let from = shown.len().saturating_sub(rows);
    for (i, line) in shown[from..].iter().enumerate() {
        font.draw(&mut pixels, iw, ih, px as isize, (py + i * LINE) as isize, line, ink);
    }

    // The screen is 320x200 and the view may not be; scale it in.
    let mut out = vec![0u32; w * h];
    for y in 0..h {
        for x in 0..w {
            let sx = (x * iw / w).min(iw - 1);
            let sy = (y * ih / h).min(ih - 1);
            let [r, g, b] = palette.rgb(pixels[sy * iw + sx]);
            out[y * w + x] = OPAQUE | ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
        }
    }
    Some(out)
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
