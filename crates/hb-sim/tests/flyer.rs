//! The flying enemies' decisions, with the port's own steering.

use hb_formats::fixed::to_units;
use hb_formats::text::{EnemyDef, Placement, SecondWeapon};
use hb_sim::combat::{step_shot, Pilot, Stop};
use hb_sim::flyer::{Flyer, Target};
use hb_sim::steer::Flat;
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
    Target {
        position,
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        forward: [0.0, 0.0, 1.0],
        speed: 0.0,
        velocity: [0.0; 3],
        alive: true,
    }
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
        if let Some(Launch::Shot(s)) = flyer.step(&def, None, &player, dt, &Flat(0.0), &mut rng) {
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
        let d = (0..3).map(|k| (flyer.body.position[k] - player.position[k]).powi(2)).sum::<f32>().sqrt();
        closest = closest.min(d);
        assert!(flyer.body.position[1] > 0.0, "flew into the ground");
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
    flyer.step(&def, None, &player, dt, &Flat(0.0), &mut rng);
    flyer.step(&def, None, &player, dt, &Flat(0.0), &mut rng);
    // Level, so it breaks upward: 2011 sets a point above the player and
    // flies to it.
    assert_eq!(flyer.phase, 2012, "phase {}", flyer.phase);
    assert!((flyer.break_point[1] - 36.0).abs() < 1e-3, "{:?}", flyer.break_point);
}


/// HOTH's `hmship5.bin`, the Mine Layer: class 55.
fn layer() -> EnemyDef {
    EnemyDef { fields: [55, 0, 5 << 16, 0, 0, 0], ..hornet() }
}

/// A mine layer flies to where the player is about to be and leaves one
/// there, rather than shooting its way in.
#[test]
fn a_mine_layer_drops_one_in_front_of_the_player() {
    let def = layer();
    let place = Placement { kind: 0, hit_points: 65536, x: 0, y: 0, z: 60 << 16, heading: 0x8000, pitch: 0, roll: 0 };
    let mut flyer = Flyer::layer(&place);
    // The player is flying along +z, well clear, with the layer ahead of him
    // and off his nose.
    let player = player_at([6.0, 0.0, 0.0]);
    let mut rng = Rng::new(7);
    let ground = &Flat(-100.0);

    let mut laid = None;
    for _ in 0..600 {
        if let Some(Launch::Mine { at, radius, damage }) =
            flyer.step(&def, None, &player, 1.0 / 30.0, ground, &mut rng)
        {
            laid = Some((at, radius, damage));
            break;
        }
    }
    let Some((at, radius, damage)) = laid else {
        panic!("no mine in twenty seconds, phase {}", flyer.phase);
    };
    // Its reach is the type's retreat range and its bite the type's shot
    // damage.
    assert_eq!(radius, 14.0);
    assert!((damage - to_units(4096)).abs() < 1e-6, "{damage}");
    // And it went down clear of the player, not on top of him.
    assert!(
        hb_sim::mine::laid::Field::clear_of(at, player.position),
        "laid at {at:?}, player at {:?}",
        player.position
    );
}

/// A plain fighter has no such phase, whatever the situation.
#[test]
fn a_fighter_lays_nothing() {
    let def = hornet();
    let place = Placement { kind: 0, hit_points: 65536, x: 0, y: 0, z: 60 << 16, heading: 0x8000, pitch: 0, roll: 0 };
    let mut flyer = Flyer::new(&place);
    let player = player_at([6.0, 0.0, 0.0]);
    let mut rng = Rng::new(7);
    for _ in 0..600 {
        if let Some(Launch::Mine { .. }) =
            flyer.step(&def, None, &player, 1.0 / 30.0, &Flat(-100.0), &mut rng)
        {
            panic!("a class 53 laid a mine");
        }
    }
}

/// MORBOS's `mtwship.bin`, the SPINE 17 hover craft: class 58.
fn spine() -> EnemyDef {
    EnemyDef { fields: [58, 0, 5 << 16, 0, 0, 0], ..hornet() }
}

/// It sits on its post until the player is within the attack range, and it
/// does not chase him past that range from home.
#[test]
fn a_hover_craft_holds_its_post_and_comes_back_to_it() {
    use hb_sim::flyer::Hover;
    let def = spine();
    let place = Placement { kind: 0, hit_points: 65536, x: 0, y: 0, z: 0, heading: 0, pitch: 0, roll: 0 };
    let mut hover = Hover::new(&place);
    let mut rng = Rng::new(3);
    let ground = &Flat(-100.0);
    let post = hover.body.position;

    // Well outside the attack range of 42: it stays put.
    let far = player_at([0.0, 0.0, 200.0]);
    for _ in 0..90 {
        hover.step(&def, None, &far, 1.0 / 30.0, ground, &mut rng);
    }
    assert_eq!(hover.phase, 2006, "still on station");
    let moved = (hover.body.position[2] - post[2]).abs();
    assert!(moved < 0.01, "it moved {moved} off its post");

    // Inside it, it comes for him.
    let near = player_at([0.0, 0.0, 30.0]);
    for _ in 0..30 {
        hover.step(&def, None, &near, 1.0 / 30.0, ground, &mut rng);
    }
    assert_ne!(hover.phase, 2006, "it should have left the post");

    // Lead it far enough away and it gives up and goes home.
    let bait = player_at([0.0, 0.0, 400.0]);
    for _ in 0..3_000 {
        hover.step(&def, None, &bait, 1.0 / 30.0, ground, &mut rng);
        if hover.phase == 2006 {
            break;
        }
    }
    assert_eq!(hover.phase, 2006, "it should be back on station");
    let back = (hover.body.position[2] - post[2]).abs();
    assert!(back < hb_sim::flyer::HOME, "it stopped {back} from its post");
}

/// Off its tether it stops fighting: with the player behind it, it turns for
/// home whatever it was doing.
#[test]
fn a_hover_craft_off_its_tether_turns_for_home() {
    use hb_sim::flyer::Hover;
    let def = spine();
    let place = Placement { kind: 0, hit_points: 65536, x: 0, y: 0, z: 0, heading: 0, pitch: 0, roll: 0 };
    let mut hover = Hover::new(&place);
    // Put it a long way from its post, chasing.
    hover.body.position = [0.0, 0.0, 300.0];
    hover.phase = 200;
    // The player behind it: his nose points away from it.
    let mut player = player_at([0.0, 0.0, 340.0]);
    player.forward = [0.0, 0.0, 1.0];
    let mut rng = Rng::new(3);
    hover.step(&def, None, &player, 1.0 / 30.0, &Flat(-100.0), &mut rng);
    assert_eq!(hover.phase, 201, "it should be heading home");
}
