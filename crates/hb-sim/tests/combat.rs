//! Shots and hits, synthetic.

use hb_formats::text::{EnemyDef, Placement};
use hb_sim::combat::{advance, first_hit, wreck_of, Health, Shot, Wreck, SHOT_SPEED};

fn placed(x: i32, z: i32, scale: i32) -> Placement {
    Placement {
        kind: 0,
        scale,
        x: x << 16,
        y: 0,
        z: z << 16,
        unknown_5: 0,
        unknown_6: 0,
        heading: 0,
    }
}

fn kind(hp_fixed: i64, laser: i64, wreck: &str) -> EnemyDef {
    EnemyDef {
        fields: [0, 0, hp_fixed, 0, 0, 0],
        model: "thing.bin".into(),
        wreck: wreck.into(),
        name: "thing".into(),
        course: -1,
        damage: [65536, laser, 65536],
        friendly: false,
        raw: Vec::new(),
    }
}

#[test]
fn a_shot_fired_along_heading_zero_travels_along_z() {
    let mut shot = Shot::fire([0.0, 0.0, 0.0], 0, 0);
    advance(&mut shot, 0.1);
    assert!(shot.position[0].abs() < 1e-3);
    assert!((shot.position[2] - SHOT_SPEED * 0.1).abs() < 1e-3);
}

#[test]
fn a_quarter_turn_fires_along_x_and_nose_down_fires_downward() {
    let mut east = Shot::fire([0.0, 0.0, 0.0], 0x4000, 0);
    advance(&mut east, 0.1);
    assert!(east.position[0] > 17.0 && east.position[2].abs() < 1e-2);
    // Positive pitch is nose down, as the demo recorded.
    let mut down = Shot::fire([0.0, 0.0, 0.0], 0, 0x2000);
    advance(&mut down, 0.1);
    assert!(down.position[1] < -10.0, "{:?}", down.position);
}

#[test]
fn a_shot_hits_the_nearest_thing_in_its_path_not_the_first_in_the_list() {
    let targets = [placed(0, 60, 4 << 16), placed(0, 20, 4 << 16), placed(50, 20, 4 << 16)];
    let shot = Shot::fire([0.0, 0.0, 0.0], 0, 0);
    // One step long enough to pass both targets on the z axis.
    let hit = first_hit(&shot, 1.0, &targets, |_| true);
    assert_eq!(hit, Some(1), "the nearer of the two in line");
    // With that one gone, the far one is next.
    let hit = first_hit(&shot, 1.0, &targets, |i| i != 1);
    assert_eq!(hit, Some(0));
}

#[test]
fn a_shot_that_misses_hits_nothing() {
    let targets = [placed(30, 30, 2 << 16)];
    let shot = Shot::fire([0.0, 0.0, 0.0], 0, 0);
    assert_eq!(first_hit(&shot, 1.0, &targets, |_| true), None);
}

#[test]
fn hit_points_come_off_until_the_object_is_destroyed() {
    // 2.5 hit points, a laser multiplier of 1.0: three hits.
    let mut health = Health::for_kind(&kind(2 * 65536 + 32768, 65536, "cube.bin"));
    assert!(!health.hit());
    assert!(!health.hit());
    assert!(health.hit(), "the third hit destroys it");
    assert!(!health.hit(), "it cannot be destroyed twice");
}

#[test]
fn a_laser_multiplier_scales_the_damage() {
    // Half damage from lasers: 1.0 hit point takes two hits.
    let mut health = Health::for_kind(&kind(65536, 32768, "cube.bin"));
    assert!(!health.hit());
    assert!(health.hit());
}

#[test]
fn cube_means_nothing_is_left_and_anything_else_is_a_wreck() {
    assert_eq!(wreck_of(&kind(65536, 65536, "cube.bin")), Wreck::Gone);
    assert_eq!(wreck_of(&kind(65536, 65536, "CUBE.BIN")), Wreck::Gone);
    assert_eq!(
        wreck_of(&kind(65536, 65536, "wbnkruin.bin")),
        Wreck::Model("wbnkruin.bin".into())
    );
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

    // Every placed object's type yields positive hit points, so every one of
    // them can in principle be destroyed.
    for p in &placed {
        assert!(Health::for_kind(&kinds[p.kind]).hit_points > 0.0);
    }

    // Take the first placement, stand ten units off along -z, fire along +z
    // until it goes.
    let target = placed[0];
    let (tx, ty, tz) = (
        target.x as f32 / 65536.0,
        target.y as f32 / 65536.0,
        target.z as f32 / 65536.0,
    );
    let mut health = Health::for_kind(&kinds[target.kind]);
    let mut shots_fired = 0;
    while !health.destroyed && shots_fired < 100 {
        let mut shot = Shot::fire([tx, ty, tz - 10.0], 0, 0);
        shots_fired += 1;
        for _ in 0..100 {
            if let Some(i) = first_hit(&shot, 0.01, &[target], |_| true) {
                assert_eq!(i, 0);
                health.hit();
                break;
            }
            advance(&mut shot, 0.01);
        }
    }
    assert!(health.destroyed, "not destroyed after {shots_fired} shots");
    // Its hit points in 16.16 decide how many: at least one, and not absurd.
    assert!((1..=20).contains(&shots_fired), "{shots_fired} shots");
}
