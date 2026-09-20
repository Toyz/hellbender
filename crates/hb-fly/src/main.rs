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
  space         fire                  b       drop a beacon
  ` 1 2 3       Valkyrie cannon, dispersion cannon, servo-kinetic and
                rapid-fire lasers        =       next weapon
  4 5 6         Dead-On, cruise and Viper missiles   v   lock the next target
  , .           main energy to the weapons, to the shield
  esc           quit

  The level's mission runs from its .NAV file: the HUD names the current
  objective, how far it is, and an arrow points at it. Flying into the jump
  zone, or finishing every objective, moves on to the next level; failing
  starts the level again.
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
    /// `fuel` says whether the afterburner has anything to burn.
    fn step(&mut self, window: &Window, dt: f32, fuel: bool) {
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
            afterburner: fuel && down(&[Key::LeftShift, Key::RightShift]),
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

    /// Keep the ship out of the ground and out of the boxes.
    ///
    /// The ground is the height query used honestly: find the surface under
    /// the ship and refuse to go below it by more than the ship's own unit.
    /// The boxes are `hb_sim::collide`, which pushes the ship out of the face
    /// it is least far through, as the engine's response does (`0x4277c0`).
    fn settle(&mut self, grid: &hb_world::Grid) {
        self.grounded = false;
        if !self.collide {
            return;
        }
        let reach = (hb_sim::collide::SHIP * 65536.0) as i32;
        let solids: Vec<hb_sim::collide::Solid> = grid
            .boxes_near(self.camera.x, self.camera.z, reach)
            .into_iter()
            .map(hb_sim::collide::Solid::of)
            .collect();
        let (moved, push) =
            hb_sim::collide::push_out(self.ship.position, hb_sim::collide::SHIP, &solids);
        self.ship.position = moved;
        if push == Some(hb_sim::collide::Push::Up) {
            self.grounded = true;
        }
        // The ground underneath, which the boxes sit on.
        let fixed = |v: f32| (v * 65536.0) as i32;
        let ground = grid
            .height_at(hb_formats::terrain::Layer::Ground, fixed(moved[0]), fixed(moved[2]))
            .unwrap_or(0);
        let floor = ground as f32 / 65536.0 + hb_sim::collide::SHIP;
        if self.ship.position[1] < floor {
            self.ship.position[1] = floor;
            self.grounded = true;
        }
        self.sync_camera();
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

    let mut buffer = vec![0u32; w * h];
    // Everything that starts again with a level: the ship, the mission, the
    // placements that follow a course (`live` is the copy the renderer draws,
    // rewritten from them each frame), and the fight.
    let (mut flight, mut mission, mut followers, mut live, mut battle, mut colours) = begin(&level);
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
    // A line the mission flashes on the HUD, and for how much longer.
    let mut flash: Option<(String, f32)> = None;
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

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let now = Instant::now();
        let dt = (now - last).as_secs_f32().min(0.1);
        last = now;

        let tab = window.is_key_down(Key::Tab);
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
                index = (index + 1) % LEVELS.len();
            }
            level = Level::load(&game, Some(&startup), LEVELS[index])?;
            describe(&level);
            play_music(&level);
            (flight, mission, followers, live, battle, colours) = begin(&level);
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
        if window.is_key_pressed(Key::B, minifb::KeyRepeat::No) && demo.is_none() {
            for event in mission.drop_beacon(eye_of(&flight.camera)) {
                if let hb_sim::mission::Event::Voice(v) = event {
                    flash = Some((v.text.replace('\n', " "), 3.0));
                    if let (Some(music), Some(s)) = (music.as_ref(), sound(v.sound)) {
                        music.effect(&s, 1.0);
                    }
                } else if let hb_sim::mission::Event::Message(m) = event {
                    flash = Some((m.to_string(), 3.0));
                }
            }
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
                flight.step(&window, dt, battle.stores.fuel > 0.0);
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
            battle.guns.track(window.is_key_pressed(Key::V, minifb::KeyRepeat::No), candidates.len(), |i| {
                candidates[i].lockable(selected)
            });
            let burn = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
            let (volleys, mut voices) = battle.trigger(window.is_key_down(Key::Space), burn, dt, &pose);
            for key in [
                (Key::Backquote, '`'),
                (Key::Key1, '1'),
                (Key::Key2, '2'),
                (Key::Key3, '3'),
                (Key::Key4, '4'),
                (Key::Key5, '5'),
                (Key::Key6, '6'),
            ] {
                if window.is_key_pressed(key.0, minifb::KeyRepeat::No) {
                    if let Some(&(_, w)) = hb_sim::weapons::KEYS.iter().find(|(c, _)| *c == key.1) {
                        voices.extend(battle.guns.select(w, &battle.stores));
                    }
                }
            }
            if window.is_key_pressed(Key::Equal, minifb::KeyRepeat::No) {
                battle.guns.next(&battle.stores);
            }
            if window.is_key_pressed(Key::Comma, minifb::KeyRepeat::No) {
                voices.extend(hb_sim::weapons::transfer_to_weapons(&mut battle.stores));
            }
            if window.is_key_pressed(Key::Period, minifb::KeyRepeat::No) {
                voices.extend(hb_sim::weapons::transfer_to_shields(&mut battle.stores, &mut battle.pilot.shield));
            }
            voices.extend(battle.tick(dt));
            for volley in volleys {
                if let (Some(music), Some(s)) = (music.as_ref(), sound(volley.sound)) {
                    music.effect(&s, 0.5);
                }
            }
            for v in voices {
                flash = Some((v.text.replace('\n', " "), 3.0));
                if let (Some(music), Some(s)) = (music.as_ref(), sound(v.sound)) {
                    music.effect(&s, 1.0);
                }
            }
        }

        let grid = hb_world::Grid::new(&level.terrain);
        let solid = |p: [f32; 3]| {
            let (x, z) = ((p[0] * 65536.0) as i32, (p[2] * 65536.0) as i32);
            (p[1] * 65536.0) as i32 <= grid.ceiling_of_solid(x, z)
        };
        // In a demo the recorded flight cannot dodge, so nothing shoots back.
        let ground = |x: f32, z: f32| grid.ceiling_of_solid((x * 65536.0) as i32, (z * 65536.0) as i32) as f32 / 65536.0;
        let axes = [flight.ship.right, flight.ship.up, flight.ship.forward];
        let noises = if demo.is_none() {
            battle.step(&level, &mut live, eye, velocity, Some((&axes, flight.ship.speed())), dt, &solid, &ground)
        } else {
            battle.step(&level, &mut live, [0.0, 1.0e6, 0.0], [0.0; 3], None, dt, &|_| false, &ground)
        };
        for noise in noises {
            let (name, volume) = match noise {
                battle::Noise::Destroyed(kind) => {
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
                    Event::Sound(name) => Some(name),
                    Event::Voice(v) => {
                        flash = Some((v.text.replace('\n', " "), 3.0));
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
                println!("picked up {}", hb_sim::powerup::KINDS[kind].0);
                if kind == hb_sim::powerup::MESSAGE_POD {
                    mission.pod_taken(i);
                }
            }
            for event in said {
                use hb_sim::powerup::Event;
                let heard = match event {
                    Event::Voice(v) => {
                        flash = Some((v.text.replace('\n', " "), 3.0));
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

        if let Some((_, left)) = &mut flash {
            *left -= dt;
            if *left <= 0.0 {
                flash = None;
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
        let frames_now = level.texture_frames(started.elapsed().as_secs_f32());
        let mut scene = level.scene();
        scene.frames = Some(&frames_now);
        scene.sky_scroll = level.sky_scroll(started.elapsed().as_secs_f32());
        scene.seconds = started.elapsed().as_secs_f32();
        // The powerups are drawn as more placements, turned to face the eye.
        // The engine turns them by two of the view's angles (`0x4266c0`
        // passes `0x5b37c8` and `0x5b37c4` to `0x42aa30`); which two is not
        // settled, so this turns them by the heading alone.
        let mut drawn = live.clone();
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
        if show_hud {
            if let Some(font) = &hud_font {
                let weapon = battle.guns.selected;
                let stock = match battle.stores.ammo[weapon] {
                    -1 => String::new(),
                    n => format!(" {n}"),
                };
                let lock = match battle.guns.lock {
                    Some(_) => "*",
                    None => "",
                };
                let readout = format!(
                    "{}{}{}  HP {:.0}  SH {:.0}  WE {:.0}  EN {:.0}",
                    hb_sim::weapons::ROWS[weapon].code,
                    stock,
                    lock,
                    battle.pilot.health * 100.0,
                    battle.pilot.shield * 100.0,
                    battle.stores.weapon_energy * 100.0,
                    battle.stores.energy * 100.0,
                );
                // The font is drawn at its authored size, which is 23 pixels
                // tall - more than a tenth of a 200-line screen, so it sits in
                // the top corner and is meant to be read, not admired.
                font.draw(&mut target.colour, w, h, 4, 3, &readout, Some(255));

                // The objective, how far, and the clock if there is one.
                if demo.is_none() {
                    let status = match mission.outcome {
                        Some(hb_sim::mission::Outcome::Failed) => "MISSION FAILED".to_string(),
                        Some(_) => "MISSION COMPLETE".to_string(),
                        None => {
                            let mut line = format!("{}  {:.0}", mission.label, mission.distance);
                            if let Some(t) = mission.time_left() {
                                line += &format!("  {:.0} s", t.ceil());
                            }
                            line
                        }
                    };
                    let line = hb_formats::font::HEIGHT + 4;
                    font.draw(&mut target.colour, w, h, 4, (3 + line) as isize, &status, Some(255));
                    if mission.outcome.is_none() {
                        draw_arrow(&mut target.colour, w, h, (w / 2, 3 + line * 3 / 2), mission.arrow, mission.near);
                    }
                }
                if let Some((text, _)) = &flash {
                    let y = h.saturating_sub(hb_formats::font::HEIGHT + 4);
                    font.draw(&mut target.colour, w, h, 4, y as isize, text, Some(255));
                }
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
            let kind = level.kinds.get(p.kind)?;
            // The flyers' routine does not read the course.
            if battle::FLYING.contains(&kind.class()) {
                return None;
            }
            let c = kind.course;
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

/// The HUD's nav arrow: a short line from `centre` toward the current point,
/// brighter within 60 units. `arrow` is the engine's (`0x59d118`), which runs
/// from the point to the player, so the way to go is half a turn from it.
fn draw_arrow(pixels: &mut [u8], w: usize, h: usize, centre: (usize, usize), arrow: u16, near: bool) {
    let turn = arrow.wrapping_add(0x8000) as f32 / 65536.0 * std::f32::consts::TAU;
    let (dx, dy) = (turn.sin(), -turn.cos());
    let colour = if near { 255 } else { 250 };
    let mut plot = |x: f32, y: f32| {
        let (x, y) = (x.round() as isize, y.round() as isize);
        if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
            pixels[y as usize * w + x as usize] = colour;
        }
    };
    let (cx, cy) = (centre.0 as f32, centre.1 as f32);
    let length = 10.0;
    for i in 0..=(length as usize * 2) {
        let t = i as f32 / 2.0;
        plot(cx + dx * t, cy + dy * t);
    }
    // The head: two short strokes back from the tip.
    let (tx, ty) = (cx + dx * length, cy + dy * length);
    for side in [-1.0f32, 1.0] {
        let (bx, by) = (-dx * 0.7 + side * -dy * 0.7, -dy * 0.7 + side * dx * 0.7);
        for i in 0..=8 {
            let t = i as f32 / 2.0;
            plot(tx + bx * t, ty + by * t);
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
