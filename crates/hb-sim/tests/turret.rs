//! The class-10 turret and the guided missile, synthetic.

use hb_formats::text::{EnemyDef, Placement, SecondWeapon};
use hb_sim::turret::{Launch, Missile, Rng, Turret, GUIDED};

fn gun(turn_rate: i32, fire_interval: i32, shot_speed: i32, weapon: i32) -> EnemyDef {
    EnemyDef {
        fields: [10, 0, 65536, 0, 0, 0],
        turn_rate,
        fire_interval,
        shot_damage: 8192,
        weapon,
        second_weapon: SecondWeapon { shot_speed, ..SecondWeapon::default() },
        ..EnemyDef::default()
    }
}

fn at_origin() -> Placement {
    Placement { kind: 0, hit_points: 4096, x: 0, y: 0, z: 0, pitch: 0, roll: 0, heading: 0 }
}

/// Run a turret for `seconds` at 30 frames a second, collecting what it fires.
fn run(t: &mut Turret, def: &EnemyDef, player: [f32; 3], velocity: [f32; 3], seconds: f32) -> Vec<Launch> {
    let mut rng = Rng::new(7);
    let p = at_origin();
    let mut out = Vec::new();
    for _ in 0..(seconds * 30.0) as usize {
        if let Some(l) = t.step(def, None, &p, player, velocity, 1.0 / 30.0, &mut rng) {
            out.push(l);
        }
    }
    out
}

#[test]
fn a_turret_swings_toward_the_player_at_its_turn_rate() {
    // Player due east: heading 0x4000. A turn rate of 2.0 closes the error
    // exponentially - after half a second about e^-1 of it is left.
    let def = gun(2 * 65536, 100 << 16, 20 << 16, 2);
    let mut t = Turret::new(&at_origin());
    run(&mut t, &def, [50.0, 0.0, 0.0], [0.0; 3], 0.5);
    let left = (0x4000 as f32 - t.heading) / 0x4000 as f32;
    assert!((0.33..0.40).contains(&left), "{left} of the turn left");
    run(&mut t, &def, [50.0, 0.0, 0.0], [0.0; 3], 3.0);
    assert!((t.heading - 0x4000 as f32).abs() < 20.0, "{}", t.heading);
}

#[test]
fn it_leads_a_moving_target() {
    // Player 40 units along +z moving +x at 20 a second; shots at 40 a second
    // take one second, so the aim is at (20, 40): atan2(20, 40).
    let def = gun(50 * 65536, 100 << 16, 40 << 16, 2);
    let mut t = Turret::new(&at_origin());
    run(&mut t, &def, [0.0, 0.0, 40.0], [20.0, 0.0, 0.0], 2.0);
    let expected = 20f32.atan2(40.0) * 65536.0 / std::f32::consts::TAU;
    assert!((t.heading - expected).abs() < 30.0, "{} vs {expected}", t.heading);
}

#[test]
fn it_fires_once_per_interval_and_only_at_a_player_in_front() {
    // Half a second between shots, turned to face the player already.
    let def = gun(10 * 65536, 32768, 30 << 16, 2);
    let mut t = Turret::new(&at_origin());
    let shots = run(&mut t, &def, [0.0, 0.0, 30.0], [0.0; 3], 3.0);
    assert_eq!(shots.len(), 5, "three seconds at one every half second, the first after 0.5");
    for l in &shots {
        let Launch::Shot(s) = l else { panic!("a straight shot") };
        assert!(s.velocity[2] > 29.0);
        assert_eq!(s.damage, 0.125);
    }
    // A turret that cannot turn and faces away never fires.
    let stiff = gun(0, 32768, 30 << 16, 2);
    let mut t = Turret::new(&at_origin());
    assert!(run(&mut t, &stiff, [0.0, 0.0, -30.0], [0.0; 3], 3.0).is_empty());
}

#[test]
fn barrel_mode_two_fans_five_shots() {
    let mut def = gun(10 * 65536, 32768, 30 << 16, 2);
    def.second_weapon.barrels = 2;
    let mut t = Turret::new(&at_origin());
    let shots = run(&mut t, &def, [0.0, 0.0, 30.0], [0.0; 3], 3.0);
    let dirs: Vec<[f32; 3]> = shots
        .iter()
        .map(|l| match l {
            Launch::Shot(s) => s.velocity,
            _ => unreachable!(),
        })
        .collect();
    // Barrels 0, 1 (pitched up), 2 (turned left), 3 (pitched down), 4.
    assert!(dirs[0][1].abs() < 0.5 && dirs[0][0].abs() < 0.5);
    assert!(dirs[1][1] > 5.0, "{:?}", dirs[1]);
    assert!(dirs[2][0] < -5.0, "{:?}", dirs[2]);
    assert!(dirs[3][1] < -5.0, "{:?}", dirs[3]);
    assert!(dirs[4][0] > 5.0, "{:?}", dirs[4]);
}

#[test]
fn a_sam_site_launches_a_missile_that_finds_the_player() {
    // The FLOAT SAM site's line 1: no move rate, turn 1.0, 3.9 s, weapon 19.
    let def = gun(65536, 256000, 1_000_000, GUIDED);
    let mut t = Turret::new(&at_origin());
    // Behind it and above: a missile has no in-front test.
    let player = [0.0, 30.0, -40.0];
    let launches = run(&mut t, &def, player, [0.0; 3], 4.0);
    assert_eq!(launches.len(), 1);
    let Launch::Missile(mut m) = launches[0].clone() else { panic!("a missile") };
    // Launched effectively at rest.
    assert!(m.speed < 0.001);
    let mut outcome = None;
    let mut frames = 0;
    while m.alive() && outcome.is_none() {
        outcome = m.step(1.0 / 30.0, Some(player), |p| p[1] < -5.0);
        frames += 1;
    }
    assert_eq!(outcome, Some(true), "it should reach the player");
    // Speed builds at 16 a second: the 50 units take a few seconds.
    let seconds = frames as f32 / 30.0;
    assert!((2.0..4.0).contains(&seconds), "{seconds} s");
}

#[test]
fn a_missile_gives_up_after_six_seconds() {
    let mut m = Missile { position: [0.0; 3], heading: 0.0, pitch: 0.0, speed: 0.0, age: 0.0 };
    // A target it cannot reach in time: 1,000 units is past six seconds of
    // flight even at top speed, and the world wraps well before that, so aim
    // it at something 400 units away on a fast-moving course it never meets.
    let mut frames = 0;
    while m.alive() {
        let target = [400.0, 0.0, frames as f32 * 3.0];
        assert_eq!(m.step(1.0 / 30.0, Some(target), |_| false), None);
        frames += 1;
    }
    assert!((m.age - Missile::LIFE).abs() < 0.05, "{}", m.age);
    assert!(m.speed <= Missile::TOP_SPEED);
}

fn game() -> Option<hb_pod::Pod> {
    let dir = std::env::var_os("HB_GAME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../original"));
    let path = dir.join("system/GAME.POD");
    if !path.exists() {
        eprintln!("skipping: {} is not there", path.display());
        return None;
    }
    Some(hb_pod::Pod::open(path).unwrap())
}

/// Hover near a real turret and see what it does to you.
fn stand_by(level: &str, which: &str, offset: [f32; 3], seconds: f32) -> Option<(usize, usize, hb_sim::combat::Pilot)> {
    use hb_sim::combat::{step_shot, Pilot, Stop};
    let pod = game()?;
    let def = pod.read("data", &format!("{level}.def")).unwrap();
    let kinds = hb_formats::text::enemy_defs(def).unwrap();
    let placed = hb_formats::text::placements(def).unwrap();
    let (index, p) = placed
        .iter()
        .enumerate()
        .find(|(_, p)| kinds[p.kind].model.eq_ignore_ascii_case(which))
        .expect("the level places one");
    let def = &kinds[p.kind];
    let parse = |name: &str| pod.read("models", name).ok().and_then(|b| hb_formats::mrgl::Model::parse(b).ok());
    // A group model stands for its first child, as `hb_render::Level` loads it.
    let mesh = parse(&def.model).and_then(|m| match m.children.first() {
        Some(first) if m.polygons.is_empty() => parse(first),
        _ => Some(m),
    });
    let player = [p.x as f32 / 65536.0 + offset[0], p.y as f32 / 65536.0 + offset[1], p.z as f32 / 65536.0 + offset[2]];
    let mut turret = Turret::new(p);
    let mut rng = Rng::new(3);
    let mut pilot = Pilot::default();
    let (mut fired, mut hits) = (0usize, 0usize);
    let mut shots = Vec::new();
    let mut missiles = Vec::new();
    let dt = 1.0 / 30.0;
    for _ in 0..(seconds / dt) as usize {
        match turret.step(def, mesh.as_ref(), p, player, [0.0; 3], dt, &mut rng) {
            Some(Launch::Shot(s)) => {
                fired += 1;
                shots.push(s)
            }
            Some(Launch::Missile(m)) => {
                fired += 1;
                missiles.push(m)
            }
            None => {}
        }
        shots.retain_mut(|s| {
            match step_shot(s, dt, &[], &[], |_| false, player, |_| false) {
                Some(Stop::Player) => {
                    hits += 1;
                    pilot.take(s.damage);
                    false
                }
                Some(_) => false,
                None => s.alive(),
            }
        });
        missiles.retain_mut(|m| match m.step(dt, Some(player), |_| false) {
            Some(true) => {
                hits += 1;
                pilot.take(Missile::DAMAGE);
                false
            }
            Some(false) => false,
            None => m.alive(),
        });
    }
    let _ = index;
    Some((fired, hits, pilot))
}

#[test]
fn a_hoth_spike_gun_hits_at_its_muzzles_height_and_not_below() {
    // The spike gun's five muzzles are spike tips 4.8 to 8.8 units above its
    // origin, its shots fly level, and it cycles five barrels 11.25 degrees
    // apart, so it sprays. Measured over a minute, 17 units off: a player at
    // the tips' height takes 13 of 59 shots, one at the gun's own height 3.
    let Some((fired, high, pilot)) = stand_by("hoth", "hrspkgnn.bin", [12.0, 7.0, 12.0], 60.0) else {
        return;
    };
    // One a second, less the first second.
    assert_eq!(fired, 59);
    assert_eq!(high, 13);
    // Each hit is 1/16 less a quarter for the starting shield.
    assert!(pilot.health < 0.5 && pilot.alive(), "{pilot:?}");
    let (_, low, _) = stand_by("hoth", "hrspkgnn.bin", [12.0, 0.0, 12.0], 60.0).unwrap();
    assert_eq!(low, 3);
}

#[test]
fn a_float_sam_site_needs_no_aim() {
    // Behind and above: no in-front test for the guided missile.
    let Some((fired, hits, pilot)) = stand_by("float", "SamSite1.Bin", [0.0, 25.0, -30.0], 30.0) else {
        return;
    };
    assert!(fired >= 7, "{fired}");
    assert_eq!(hits, fired, "every missile finds a player who does not move");
    assert!(!pilot.alive() || pilot.health < 0.5, "{pilot:?}");
}

