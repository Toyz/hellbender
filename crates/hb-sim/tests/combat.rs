//! Shots and hits: the engine's numbers, synthetic situations, and one real
//! level.

use hb_formats::text::{EnemyDef, Placement};
use hb_sim::combat::{
    multiplier, player_is_hit, step_shot, weapon, wreck_of, Health, HitVolume, Pilot, Shot, Side,
    Stop, Wreck, SHOT_LIFE,
};

fn placed(x: i32, z: i32, hit_points: i32) -> Placement {
    Placement { kind: 0, hit_points, x: x << 16, y: 0, z: z << 16, pitch: 0, roll: 0, heading: 0 }
}

fn kind(radius: i64, laser: i64, wreck: &str) -> EnemyDef {
    EnemyDef {
        fields: [0, 0, radius, 0, 0, 0],
        model: "thing.bin".into(),
        wreck: wreck.into(),
        damage: [65536, laser, 32768],
        course: -1,
        ..EnemyDef::default()
    }
}

/// A cube of the given half-size, standing in for a model's box.
fn cube(half: f32) -> HitVolume {
    HitVolume::Box { min: [-half; 3], max: [half; 3] }
}

fn fly(shot: &mut Shot, targets: &[Placement], volumes: &[HitVolume], alive: impl Fn(usize) -> bool) -> Option<Stop> {
    // Frames of a thirtieth of a second until the shot stops or expires.
    while shot.alive() {
        if let Some(stop) = step_shot(shot, 1.0 / 30.0, targets, volumes, &alive, [0.0, 500.0, 0.0], |_| false) {
            return Some(stop);
        }
    }
    None
}

#[test]
fn the_players_laser_is_row_one_of_the_weapon_table() {
    let laser = weapon(1).unwrap();
    assert_eq!(laser.speed, 32.0);
    assert_eq!(laser.damage, 1.0 / 16.0);
    // A missile row: 64 units a second and a whole health's worth.
    assert_eq!(weapon(18).unwrap().speed, 64.0);
    assert_eq!(weapon(18).unwrap().damage, 1.0);
}

#[test]
fn a_laser_goes_as_fast_as_the_ship_plus_its_own_speed() {
    let shot = Shot::player_laser([0.0; 3], 0, 0, 20.0);
    assert!((shot.velocity[2] - 52.0).abs() < 1e-3, "{:?}", shot.velocity);
    // Two seconds of life at that speed.
    assert_eq!(SHOT_LIFE, 2.0);
}

#[test]
fn a_quarter_turn_fires_along_x_and_nose_down_fires_downward() {
    let east = Shot::player_laser([0.0; 3], 0x4000, 0, 0.0);
    assert!(east.velocity[0] > 31.9 && east.velocity[2].abs() < 1e-3);
    // Positive pitch is nose down, as the demo recorded.
    let down = Shot::player_laser([0.0; 3], 0, 0x2000, 0.0);
    assert!(down.velocity[1] < -20.0, "{:?}", down.velocity);
}

#[test]
fn a_shot_stops_in_the_first_thing_it_reaches() {
    let targets = [placed(0, 40, 4096), placed(0, 15, 4096), placed(25, 15, 4096)];
    let volumes = [cube(2.0)];
    let mut shot = Shot::player_laser([0.0; 3], 0, 0, 0.0);
    assert_eq!(fly(&mut shot, &targets, &volumes, |_| true), Some(Stop::Object(1)));
    // With that one gone, the far one is next.
    let mut shot = Shot::player_laser([0.0; 3], 0, 0, 0.0);
    assert_eq!(fly(&mut shot, &targets, &volumes, |i| i != 1), Some(Stop::Object(0)));
}

#[test]
fn a_shot_runs_out_after_two_seconds() {
    // 32 units a second for two seconds: 64 units, and not a step further.
    let targets = [placed(0, 70, 4096)];
    let mut shot = Shot::player_laser([0.0; 3], 0, 0, 0.0);
    assert_eq!(fly(&mut shot, &targets, &[cube(2.0)], |_| true), None);
    let targets = [placed(0, 60, 4096)];
    let mut shot = Shot::player_laser([0.0; 3], 0, 0, 0.0);
    assert_eq!(fly(&mut shot, &targets, &[cube(2.0)], |_| true), Some(Stop::Object(0)));
}

#[test]
fn the_box_turns_with_the_object() {
    // A long thin box, 10 along its own z and 1 across. Turned a quarter, it
    // lies along world x, so a shot up the z axis 5 units off-centre misses it
    // and one 5 units off along x hits.
    let long = HitVolume::Box { min: [-1.0, -1.0, -10.0], max: [1.0, 1.0, 10.0] };
    let mut turned = placed(0, 20, 4096);
    turned.heading = 0x4000;
    let mut shot = Shot::player_laser([0.0, 0.0, 0.0], 0, 0, 0.0);
    assert_eq!(fly(&mut shot, &[turned], std::slice::from_ref(&long), |_| true), Some(Stop::Object(0)));
    let mut shot = Shot::player_laser([5.0, 0.0, 0.0], 0, 0, 0.0);
    assert_eq!(fly(&mut shot, &[turned], std::slice::from_ref(&long), |_| true), Some(Stop::Object(0)));
    let straight = placed(0, 20, 4096);
    let mut shot = Shot::player_laser([5.0, 0.0, 0.0], 0, 0, 0.0);
    assert_eq!(fly(&mut shot, &[straight], std::slice::from_ref(&long), |_| true), None);
}

#[test]
fn a_shot_into_the_ground_stops_there() {
    let mut shot = Shot::player_laser([0.0, 10.0, 0.0], 0, 0x2000, 0.0);
    let mut stopped = None;
    while shot.alive() && stopped.is_none() {
        stopped = step_shot(&mut shot, 1.0 / 30.0, &[], &[], |_| true, [0.0; 3], |p| p[1] < 0.0);
    }
    assert_eq!(stopped, Some(Stop::Ground));
    assert!(shot.position[1] < 0.0 && shot.position[1] > -2.0, "{:?}", shot.position);
}

#[test]
fn enemy_shots_hit_the_player_inside_two_units() {
    assert!(player_is_hit([0.0, 0.0, 0.0], [1.9, -1.9, 1.9]));
    assert!(!player_is_hit([0.0, 0.0, 0.0], [2.0, 0.0, 0.0]));
    // Across the wrap: 511 and -511 are two units apart.
    assert!(player_is_hit([511.0, 0.0, 0.0], [-511.5, 0.0, 0.0]));

    let mut shot = Shot::fire([0.0, 0.0, -20.0], [0.0, 0.0, 1.0], 30.0, 0.125, 2, Side::Enemy);
    let mut stopped = None;
    while shot.alive() && stopped.is_none() {
        stopped = step_shot(&mut shot, 1.0 / 30.0, &[], &[], |_| true, [0.0, 1.0, 0.0], |_| false);
    }
    assert_eq!(stopped, Some(Stop::Player));
}

#[test]
fn hit_points_are_the_placements_and_count_laser_hits() {
    // 12,288 is three laser hits.
    let mut health = Health::for_placement(&placed(0, 0, 12288));
    let laser = weapon(1).unwrap().damage * multiplier(&kind(65536, 65536, "cube.bin"), 1);
    assert!(!health.take(laser));
    assert!(!health.take(laser));
    assert!(health.take(laser), "the third hit destroys it");
    assert!(!health.take(laser), "it cannot be destroyed twice");
}

#[test]
fn the_multiplier_follows_the_weapon_kind() {
    let def = kind(65536, 16384, "cube.bin");
    // Lasers are kinds 1-3: a quarter here.
    assert_eq!(multiplier(&def, 1), 0.25);
    assert_eq!(multiplier(&def, 3), 0.25);
    // Missiles take the third line; the cannon, 23, the first.
    assert_eq!(multiplier(&def, 19), 0.5);
    assert_eq!(multiplier(&def, 23), 1.0);
    // Anything else is unscaled.
    assert_eq!(multiplier(&def, 0), 1.0);
    assert_eq!(multiplier(&def, 7), 1.0);
}

#[test]
fn the_shield_takes_a_share_and_wears_down() {
    let mut pilot = Pilot::default();
    assert_eq!((pilot.health, pilot.shield), (1.0, 0.5));
    // A half shield takes a quarter off.
    assert!(!pilot.take(0.25));
    assert!((pilot.health - (1.0 - 0.25 * 0.75)).abs() < 1e-6);
    assert!((pilot.shield - (0.5 - 1.0 / 32.0)).abs() < 1e-6);
    // Enough missiles kill.
    let mut hits = 1;
    while !pilot.take(0.25) {
        hits += 1;
        assert!(hits < 20);
    }
    assert!(!pilot.alive());
    assert_eq!(hits, 5);
}

#[test]
fn cube_means_nothing_is_left_and_anything_else_is_a_wreck() {
    assert_eq!(wreck_of(&kind(65536, 65536, "cube.bin")), Wreck::Gone);
    assert_eq!(wreck_of(&kind(65536, 65536, "CUBE.BIN")), Wreck::Gone);
    assert_eq!(wreck_of(&kind(65536, 65536, "wbnkruin.bin")), Wreck::Model("wbnkruin.bin".into()));
}

#[test]
fn shooting_a_real_placement_destroys_it() {
    use std::path::PathBuf;
    let dir = std::env::var_os("HB_GAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../original"));
    let path = dir.join("system/GAME.POD");
    if !path.exists() {
        eprintln!("skipping: {} is not there", path.display());
        return;
    }
    let pod = hb_pod::Pod::open(path).unwrap();
    let def = pod.read("data", "float.def").unwrap();
    let kinds = hb_formats::text::enemy_defs(def).unwrap();
    let placed = hb_formats::text::placements(def).unwrap();
    let volumes: Vec<HitVolume> = kinds
        .iter()
        .map(|k| {
            let mesh = pod.read("models", &k.model).ok().and_then(|b| hb_formats::mrgl::Model::parse(b).ok());
            HitVolume::for_type(k, mesh.as_ref())
        })
        .collect();

    // Stand off the first placement along -z, beyond its reach, and fire up
    // +z at its centre until it goes.
    let target = placed[0];
    let reach = volumes[target.kind].reach();
    let (tx, ty, tz) = (target.x as f32 / 65536.0, target.y as f32 / 65536.0, target.z as f32 / 65536.0);
    let mut health = Health::for_placement(&target);
    let mut shots = 0;
    while !health.destroyed && shots < 100 {
        let mut shot = Shot::player_laser([tx, ty, tz - reach - 5.0], 0, 0, 0.0);
        shots += 1;
        if let Some(Stop::Object(0)) = fly(&mut shot, &[target], &volumes, |_| true) {
            health.take(shot.damage * multiplier(&kinds[target.kind], shot.kind));
        }
    }
    assert!(health.destroyed, "not destroyed after {shots} shots");
    // The placement's own count: 81,920 is twenty laser hits.
    assert_eq!(target.hit_points, 81920);
    assert_eq!(shots, 20);
}
