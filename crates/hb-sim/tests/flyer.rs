//! The flying enemies' decisions, with the port's own steering.

use hb_formats::text::{EnemyDef, Placement, SecondWeapon};
use hb_sim::combat::{step_shot, Pilot, Stop};
use hb_sim::flyer::{Flyer, Target};
use hb_sim::turret::{Launch, Rng};

fn hornet() -> EnemyDef {
    // HOTH's hmyhornt.bin, class 53: 18.3 units a second, turn 1.5, a shot
    // every second for 1/16, attack at 42 and retreat at 14.
    EnemyDef {
        fields: [53, 0, 5 << 16, 0, 0, 0],
        move_rate: 1_200_000,
        turn_rate: 100_000,
        fire_interval: 65536,
        shot_damage: 4096,
        weapon: 0,
        attack_retreat: [42, 14, 0, 0],
        second_weapon: SecondWeapon { shot_speed: 2_000_000, ..SecondWeapon::default() },
        ..EnemyDef::default()
    }
}

fn player_at(position: [f32; 3]) -> Target {
    Target { position, right: [1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0], forward: [0.0, 0.0, 1.0], speed: 0.0 }
}

#[test]
fn a_flyer_makes_passes_and_shoots_on_the_way_in() {
    let def = hornet();
    let place = Placement { kind: 0, hit_points: 4096, x: 0, y: 20 << 16, z: -70 << 16, pitch: 0, roll: 0, heading: 0 };
    let mut flyer = Flyer::new(&place);
    let player = player_at([0.0, 20.0, 0.0]);
    let mut rng = Rng::new(5);
    let (mut phases, mut shots, mut closest, mut pilot) = (Vec::new(), Vec::new(), f32::MAX, Pilot::default());
    let dt = 1.0 / 30.0;
    for _ in 0..(40.0 / dt) as usize {
        if let Some(Launch::Shot(s)) = flyer.step(&def, None, &player, dt, |_, _| 0.0, &mut rng) {
            shots.push(s);
        }
        shots.retain_mut(|s| match step_shot(s, dt, &[], &[], |_| false, player.position, |_| false) {
            Some(Stop::Player) => {
                pilot.take(s.damage);
                false
            }
            Some(_) => false,
            None => s.alive(),
        });
        if phases.last() != Some(&flyer.phase) {
            phases.push(flyer.phase);
        }
        let d = (0..3).map(|k| (flyer.position[k] - player.position[k]).powi(2)).sum::<f32>().sqrt();
        closest = closest.min(d);
        assert!(flyer.position[1] > 0.0, "flew into the ground");
    }
    // In, a pass, out, and round again - more than once in 40 seconds.
    let passes = phases.windows(2).filter(|w| w[0] == 200 && (w[1] == 2000 || w[1] == 201)).count();
    assert!(passes >= 2, "{phases:?}");
    assert!(closest < 15.0, "never closer than {closest}");
    assert!(pilot.health < 1.0, "never hit: {pilot:?}");
}

#[test]
fn it_breaks_away_when_the_player_is_on_its_tail() {
    let def = hornet();
    // Both heading +z, the player 20 units behind and aimed at it.
    let place = Placement { kind: 0, hit_points: 4096, x: 0, y: 20 << 16, z: 20 << 16, pitch: 0, roll: 0, heading: 0 };
    let mut flyer = Flyer::new(&place);
    let mut player = player_at([0.0, 20.0, 0.0]);
    player.speed = 16.0;
    let mut rng = Rng::new(1);
    let dt = 1.0 / 30.0;
    flyer.step(&def, None, &player, dt, |_, _| 0.0, &mut rng);
    flyer.step(&def, None, &player, dt, |_, _| 0.0, &mut rng);
    // Level, so it breaks upward: 2011 sets a point above the player and
    // flies to it.
    assert_eq!(flyer.phase, 2012, "phase {}", flyer.phase);
    assert!((flyer.break_point[1] - 36.0).abs() < 1e-3, "{:?}", flyer.break_point);
}

