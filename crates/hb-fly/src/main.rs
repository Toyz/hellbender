//! Fly around a Hellbender level.
//!
//! A window, a keyboard and the port's renderer. The flight model here is not
//! the game's - `hb-sim` will be, when the original's is read - it is enough to
//! move an eye through the world and see whether the world is right.

use std::time::Instant;

use hb_formats::fixed::{from_units, to_units};
use hb_formats::terrain::CELL_SIZE;
use hb_formats::Angle;
mod battle;
mod sound;
mod keys;
mod movie;
mod stick;

use hb_formats::act;
use hb_formats::raw::Image;
use hb_pod::Pod;
use hb_render::{Camera, Level, Target};
use minifb::{Key, Window, WindowOptions};

const USAGE: &str = "\
hb-fly - fly around a Hellbender level

  hb-fly [level] [--mode 200|400|480] [--scale N] [--demo 1|2|3]
         [--no-intro] [--movie NAME] [--no-movies]

  level    a level stem, default `hoth`
  --mode   the game's three screen sizes, default 200 (320x200)
  --scale  integer upscale of the window, default 3
  --demo   replay one of the game's recorded attract-mode flights
  --no-intro   skip the four cutscenes the game opens on
  --movie      play one cutscene by name, e.g. --movie Intro.smk
  --no-movies  skip every cutscene

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
  o             look ahead, right, behind, left
  esc           quit

  And the port's own switches, on keys the game does not bind:

  h  hud on/off     k  cockpit on/off    y  music on/off
  g  collision      p  cycle the level

  A chapter opens with its briefing movie and then its briefing screen; any
  key skips either. The level's
  mission runs from its .NAV file: the HUD shows the current
  objective's code, how far it is, and an arrow on the radar points at it.
  Flying into the jump zone, or finishing every objective, moves on to the
  next level; failing starts the level again.
";

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
        let mut ship = hb_sim::flight::Ship::new([to_units(camera.x), to_units(camera.y), to_units(camera.z)], camera.yaw.0 as f32);
        // Stopped, which is where the engine starts it: `0x512588` is 0 in
        // the image's `.data` and nothing at the level start moves it, so the
        // player gives it power.
        ship.throttle = 0.0;
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
        self.ship.position = hb_formats::vector::in_world(self.ship.position);
        self.sync_camera();
    }

    /// One frame of the jump-out: nose up, full throttle, no auto-level and
    /// no collision - the ship is leaving.
    fn jump_out(&mut self, dt: f32) {
        self.ship.auto_level = false;
        self.ship.throttle = 1.0;
        // `angles` gives the circle unsigned; the nose comes up to -0x3f00.
        let pitch = hb_formats::fixed::signed(self.ship.angles()[0]) / hb_formats::fixed::TURN;
        if pitch > -jump::PITCH {
            let by = (jump::SPIN * dt).min(pitch + jump::PITCH);
            self.ship.pitch_by(-by);
        }
        self.was = self.ship.position;
        self.ship.step(&hb_sim::flight::Controls::default(), dt);
        self.ship.position = hb_formats::vector::in_world(self.ship.position);
        self.sync_camera();
    }

    fn sync_camera(&mut self) {
        let [x, y, z] = self.ship.position;
        let [pitch, roll, heading] = self.ship.angles();
        self.camera.x = from_units(x);
        self.camera.y = from_units(y);
        self.camera.z = from_units(z);
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
    fn settle(&mut self, grid: &hb_world::Grid, scenery: &[hb_sim::collide::Solid]) {
        self.grounded = false;
        if !self.collide {
            return;
        }
        let target = self.ship.position;
        let travel = hb_formats::vector::offset(self.was, target);
        let length = hb_formats::vector::length(travel);
        let pieces = (length / (hb_sim::collide::SHIP / 2.0)).ceil().max(1.0) as usize;
        let mut at = target;
        for piece in 1..=pieces {
            let t = piece as f32 / pieces as f32;
            let along = hb_formats::vector::add(self.was, hb_formats::vector::scale(travel, t));
            let (resolved, held) = self.resolve(grid, scenery, along);
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
    fn resolve(
        &mut self,
        grid: &hb_world::Grid,
        scenery: &[hb_sim::collide::Solid],
        position: [f32; 3],
    ) -> ([f32; 3], bool) {
        let unit = hb_sim::collide::SHIP;
        let mut solids: Vec<hb_sim::collide::Solid> = grid
            .boxes_near(from_units(position[0]), from_units(position[2]), from_units(unit))
            .into_iter()
            .map(hb_sim::collide::Solid::of)
            .collect();
        // And the scenery the engine would have let the ship through - see
        // `hb_sim::collide::solid_of`.
        solids.extend(scenery.iter().filter(|s| {
            (0..3).all(|k| {
                let (lo, hi) = (s.min[k] - unit, s.max[k] + unit);
                if k == 1 {
                    position[k] >= lo && position[k] <= hi
                } else {
                    let middle = (lo + hi) / 2.0;
                    hb_formats::fixed::wrapped(position[k] - middle).abs() <= (hi - lo) / 2.0
                }
            })
        }));
        let (mut at, push) = hb_sim::collide::push_out(position, unit, &solids);
        let mut held = push.is_some();
        if push == Some(hb_sim::collide::Push::Up) {
            self.grounded = true;
        }
        let corners = [(0.0, 0.0), (-unit, -unit), (unit, -unit), (-unit, unit), (unit, unit)];
        let sample = |layer, at: [f32; 3], dx: f32, dz: f32| {
            grid.height_at(layer, from_units(at[0] + dx), from_units(at[2] + dz)).map(to_units)
        };
        // Underground the chamber holds the ship, not the ground: two
        // heightfields with rock where they meet (`0x4290f0`).
        let cell = hb_world::Cell::containing(from_units(at[0]), from_units(at[2]));
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
    // Which cutscenes are still to play. A movie holds the window until it
    // ends or a key skips it.
    let mut pending: Vec<String> = Vec::new();
    let mut movies = true;
    let mut opening = true;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--mode" => mode = it.next().and_then(|v| v.parse().ok()).unwrap_or(200),
            "--scale" => scale = it.next().and_then(|v| v.parse().ok()).unwrap_or(3),
            "--demo" => demo_number = it.next().and_then(|v| v.parse().ok()),
            "--movie" => {
                pending.extend(it.next().cloned());
                movies = false;
                opening = false;
            }
            "--no-intro" => opening = false,
            "--no-movies" => movies = false,
            other if !other.starts_with('-') => level_name = other.to_string(),
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }

    let game = Pod::open(hb_pod::game_dir().join("system/GAME.POD")).map_err(|e| e.to_string())?;
    let startup =
        Pod::open(hb_pod::game_dir().join("system/STARTUP.POD")).map_err(|e| e.to_string())?;

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
    // Four of them: ahead, right, behind and left, which `keyChangeViews`
    // turns between (`0x4209a8`).
    let cockpits: Vec<Option<Image>> = (0..4)
        .map(|which| {
            startup
                .read("art", &hb_render::cockpit::view(which, mode as u32))
                .ok()
                .and_then(|b| Image::parse_guessed(b).ok().flatten())
        })
        .collect();
    let mut view = 0usize;
    let mut show_cockpit = cockpits[0].is_some();
    // The hand on the stick: 27 pictures, three sets of nine, chosen by where
    // the stick is and whether a trigger is down (`0x41fde0`). `[Game]`'s
    // `cockpitHandFlag` turns it off; it is 1 in the engine's own defaults.
    let hands: Vec<Option<Image>> = (0..3)
        .flat_map(|set| (0..9).map(move |cell| (set, cell)))
        .map(|(set, cell)| {
            startup
                .read("art", &hb_render::cockpit::hand(set, cell, mode as u32))
                .ok()
                .and_then(|b| Image::parse_guessed(b).ok().flatten())
        })
        .collect();
    let show_hand = hb_formats::ini::Ini::read(&hb_pod::game_dir().join("system/hellbend.ini"))
        .and_then(|ini| ini.int("Game", "cockpitHandFlag"))
        .unwrap_or(1)
        == 1
        && hands.iter().any(Option::is_some);
    // The game's own key bindings. Without an .INI they are the engine's
    // defaults, which is what a fresh install plays on.
    let (binds, from_ini) = keys::Bindings::load(&hb_pod::game_dir());
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
        "cockpit: {} of the four views, {} of the 27 hands{}",
        cockpits.iter().filter(|c| c.is_some()).count(),
        hands.iter().filter(|h| h.is_some()).count(),
        if show_hand { "" } else { " - cockpitHandFlag is off" }
    );

    // The level's music, if a device will take it.
    let music = match sound::Music::open() {
        Ok(mut music) => {
            // The engine's two volumes, 16.16 in the `.INI` and 1.0 by
            // default. HB_VOLUME scales both on top, for a machine where
            // 1.0 is too much.
            let ini = hb_formats::ini::Ini::read(&hb_pod::game_dir().join("system").join("hellbend.ini"))
                .unwrap_or_default();
            let setting = |name: &str| {
                ini.int("Sound", name).map_or(1.0, |v| to_units(v as i32)).clamp(0.0, 1.0)
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
    // `hb hudfont hudfont.bin --extract` writes the table out on its own, and
    // it is looked for first, so a directory with one in it needs no
    // executable at all.
    let hud_font = hb_render::hud::font_from_disc()
        .map_err(|why| println!("hud: {why}, so no HUD font"))
        .ok();
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
    // The scatter an asteroid starts with, and anything else the loop needs
    // a number for.
    let mut rng = hb_sim::turret::Rng::new(0x5eed);
    // Shots drawn so far, for the one in four that carries a light
    // (`0x50f028`).
    let mut shot_lights = 0usize;
    // Everything that starts again with a level (`Round`).
    let Round {
        mut flight, mut mission, mut followers, mut live, mut battle, mut colours, mut scenery,
        mut doors, mut hoverers, mut gone, mut last_eye, mut weather, mut weather_eye,
        mut lightning
    } = begin(&level, &mut rng);
    // The missile warning and the afterburner's note, while they are held.
    let mut warning: Option<u64> = None;
    let (mut burner, mut burning) = (None, false);

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
    // A chapter's briefing is a movie before it is a screen, and the `.LVL`
    // names it on line 36. The arrival follows it.
    if movies {
        // The game opens on four of them before it opens on anything else.
        if opening && demo.is_none() {
            let mut first: Vec<String> =
                movie::OPENING.iter().map(|s| s.to_string()).collect();
            first.append(&mut pending);
            pending = first;
        }
        if briefing.is_some() {
            pending.extend(level.manifest.briefing_movie.clone());
        }
        pending.extend(opening_of(&level));
    }
    // The one that is playing, its clock, and the buffer it draws into -
    // which is the movie's size, not the game's.
    let mut show: Option<movie::Show> = None;
    let mut reel: Vec<u32> = Vec::new();
    // The jump-out: how long it has been running, and whether its one sound
    // has gone off. See `jump_out`.
    let mut jumping: Option<(f32, bool)> = None;
    // And flying in, which is the port's own - see `entry`.
    let mut welcomed = false;
    // Whether the opening camera has had its turn. It is armed once, not once
    // a frame - without this it re-arms the moment it lands and never ends.
    let mut opened = false;
    // `HB_DOORS=1` says where each shot landed and what it started.
    let trace_doors = std::env::var_os("HB_DOORS").is_some();
    // The opening camera: how far it still has to fall, and where it has spun
    // to. `None` once it has landed.
    let mut arriving: Option<(f32, f32)> = None;

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

        // A movie holds everything ahead of the briefing screen. It starts
        // its own soundtrack over the music and any key cuts it short.
        if show.is_none() && !pending.is_empty() {
            let name = pending.remove(0);
            match movie::Show::open(&hb_pod::game_dir(), &name) {
                Some(mut opened) => {
                    println!("movie: {}", opened.name);
                    if let (Some(track), Some(music)) = (opened.sound(), music.as_ref()) {
                        music.stop();
                        music.effect(&std::sync::Arc::new(track), 1.0);
                    }
                    opened.begin();
                    show = Some(opened);
                }
                None => eprintln!("movie: {name} is not under {}", movie::STORY),
            }
        }
        if let Some(playing) = show.as_mut() {
            let skipped =
                window.get_keys_pressed(minifb::KeyRepeat::No).iter().any(|k| *k != quit);
            let more = playing.step();
            // Nothing else in the frame: the world behind a movie is work
            // nobody sees, and at 640x480 it is enough to make it stutter.
            // The movie keeps its own 320x240 and the window stretches it.
            let (mw, mh) = playing.size();
            reel.resize(mw * mh, 0);
            playing.draw(&mut reel, mw, mh);
            window.update_with_buffer(&reel, mw, mh).map_err(|e| e.to_string())?;
            if !more || skipped {
                show = None;
                if let Some(music) = music.as_ref() {
                    music.silence();
                }
                play_music(&level);
            }
            last = Instant::now();
            continue;
        }

        // A briefing holds everything until a key is pressed, the way a
        // briefing screen does. Nothing moves behind it.
        if show.is_none() && briefing.is_some() {
            typing += dt;
            dt = 0.0;
            if window.get_keys_pressed(minifb::KeyRepeat::No).iter().any(|k| *k != quit) {
                briefing = None;
                last = Instant::now();
            }
        }

        // The opening camera is armed on the first running frame, over the
        // ship, and Eve waits for it to land.
        let mut held = false;
        if !opened && dt > 0.0 && show.is_none() && briefing.is_none() {
            opened = true;
            let at = flight.ship.position;
            let ground = floor_of(&level)(at);
            arriving = Some(((ground + entry::ABOVE).max(at[1] + entry::STOP), 0.0));
        }

        // Eve introduces herself on the first frame of the cockpit, which is
        // after the spin rather than over it: `0x4201a9` is in the cockpit's
        // own per-frame code, and it plays phrase 128 and clears the flag the
        // new-game routine armed at `0x4834e3`.
        if let Some((height, spin)) = arriving.as_mut() {
            if dt > 0.0 {
                // The engine's loop only draws: nothing flies, nothing ticks,
                // so the eye stays over the ship instead of chasing it.
                held = true;
                *height -= entry::FALL * dt;
                *spin += entry::SPIN * dt;
                let skipped =
                    window.get_keys_pressed(minifb::KeyRepeat::No).iter().any(|k| *k != quit);
                if skipped || *height <= flight.ship.position[1] + entry::STOP {
                    arriving = None;
                }
            }
        } else if !welcomed && dt > 0.0 && show.is_none() && briefing.is_none() {
            welcomed = true;
            if let Some(eve) = hb_sim::phrases::phrase(WELCOME) {
                say(&mut panel, &mut saying, eve.text);
                if let (Some(music), Some(s)) = (music.as_ref(), sound(eve.sound)) {
                    music.effect(&s, 1.0);
                }
            }
        }

        if held {
            dt = 0.0;
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
            // A switch may name a texture the level's own `.TEX` list does
            // not carry - `SUPRSWT1.RAW` in `MORBOS` - so load it and give it
            // a slot rather than leaving the switch stuck on one face.
            let index = match level
                .texture_names
                .iter()
                .position(|n| n.to_ascii_lowercase() == wanted)
            {
                Some(index) => index,
                None => {
                    let Some(image) = startup
                        .read("art", &wanted)
                        .or_else(|_| game.read("art", &wanted))
                        .ok()
                        .and_then(|b| Image::parse_guessed(b).ok().flatten())
                    else {
                        continue;
                    };
                    level.texture_names.push(name.clone());
                    level.textures.push(Some(image));
                    level.mips.push([None, None]);
                    level.texture_names.len() - 1
                }
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
                    if outcome == hb_sim::mission::Outcome::Jumped {
                        jumping = Some((0.0, false));
                    }
                }
                ended += dt;
                // A jump zone is flown out of; anything else just ends.
                let over = match jumping {
                    Some((clock, _)) => {
                        clock > jump::SECONDS
                            || window.get_keys_pressed(minifb::KeyRepeat::No).iter().any(|k| *k != quit)
                    }
                    None => ended > 3.0,
                };
                if over {
                    jumping = None;
                }
                over.then_some(outcome != hb_sim::mission::Outcome::Failed)
            }
            _ => None,
        };
        if (tab && !tab_was_down) || next.is_some() {
            if next != Some(false) {
                index = (index + 1) % hb_formats::campaign::CAMPAIGN.len();
            }
            let leaving = level.manifest.story_movies.clone();
            level = Level::load(&game, Some(&startup), hb_formats::campaign::CAMPAIGN[index].stem)?;
            briefing = has_briefing(&game, &level.stem).then(|| level.stem.clone());
            typing = 0.0;
            if movies {
                // The level being left says goodbye before the next one
                // arrives - `HOTH3` names `snowout.smk`, `ROID4` `astrout.smk`.
                pending.extend(departure(&leaving));
                if briefing.is_some() {
                    pending.extend(level.manifest.briefing_movie.clone());
                }
                pending.extend(opening_of(&level));
            }
            describe(&level);
            play_music(&level);
            Round {
                flight, mission, followers, live, battle, colours, scenery, doors, hoverers,
                gone, last_eye, weather, weather_eye, lightning
            } = begin(&level, &mut rng);
            ended = 0.0;
            dying = None;
            flash = None;
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
        if pressed(binds.change_views) {
            view = (view + 1) % 4;
            if cockpits[view].is_none() {
                view = 0;
            }
        }
        if window.is_key_pressed(binds.cockpit, minifb::KeyRepeat::No) {
            show_cockpit = !show_cockpit && cockpits[0].is_some();
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
            None => match jumping.as_mut() {
                // Flying out takes the controls away: the ship stands on its
                // tail at full throttle and the camera swings twice round it.
                Some((clock, sounded)) => {
                    *clock += dt;
                    flight.jump_out(dt);
                    if !*sounded && *clock >= jump::SOUND {
                        *sounded = true;
                        if let (Some(music), Some(s)) = (music.as_ref(), sound(jump::BLAST)) {
                            music.effect(&s, 1.0);
                        }
                    }
                }
                None => {
                    flight.step(&window, &binds, joystick.as_ref(), dt, battle.stores.fuel > 0.0);
                    flight.settle(&hb_world::Grid::new(&level.terrain), &scenery);
                }
            },
        }
        target.clear(0);
        let eye = eye_of(&flight.camera);
        let velocity = if dt > 0.0 {
            hb_formats::vector::scale(hb_formats::vector::offset(last_eye, eye), 1.0 / dt)
        } else {
            [0.0; 3]
        };
        last_eye = eye;

        // The guns, the afterburner's fuel, and the energy that feeds both:
        // `hb_sim::weapons`, from the trigger at `0x47db11` on.
        if demo.is_none() && battle.pilot.alive() {
            let pose = hb_sim::weapons::Pose {
                position: eye,
                right: flight.ship.right,
                up: flight.ship.up,
                forward: flight.ship.forward,
                pitch: flight.camera.pitch.0 as i16 as f32,
                heading: flight.camera.yaw.0 as f32,
                speed: hb_formats::vector::length(flight.ship.world_velocity()),
            };
            // The missile lock (`keyMissileLock`, V): what the selected weapon
            // can lock on, as `0x47bbd0` judges it.
            let selected = battle.guns.selected;
            let candidates: Vec<hb_sim::weapons::Candidate> = live
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let at = hb_sim::combat::position_of(p);
                    let near = hb_formats::vector::nearest(eye, at);
                    let kind = &level.kinds[level.placements[i].kind];
                    hb_sim::weapons::Candidate {
                        class: kind.class(),
                        friendly: kind.friendly,
                        alive: !battle.health[i].destroyed && battle.health[i].hit_points > 0.0,
                        near: hb_sim::combat::in_range(eye, at),
                        view: flight.camera.to_view(from_units(near[0]), from_units(near[1]), from_units(near[2])),
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
            let (x, z) = (from_units(p[0]), from_units(p[2]));
            let y = from_units(p[1]);
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
        let ground = |x: f32, z: f32| grid.solid_top(x, z);
        let axes = [flight.ship.right, flight.ship.up, flight.ship.forward];
        let noises = if demo.is_none() {
            battle.step(&level, &mut live, eye, velocity, Some((&axes, flight.ship.speed())), dt, &solid, &ground, &grid)
        } else {
            battle.step(&level, &mut live, [0.0, 1.0e6, 0.0], [0.0; 3], None, dt, &|_| false, &ground, &grid)
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
                from_units(hit[0]),
                from_units(hit[2]),
            );
            let started = doors.shot((cell.x, cell.z), hit[1] * hb_sim::quake::WORD);
            // And a lamp on the face it hit takes the shot (`0x48c800`); one
            // that goes out bursts where it was hit (`0x476db4`).
            let face = hb_sim::lights::face_at(hit, &level.terrain);
            let (paint, broke) = level.lamps.shot(face);
            if let Some(paint) = paint {
                paint.face.paint(&mut level.terrain, paint.texture);
            }
            if broke {
                battle.burst(hit, 1.0);
            }
            if trace_doors {
                println!(
                    "shot: cell ({:3},{:3}) at {:6.1} words -> {started} door(s) started",
                    cell.x,
                    cell.z,
                    hit[1] * hb_sim::quake::WORD
                );
            }
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
            let under = |p: [f32; 3]| grid.solid_top(p[0], p[2]);
            let mut at = flight.ship.position;
            let ground = under(at);
            let wants = wreck.step(&mut at, dt, ground);
            flight.ship.position = at;
            flight.camera.x = from_units(at[0]);
            flight.camera.y = from_units(at[1]);
            flight.camera.z = from_units(at[2]);
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
                    Round {
                        flight, mission, followers, live, battle, colours, scenery, doors,
                        hoverers, gone, last_eye, weather, weather_eye, lightning
                    } = begin(&level, &mut rng);
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
            [placed.x, placed.y, placed.z] = moved.at.map(from_units);
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

        let floor = floor_of(&level);
        for (i, follower) in &mut followers {
            if battle.health[*i].destroyed {
                continue;
            }
            // Only actors within the engine's 80-unit box think.
            if !hb_sim::combat::in_range(eye, hb_sim::combat::position_of(&live[*i])) {
                continue;
            }
            follower.step(dt, &floor);
            let [x, y, z] = follower.position_fixed();
            let placed = &mut live[*i];
            placed.x = x;
            placed.y = y;
            placed.z = z;
            placed.heading = follower.heading();
        }
        drop(floor);

        // Animated textures advance on the wall clock.
        // The animated models, before the scene borrows the level.
        level.animate(started.elapsed().as_secs_f32());
        // Lightning (`0x49d290`), only with the eye at or above the ground,
        // and the bolt of a strike that is lighting the sky, drawn afresh
        // each frame.
        let bolt = {
            let eye = camera_at(&flight.camera);
            let sky = from_units(level.sky_height());
            if eye[1] >= 0 {
                for event in lightning.step(dt, eye, sky, &mut rng) {
                    let (name, at) = match event {
                        hb_sim::weather::Flash::Struck { at } => ("lghtng.wav", Some(at)),
                        hb_sim::weather::Flash::Thunder { at } => ("thun-c.wav", Some(at)),
                        hb_sim::weather::Flash::Dark => ("", None),
                    };
                    if let (Some(at), Some(music), Some(s)) = (at, music.as_ref(), sound(name)) {
                        let at = at.map(to_units);
                        music.effect(&s, hb_sim::combat::falloff(eye_of(&flight.camera), at));
                    }
                }
            }
            // The bolt, while the flash lasts and the eye is below the sky
            // layer (`0x49d490`).
            match lightning.flashing() {
                Some(strike) if eye[1] <= sky => {
                    let grid = hb_world::Grid::new(&level.terrain);
                    hb_sim::weather::bolt(strike.at, |p| grid.ceiling_of_solid(p[0], p[2]), &mut rng)
                }
                _ => Vec::new(),
            }
        };
        // The lamps (`0x48ae30`), and every fourth shot's own light
        // (`0x476e8c`).
        let lights = {
            let eye = camera_at(&flight.camera);
            let cell = hb_world::grid::Cell::containing(eye[0], eye[2]);
            let (mut lights, paints) = level.lamps.step(dt, (cell.x as usize, cell.z as usize));
            for paint in paints {
                paint.face.paint(&mut level.terrain, paint.texture);
            }
            for flying in &battle.shots {
                shot_lights += 1;
                if shot_lights % 4 == 1 {
                    lights.push(hb_sim::lights::Light::moving(flying.shot.position, hb_sim::lights::SHOT_LIGHT));
                }
            }
            lights
        };
        let frames_now = level.texture_frames(started.elapsed().as_secs_f32());
        let mut scene = level.scene();
        scene.lights = &lights;
        // A flash lights every model to full and swaps in the bright sky
        // (`0x48a640`, `0x451450`); the ground's light was fixed at load.
        if lightning.flashing().is_some() {
            scene.sun_ambient = 1.0;
            if let Some(lit) = level.sky_remap_lit.as_ref() {
                scene.sky_remap = Some(lit);
            }
        }
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
                drawn.push(hb_formats::text::Placement {
                    kind: *mesh,
                    hit_points: 0,
                    x: from_units(item.position[0]),
                    y: from_units(item.position[1]),
                    z: from_units(item.position[2]),
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
        let near = |p: [f32; 3]| hb_formats::vector::nearest(eye, p);
        let shot_at = |drawn: &mut Vec<hb_formats::text::Placement>, mesh, at: [f32; 3], angles: (u16, u16)| {
            drawn.push(hb_formats::text::Placement {
                kind: mesh,
                hit_points: 0,
                x: from_units(at[0]),
                y: from_units(at[1]),
                z: from_units(at[2]),
                pitch: angles.1 as i32,
                roll: 0,
                heading: angles.0,
            });
        };
        // Which way a velocity points, in the engine's circle.
        let along = |v: [f32; 3]| {
            let (heading, pitch) = hb_formats::vector::angles_of(v);
            (heading as i32 as u16, pitch as i32 as u16)
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
        // The player's own ship, which is only ever seen while the eye is off
        // it - the opening camera and the jump-out.
        if arriving.is_some() || jumping.is_some() {
            if let Some(mesh) = level.ship_mesh {
                let [pitch, roll, heading] = flight.ship.angles();
                drawn.push(hb_formats::text::Placement {
                    kind: mesh,
                    hit_points: 0,
                    x: from_units(flight.ship.position[0]),
                    y: from_units(flight.ship.position[1]),
                    z: from_units(flight.ship.position[2]),
                    pitch: pitch as i32,
                    roll: roll as i32,
                    heading: heading as u16,
                });
            }
        }
        scene.placements = &drawn;
        // The view the frame is drawn from: the ship's, turned by whichever
        // quarter `keyChangeViews` has left it on.
        let seen = if let Some((height, spin)) = arriving {
            // Straight over the ship, looking straight down, turning.
            let mut c = flight.camera;
            c.y = from_units(height);
            c.pitch = Angle(entry::PITCH);
            c.roll = Angle(0);
            c.yaw = Angle((spin * hb_formats::fixed::TURN) as i32 as u16);
            c
        } else {
            let mut c = flight.camera;
            let mut turn = hb_render::cockpit::VIEWS[view].0;
            if let Some((clock, _)) = jumping {
                let (yaw, pitch) = jump::look(clock);
                turn = turn.wrapping_add((yaw * hb_formats::fixed::TURN) as i32 as u16);
                c.pitch = Angle(c.pitch.0.wrapping_add((pitch * hb_formats::fixed::TURN) as i32 as u16));
                // And the eye comes off the ship, back along where it looks.
                let (a, b) = (
                    hb_formats::fixed::radians(c.yaw.0.wrapping_add(turn) as f32),
                    hb_formats::fixed::radians(c.pitch.0 as i16 as f32),
                );
                let back = jump::BACK * 65536.0;
                c.x -= (a.sin() * b.cos() * back) as i32;
                c.y += (b.sin() * back) as i32;
                c.z -= (a.cos() * b.cos() * back) as i32;
            }
            c.yaw = Angle(c.yaw.0.wrapping_add(turn));
            c
        };
        hb_render::draw_world(&mut target, &scene, &seen);
        for (at, colour) in sparks {
            hb_render::scene::draw_spark(&mut target, &seen, at, colour);
        }
        // Missile smoke (`0x479480`).
        if let Some(texture) = level.puff.as_ref() {
            for (segment, radius) in battle.smoke.shown() {
                let from = near(segment.from);
                let to = hb_formats::vector::add(from, hb_formats::vector::offset(segment.from, segment.to));
                hb_render::scene::draw_smoke(&mut target, &scene, &seen, from, to, radius, texture);
            }
        }
        // The explosions, each a square facing the eye on its own frame.
        for puff in &battle.blasts.puffs {
            let Some(frame) = puff.frame() else { continue };
            if let Some(Some(texture)) = level.blast.get(frame) {
                hb_render::scene::draw_sprite(
                    &mut target,
                    &scene,
                    &seen,
                    near(puff.position),
                    puff.size,
                    texture,
                );
            }
        }
        // Snow and rain, after the world and before the cockpit
        // (`0x49c9b0`, `0x49ce80`), and only when the eye is above the ground,
        // below the sky layer and under nothing.
        {
            use hb_sim::weather::{self, Weather};
            let at = camera_at(&seen);
            let grid = hb_world::Grid::new(&level.terrain);
            let cell = hb_world::grid::Cell::containing(at[0], at[2]);
            let over = grid
                .box_span(hb_world::Layer::BoxA, cell)
                .map(|(bottom, _)| bottom)
                .filter(|&bottom| at[1] < bottom);
            let sky = from_units(level.sky_height());
            let ship = camera_at(&flight.camera);
            let moved: [i32; 3] = std::array::from_fn(|k| hb_formats::fixed::wrap(ship[k] - weather_eye[k]));
            weather_eye = ship;
            if Weather::falls(at[1], sky, over) {
                if level.manifest.has_snow() {
                    Weather::step(&mut weather.snow, dt, at);
                    for flake in weather.snow.iter().filter(|p| p.shown) {
                        hb_render::scene::draw_flake(&mut target, &seen, flake.position, weather::SNOW_INDEX);
                    }
                }
                if level.manifest.has_rain() {
                    Weather::step(&mut weather.rain, dt, at);
                    for drop in weather.rain.iter().filter(|p| p.shown) {
                        let end = Weather::streak_end(drop, moved);
                        hb_render::scene::draw_streak(&mut target, &seen, drop.position, end, weather::RAIN_INDEX);
                    }
                }
            }
        }
        hb_render::scene::draw_bolt(&mut target, &seen, &bolt);
        // The targeting marks (`0x47b700`): the current objective's target
        // (`0x472ae0`) and the lock. Not while the eye is off the ship -
        // nothing of the cockpit is.
        if arriving.is_none() {
            let mut mark = |i: usize, size: isize, colour: u8, bar: bool| {
                let (Some(p), Some(health)) = (live.get(i), battle.health.get(i)) else { return };
                // Only an actor that ran this frame: standing, and within the
                // 80-unit box (`+0x84`).
                let at = hb_sim::combat::position_of(p);
                if health.destroyed || !hb_sim::combat::in_range(eye, at) {
                    return;
                }
                let [x, y, z] = hb_formats::vector::nearest(eye, at).map(from_units);
                let v = seen.to_view(x, y, z);
                // In front, and inside ninety degrees each way.
                if v[2] <= 0.0 || v[0].abs() > v[2] || v[1].abs() > v[2] {
                    return;
                }
                let (sx, sy) = Camera::to_screen(v, w, h);
                let (sx, sy) = (sx as isize, sy as isize);
                let full = to_units(level.placements[i].hit_points);
                let bar = bar.then_some((health.hit_points, full));
                if hb_render::hud::is_target(level.kinds[p.kind].class()) {
                    hb_render::hud::target_box(&mut target, sx, sy, size, colour, bar);
                } else {
                    hb_render::hud::target_diamond(&mut target, sx, sy, size, colour, bar);
                }
            };
            // The lock first, and the objective over it.
            if let Some(i) = battle.guns.lock {
                mark(i, hb_render::hud::LOCK_SIZE, hb_render::hud::LOCK_COLOUR, false);
            }
            use hb_formats::nav::{Data, Kind};
            if mission.current < mission.len() {
                let nav = mission.nav(mission.current);
                let objective = match (nav.kind, &nav.data) {
                    // The first of the list still standing (`0x472b0d`).
                    (Kind::Destroy, Data::Targets(list)) => list
                        .iter()
                        .copied()
                        .find(|&t| battle.health.get(t).is_some_and(|h| !h.destroyed))
                        .map(|t| (t, hb_render::hud::OBJECTIVE_COLOUR)),
                    (Kind::Kill, Data::Actor(t)) => Some((*t, hb_render::hud::OBJECTIVE_COLOUR)),
                    (Kind::Escort, Data::Actor(t)) => Some((*t, hb_render::hud::ESCORT_COLOUR)),
                    _ => None,
                };
                if let Some((t, colour)) = objective {
                    mark(t, hb_render::hud::OBJECTIVE_SIZE, colour, true);
                }
            }
        }
        // The cockpit is not drawn while the eye is off the ship, which is
        // what the engine's own outside view must do too.
        if show_cockpit && jumping.is_none() && arriving.is_none() {
            if let Some(Some(art)) = cockpits.get(view) {
                target.overlay(art);
            }
            // The hand is drawn in the forward view only (`0x420ab6`).
            if show_hand && view == 0 {
                // Where the stick is: the flight model's own ramped inputs,
                // which is what the engine hands the chooser after dividing
                // its `0x2492` scale back out.
                let [up, down, left, right, ..] = flight.ship.keys;
                let held = |k: Option<minifb::Key>| k.is_some_and(|k| window.is_key_down(k));
                let set = if held(binds.weapon)
                    || joystick.as_ref().is_some_and(|s| s.button(stick::WEAPON))
                {
                    hb_render::cockpit::WEAPON
                } else if held(binds.fire)
                    || joystick.as_ref().is_some_and(|s| s.button(stick::FIRE))
                {
                    hb_render::cockpit::FIRE
                } else {
                    hb_render::cockpit::REST
                };
                let cell = hb_render::cockpit::cell(right - left, up - down);
                if let Some(Some(art)) = hands.get(set * 9 + cell) {
                    let [x, y, ..] = hb_render::cockpit::hand_box(w, h);
                    target.overlay_at(art, x, y);
                }
            }
        }
        // Nothing of the cockpit while the eye is off the ship: no picture, no
        // readouts, no crosshair.
        if show_reticle && arriving.is_none() {
            if let Some(model) = &reticle {
                pulse += pulse_step;
                if pulse >= 63 || pulse <= 32 {
                    pulse_step = -pulse_step;
                }
                hb_render::hud::reticle(&mut target, model, pulse as u8);
            }
        }
        if show_hud && arriving.is_none() {
            if let Some(font) = &hud_font {
                // The radar, which the engine fills from every object that is
                // alive and not hidden, turned so the nose points up
                // (`0x437148`). Blips beyond the box are dropped by the draw.
                let heading = flight.camera.yaw.to_radians();
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
                        let [dx, _, dz] = hb_formats::vector::offset(eye, at);
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
                    if arriving.is_none() {
                        hb_render::hud::message(&mut target, font, text);
                    }
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
                to_units(flight.camera.y),
                to_units(ground),
                flight.ship.speed(),
                if flight.grounded { "  [on the deck]" } else { "" }
            );
            frames = 0;
            since = Instant::now();
        }
    }
    Ok(())
}

/// The level's lightning, armed as it loads when its weather has the bit
/// (`0x49d220`), with the `.LVL`'s line 42 as the engine takes it.
fn lightning_for(level: &Level, rng: &mut hb_sim::turret::Rng) -> hb_sim::weather::Lightning {
    if !level.manifest.has_lightning() {
        return hb_sim::weather::Lightning::default();
    }
    let [base, range] = level.manifest.weather_params[1];
    hb_sim::weather::Lightning::new(base as i32, range as i32, rng)
}

/// Where a camera is, 16.16.
fn camera_at(camera: &Camera) -> [i32; 3] {
    [camera.x, camera.y, camera.z]
}

/// The scenery the port makes solid: every placement of a class the engine's
/// own ram test skips, as a world box. See `hb_sim::collide::solid_of` for why
/// this is the port's own and not the engine's.
fn scenery_of(level: &Level) -> Vec<hb_sim::collide::Solid> {
    level
        .placements
        .iter()
        // Above ground only. A chamber is tunnels, and the things standing in
        // them - reactors, cores - are big enough that a box around one fills
        // the passage it is in, so underground the engine's answer stands and
        // the ship flies past.
        .filter(|p| p.y >= 0)
        .filter(|p| !hb_sim::combat::rammable(level.kinds[p.kind].class()))
        .map(|p| {
            let volume = hb_sim::combat::HitVolume::for_type(
                &level.kinds[p.kind],
                level.meshes.get(p.kind).and_then(Option::as_ref),
            );
            hb_sim::collide::solid_of(&volume, hb_sim::combat::position_of(p), p.heading as u16)
        })
        .collect()
}

/// Every placement of a class whose routine reads its course, paired with a
/// follower for it. A type of any other class may name a course and the
/// engine never looks. A dangling course id - six levels have them - is
/// skipped, as the engine's "Bad course ID for enemy" diagnostic implies it
/// copes.
fn followers_for(level: &Level) -> Vec<(usize, hb_sim::Follower)> {
    level
        .placements
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let kind = level.kinds.get(p.kind)?;
            if !hb_sim::course::COURSE_CLASSES.contains(&kind.class()) {
                return None;
            }
            let course = level.courses.get(usize::try_from(kind.course).ok()?)?;
            Some((i, hb_sim::Follower::new(course, p, kind, i)?))
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
            let at = [p.x, p.y, p.z].map(to_units);
            let angles = [p.pitch as f32, p.roll as f32, p.heading as f32];
            let motion = hb_sim::behaviour::Motion::of(kind.class(), at, angles)?;
            Some((i, motion, to_units(kind.radius())))
        })
        .collect()
}

fn eye_of(camera: &Camera) -> [f32; 3] {
    [to_units(camera.x), to_units(camera.y), to_units(camera.z)]
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
        let nearest = |want: [u8; 3]| palette.nearest_in(want, 240..=255);
        ShotColours {
            player: 255,
            enemy: nearest([255, 40, 20]),
            missile: nearest([255, 200, 40]),
        }
    }
}

/// Everything that starts again with a level, whether it is a new one or
/// the same one after the ship went down.
struct Round {
    /// The ship, at the mission's start.
    flight: Flight,
    mission: hb_sim::mission::Mission,
    /// The placements that follow a course. `live` is the copy the renderer
    /// draws, rewritten from them and the fight each frame.
    followers: Vec<(usize, hb_sim::Follower)>,
    live: Vec<hb_formats::text::Placement>,
    battle: battle::Battle,
    colours: ShotColours,
    scenery: Vec<hb_sim::collide::Solid>,
    doors: hb_sim::quake::Quakes,
    hoverers: Vec<(usize, hb_sim::behaviour::Motion, f32)>,
    /// The actors that have left the level: class 17 ships, once they are
    /// high enough. Not destroyed - simply no longer here.
    gone: Vec<bool>,
    /// Where the eye was last frame, for the ship's velocity.
    last_eye: [f32; 3],
    /// Snow and rain, and where the ship was when they last moved.
    weather: hb_sim::weather::Weather,
    weather_eye: [i32; 3],
    lightning: hb_sim::weather::Lightning,
}

/// A level's opening state.
fn begin(level: &Level, rng: &mut hb_sim::turret::Rng) -> Round {
    // The mission's optional points are chosen the same way every time.
    let mut seeded = hb_sim::turret::Rng::new(0x1996);
    let mut mission =
        hb_sim::mission::Mission::new(level.navs.clone(), &level.placements, floor_of(level), &mut seeded);
    if !mission.is_empty() {
        println!("mission: {} points - {}", mission.len(), mission.objective());
    }
    // The `.PUP`'s powerups go down first, so their indices are the ones the
    // mission's message pods name.
    let mut battle = battle::Battle::new(level);
    let floor = floor_of(level);
    for p in &level.powerups {
        let at = p.position.map(to_units);
        let size = level.powerup_size.get(p.kind).copied().unwrap_or(0.0);
        battle.field.place(at, p.kind, size, floor(at));
    }
    let pods: Vec<[f32; 3]> = battle.field.items.iter().map(|p| p.position).collect();
    mission.place_pods(&pods);
    let flight = Flight::new(start_camera(level, &mission));
    let at = camera_at(&flight.camera);
    let followers = followers_for(level);
    let doors = doors_of(level);
    println!(
        "sim: {} of {} objects follow a course, {} turrets, {} flyers, {} doors, {} moving patches of ground",
        followers.len(),
        level.placements.len(),
        battle.turret_count(),
        battle.flyer_count(),
        doors.doors.len(),
        doors.patches.len()
    );
    Round {
        last_eye: eye_of(&flight.camera),
        weather: hb_sim::weather::Weather::new(at, rng),
        weather_eye: at,
        lightning: lightning_for(level, rng),
        followers,
        live: level.placements.clone(),
        gone: vec![false; level.placements.len()],
        colours: ShotColours::for_palette(&level.palette),
        scenery: scenery_of(level),
        doors,
        hoverers: hoverers_for(level),
        flight,
        mission,
        battle,
    }
}

/// The camera a level opens on, which is `0x45a740`.
///
/// It puts the eye straight over the ship - `0x42c980(ship.x, y, ship.z)` -
/// looking straight down, `0x42c9a0(0x3fff, 0, 0)`, and switches to view mode
/// 2, the outside one. Then it falls: `y` loses `0x100000` a second and a yaw
/// gains a quarter of the frame time, until `y` is within `0x20000` of the
/// ship. Any of the three skip keys ends it early, and it is not the sequence
/// at `0x45a2a0` - patching that one to a bare `ret` leaves this untouched.
///
/// The ship does not move while it runs. The loop draws the world and nothing
/// else.
mod entry {
    /// Where the eye starts: the ground under the ship plus half of
    /// `0x5055d4`, which is 128.0, so 64 units up (`0x45a7a4`).
    pub const ABOVE: f32 = 64.0;
    /// How fast it falls, units a second (`0x100000`).
    pub const FALL: f32 = 16.0;
    /// How far above the ship it stops (`0x20000`).
    pub const STOP: f32 = 2.0;
    /// Turns a second, the same quarter the rest of the engine's animations
    /// use.
    pub const SPIN: f32 = 0.25;
    /// Straight down.
    pub const PITCH: u16 = 0x3fff;
}

/// Eve introducing herself once the cockpit is up: phrase 128 of the table at
/// `0x505c20`. `0x4834e3`, in the new-game routine, arms the flag; `0x4201a9`,
/// in the cockpit's own per-frame code, plays the phrase and clears it, which
/// is why it happens once and after whatever came before it.
const WELCOME: usize = 128;

/// Flying out of a jump zone, which is what ends a level (`0x45a2a0`).
///
/// The engine takes the controls away and runs a loop of its own: the ship's
/// roll goes to zero, its pitch climbs a quarter of a turn a second until it
/// is standing on its tail, the throttle is held at full, and the camera
/// swings round the ship a quarter of a turn a second - twice, which is what
/// its clock reaching `0x20000` means. `blast4.wav` goes off seven eighths of
/// the way through, and any of three keys cuts it short.
mod jump {
    /// How long it runs: the engine's clock gains a quarter of the frame time
    /// each frame and ends past 2.0.
    pub const SECONDS: f32 = 8.0;
    /// When `blast4.wav` plays: the clock past `0x1c000`.
    pub const SOUND: f32 = 7.0;
    /// Turns a second, for both the ship's pitch and the camera's swing.
    pub const SPIN: f32 = 0.25;
    /// The camera's pitch rises at half that for the first half of the run.
    pub const RISE: f32 = 0.125;

    /// Where the camera is looking, in turns, at `clock` seconds: the yaw goes
    /// round twice, and the pitch climbs to [`PITCH`] and then swings all the
    /// way past level to the other side (`0x45a374` and `0x45a39b`).
    ///
    /// The engine also puts the camera off the ship for this: `0x512568` goes
    /// to 2 and the view is built at `0x47ffc8` from these two angles around
    /// [`BACK`], which the level start sets.
    pub fn look(clock: f32) -> (f32, f32) {
        let half = SECONDS / 2.0;
        let pitch = if clock < half {
            (clock * RISE).min(PITCH)
        } else {
            ((half * RISE).min(PITCH) - (clock - half) * SPIN).max(-PITCH)
        };
        (clock * SPIN, pitch)
    }
    /// How far the nose comes up: `0xffffc100` of the circle.
    pub const PITCH: f32 = hb_formats::fixed::to_units(0x3f00);
    pub const BLAST: &str = "blast4.wav";
    /// How far behind the ship the outside camera sits: `0x481792` puts
    /// `0x20000` in `0x512558` as a level starts, and `0x480021` is what
    /// pushes the eye off the ship by it.
    pub const BACK: f32 = 2.0;
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
        hb_world::Grid::new(&level.terrain).solid_top(p[0], p[2])
    }
}

/// Where the mission starts the player, or the middle of the map when it has
/// no start.
fn start_camera(level: &Level, mission: &hb_sim::mission::Mission) -> Camera {
    match mission.start {
        Some(start) => {
            let [x, y, z] = start.position.map(from_units);
            let mut camera = Camera::looking_at(x, y, z, Angle(start.angles[2]));
            camera.pitch = Angle(start.angles[0]);
            camera
        }
        None => start_of(level),
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

/// The movies a level opens with, in the order the engine plays them.
///
/// `0x45b9f0` is the routine, called as a level starts (`0x481627`). It plays
/// two of the five story slots and it plays them in this order: the third,
/// then the first. Each is compared against `"null"` first, and the buffers
/// they live in are 40 bytes apart from `0x666f58`, which is what puts them in
/// slot order.
///
/// The names say what the slots are for. Slot 1 is `snowin.smk` in `HOTH`,
/// `morbin.smk` in `MORBOS`, `astrin.smk` in `ROID` and `shiv1in.smk` in
/// `SHIP`, all first levels of a chapter - the arrival. Slot 3 is
/// `morbos1.smk`, `eyrie1.smk`, `chimera1.smk` and so on, one a planet - the
/// chapter's own film. So a level opens on its film and then its arrival.
fn opening_of(level: &Level) -> Vec<String> {
    let story = &level.manifest.story_movies;
    [story[2].clone(), story[0].clone()].into_iter().flatten().collect()
}

/// And the one it leaves on, the second slot.
fn departure(story: &[Option<String>; 5]) -> Option<String> {
    story[1].clone()
}

/// Whether a level has a briefing at all - only the first of each chapter
/// does.
fn has_briefing(game: &Pod, stem: &str) -> bool {
    game.read("data", &format!("{stem}.txt")).is_ok_and(|b| {
        hb_formats::brief::Brief::parse(b).is_ok_and(|s| !s.lines.is_empty())
    })
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
    let (image, palette) = screen?;
    let font = font?;
    let brief =
        hb_formats::brief::Brief::parse(game.read("data", &format!("{stem}.txt")).ok()?).ok()?;
    if brief.lines.is_empty() {
        return None;
    }
    let mut pixels = image.pixels.clone();
    let (iw, ih) = (image.shape.width, image.shape.height);
    // Only as much as has been typed so far, scrolled so the last thing typed
    // is in view - the engine types the whole thing out.
    let rows = hb_render::brief::typed(font, &brief.lines, revealed);
    hb_render::brief::draw(&mut pixels, iw, ih, font, &rows, hb_render::brief::ink(palette));

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
