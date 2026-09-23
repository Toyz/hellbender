//! The player's weapons and energy.

use hb_formats::vector;
use hb_sim::combat::{self, Side};
use hb_sim::powerup::Stores;
use hb_sim::turret::Rng;
use hb_sim::weapons::{self, Guns, Pose, ROWS};

fn level() -> Pose {
    Pose {
        position: [0.0, 10.0, 0.0],
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        forward: [0.0, 0.0, 1.0],
        pitch: 0.0,
        heading: 0.0,
        speed: 16.0,
    }
}

#[test]
fn the_table_is_the_engines() {
    // Speed and damage agree with the rows combat already used.
    for (i, &(speed, damage)) in combat::WEAPONS.iter().enumerate() {
        assert_eq!((ROWS[i].speed, ROWS[i].damage), (speed, damage), "row {i}");
    }
    let v = ROWS[weapons::VALKYRIE];
    assert_eq!((v.code, v.name, v.speed >> 16, v.damage, v.rate), ("VAL", "Valkyrie Cannon", 128, 8192, 6));
    assert_eq!(v.sound, "m-gun-r.wav");
    // The model name sits before its row: the bosses' weapons are wboss1-8.
    assert_eq!(ROWS[9].model, "wboss1.bin");
    assert_eq!(ROWS[16].model, "wboss8.bin");
    assert_eq!(ROWS[24].model, "cruise5.bin");
    assert_eq!(weapons::START, weapons::VALKYRIE);
}

#[test]
fn the_trigger_fires_at_once_and_then_at_the_weapons_rate() {
    let (mut guns, mut stores, mut rng) = (Guns::default(), Stores::default(), Rng::new(1));
    let dt = 1.0 / 60.0;
    let mut volleys = 0;
    let (first, _, _) = guns.step(true, false, dt, &level(), &mut stores, &mut rng);
    assert_eq!(first.len(), 1, "a press fires on its first frame");
    volleys += first.len();
    for _ in 1..600 {
        volleys += guns.step(true, false, dt, &level(), &mut stores, &mut rng).0.len();
    }
    // Six a second for ten seconds.
    assert!((59..=61).contains(&volleys), "{volleys}");
    // Let go, then press again: at once.
    guns.step(false, false, dt, &level(), &mut stores, &mut rng);
    assert_eq!(guns.step(true, false, dt, &level(), &mut stores, &mut rng).0.len(), 1);
}

#[test]
fn barrels_follow_weapon_energy_and_cost_it() {
    let (mut guns, mut stores, mut rng) = (Guns::default(), Stores::default(), Rng::new(1));
    let dt = 1.0 / 60.0;
    let fire = |guns: &mut Guns, stores: &mut Stores, rng: &mut Rng| {
        guns.step(false, false, dt, &level(), stores, rng);
        guns.step(true, false, dt, &level(), stores, rng).0.remove(0)
    };

    // Half: two barrels, a 256th a volley.
    assert_eq!(stores.weapon_energy, 0.5);
    let v = fire(&mut guns, &mut stores, &mut rng);
    assert_eq!(v.shots.len(), 2);
    assert!((stores.weapon_energy - (0.5 - 1.0 / 256.0)).abs() < 1e-6);
    assert_eq!(v.sound, "m-gun-r.wav");
    let s = v.shots[0];
    assert_eq!(s.side, Side::Player);
    assert_eq!(s.kind, weapons::VALKYRIE as i32);
    // Along the nose at 128 plus the ship's 16.
    assert!((s.velocity[2] - 144.0).abs() < 1e-3, "{:?}", s.velocity);
    // Half a unit down, half a unit either side, a frame's flight back.
    let xs: Vec<f32> = v.shots.iter().map(|s| s.position[0]).collect();
    assert_eq!(xs, [0.5, -0.5]);
    assert!((s.position[1] - 9.5).abs() < 1e-6);
    assert!((s.position[2] + 144.0 * dt).abs() < 1e-4);

    // Above half: four, two 256ths.
    stores.weapon_energy = 0.75;
    let v = fire(&mut guns, &mut stores, &mut rng);
    assert_eq!(v.shots.len(), 4);
    assert!((stores.weapon_energy - (0.75 - 2.0 / 256.0)).abs() < 1e-6);

    // Empty: one barrel, side to side, free.
    stores.weapon_energy = 0.0;
    let a = fire(&mut guns, &mut stores, &mut rng);
    let b = fire(&mut guns, &mut stores, &mut rng);
    assert_eq!((a.shots.len(), b.shots.len()), (1, 1));
    assert_eq!(a.shots[0].position[0], -b.shots[0].position[0]);
    assert_eq!(a.shots[0].position[0].abs(), 0.75);
    assert_eq!(stores.weapon_energy, 0.0);
}

#[test]
fn the_dispersion_cannon_also_fires_straight_back() {
    let (mut guns, mut stores, mut rng) = (Guns::default(), Stores::default(), Rng::new(1));
    stores.ammo[weapons::DISPERSION] = 10;
    assert_eq!(guns.select(weapons::DISPERSION, &stores), None);
    stores.weapon_energy = 0.0;
    let v = guns.step(true, false, 1.0 / 60.0, &level(), &mut stores, &mut rng).0.remove(0);
    assert_eq!(v.shots.len(), 2);
    let [a, b] = [v.shots[0].velocity, v.shots[1].velocity];
    let dot = vector::dot(a, b) / vector::dot(a, a);
    assert!((dot + 1.0).abs() < 1e-4, "{dot}");
    // The first step of the pattern is 1,024 off in pitch: about 5.6 degrees.
    let up = -a[1] / vector::length(a);
    assert!((up.asin().to_degrees().abs() - 5.625).abs() < 0.01, "{up}");
    assert_eq!(stores.ammo[weapons::DISPERSION], 9);

    // Two barrels: four ahead, two behind; four: nine ahead, two behind.
    stores.weapon_energy = 0.5;
    guns.step(false, false, 0.0, &level(), &mut stores, &mut rng);
    let v = guns.step(true, false, 1.0 / 60.0, &level(), &mut stores, &mut rng).0.remove(0);
    let ahead = v.shots.iter().filter(|s| s.velocity[2] > 0.0).count();
    assert_eq!((ahead, v.shots.len()), (4, 6));
    stores.weapon_energy = 0.75;
    guns.step(false, false, 0.0, &level(), &mut stores, &mut rng);
    let v = guns.step(true, false, 1.0 / 60.0, &level(), &mut stores, &mut rng).0.remove(0);
    let ahead = v.shots.iter().filter(|s| s.velocity[2] > 0.0).count();
    assert_eq!((ahead, v.shots.len()), (9, 11));
}

#[test]
fn a_weapon_runs_out_and_the_next_one_is_taken() {
    let (mut guns, mut stores, mut rng) = (Guns::default(), Stores::default(), Rng::new(1));
    // Not there: a line, and the selection stays.
    assert_eq!(guns.select(weapons::RAPID_FIRE, &stores), Some(weapons::NO_RAPID_FIRE));
    assert_eq!(guns.selected, weapons::VALKYRIE);
    stores.ammo[weapons::RAPID_FIRE] = 2;
    guns.select(weapons::RAPID_FIRE, &stores);
    for _ in 0..2 {
        guns.step(false, false, 0.0, &level(), &mut stores, &mut rng);
        guns.step(true, false, 1.0 / 60.0, &level(), &mut stores, &mut rng);
    }
    assert_eq!(stores.ammo[weapons::RAPID_FIRE], 0);
    // The next with a stock after 3 is the Dead-On's 20.
    assert_eq!(guns.selected, weapons::DEAD_ON);
    // The next-weapon key goes round the ones with a stock, in slot order.
    stores.ammo[weapons::DISPERSION] = 5;
    let mut order = Vec::new();
    for _ in 0..6 {
        guns.next(&stores);
        order.push(guns.selected);
    }
    use weapons::*;
    assert_eq!(order, [VIPER, VALKYRIE, CRUISE, SERVO_KINETIC, DISPERSION, DEAD_ON]);
}

#[test]
fn energy_moves_an_eighth_at_a_press_and_overflow_goes_back() {
    let mut stores = Stores::default();
    weapons::transfer_to_weapons(&mut stores);
    assert_eq!((stores.energy, stores.weapon_energy), (0.375, 0.625));
    stores.energy = 0.0625;
    weapons::transfer_to_weapons(&mut stores);
    assert_eq!((stores.energy, stores.weapon_energy), (0.0, 0.6875));
    stores.energy = 0.5;
    stores.weapon_energy = 0.95;
    weapons::transfer_to_weapons(&mut stores);
    assert_eq!(stores.weapon_energy, 1.0);
    assert!((stores.energy - 0.45).abs() < 1e-6, "{}", stores.energy);
    let mut shield = 0.5;
    weapons::transfer_to_shields(&mut stores, &mut shield);
    assert_eq!(shield, 0.625);
}

#[test]
fn the_afterburner_burns_a_tank_in_sixteen_seconds_and_refills_from_weapon_energy() {
    let (mut guns, mut stores, mut rng) = (Guns::default(), Stores::default(), Rng::new(1));
    let dt = 1.0 / 60.0;
    let mut lit_for = 0.0;
    for _ in 0..(20 * 60) {
        if guns.step(false, true, dt, &level(), &mut stores, &mut rng).2 {
            lit_for += dt;
        }
    }
    assert!((lit_for - 16.0).abs() < 0.05, "{lit_for}");
    assert_eq!(stores.fuel, 0.0);

    // Empty: five seconds, then a thirty-second; after that it fills from
    // weapon energy at a thirty-second a second.
    let (mut hull, mut empty_for) = (0.5, 0.0);
    for _ in 0..(5 * 60 + 1) {
        weapons::regenerate(&mut stores, &mut hull, dt, &mut empty_for);
    }
    // (Within a frame's refill: the five seconds are summed in frames.)
    assert!((stores.fuel - 1.0 / 32.0).abs() < 1.0 / 32.0 / 60.0 + 1e-6, "{}", stores.fuel);
    let before = stores.weapon_energy;
    for _ in 0..60 {
        weapons::regenerate(&mut stores, &mut hull, dt, &mut empty_for);
    }
    assert!((stores.fuel - 2.0 / 32.0).abs() < 2e-3, "{}", stores.fuel);
    assert!((before - stores.weapon_energy - 0xda as f32 / 65536.0).abs() < 1e-5);
    // The hull creeps up by 0x48 a second.
    assert!((hull - (0.5 + 0x48 as f32 / 65536.0 * (6.0 + 1.0 / 60.0))).abs() < 1e-5, "{hull}");
}

fn candidate(class: i64, view: [f32; 3]) -> weapons::Candidate {
    weapons::Candidate { class, friendly: false, alive: true, near: true, view }
}

#[test]
fn missiles_lock_by_what_flies_and_what_is_on_screen() {
    let ahead = [0.0, 0.0, 20.0];
    // Class 7 flies (a flyer), class 10 does not (a turret).
    assert!(candidate(7, ahead).lockable(weapons::VIPER));
    assert!(!candidate(10, ahead).lockable(weapons::VIPER));
    assert!(candidate(10, ahead).lockable(weapons::CRUISE));
    assert!(!candidate(7, ahead).lockable(weapons::CRUISE));
    // The guns lock nothing, nor does the Dead-On.
    assert!(!candidate(7, ahead).lockable(weapons::VALKYRIE));
    assert!(!candidate(7, ahead).lockable(weapons::DEAD_ON));
    // Inside 90 degrees each way and in front, or not at all.
    assert!(candidate(7, [19.0, -19.0, 20.0]).lockable(weapons::VIPER));
    assert!(!candidate(7, [21.0, 0.0, 20.0]).lockable(weapons::VIPER));
    assert!(!candidate(7, [0.0, 0.0, -5.0]).lockable(weapons::VIPER));
    let mut friend = candidate(7, ahead);
    friend.friendly = true;
    assert!(!friend.lockable(weapons::VIPER));
    let mut far = candidate(7, ahead);
    far.near = false;
    assert!(!far.lockable(weapons::VIPER));
    assert!(!candidate(33, ahead).lockable(30));
}

#[test]
fn the_lock_takes_the_first_it_can_steps_on_a_press_and_lets_go() {
    let mut guns = Guns::default();
    let can = [false, true, false, true, true];
    guns.track(false, 5, |i| can[i]);
    assert_eq!(guns.lock, Some(1));
    guns.track(true, 5, |i| can[i]);
    assert_eq!(guns.lock, Some(3));
    guns.track(true, 5, |i| can[i]);
    assert_eq!(guns.lock, Some(4));
    guns.track(true, 5, |i| can[i]);
    assert_eq!(guns.lock, Some(1), "round the list");
    // The locked one can no longer be held: dropped at the end of the frame
    // (`0x47b9c5`), and the next frame takes the first that can be.
    let gone = [false, false, false, true, true];
    guns.track(false, 5, |i| gone[i]);
    assert_eq!(guns.lock, None);
    guns.track(false, 5, |i| gone[i]);
    assert_eq!(guns.lock, Some(3));
    guns.track(false, 5, |_| false);
    assert_eq!(guns.lock, None);
}

#[test]
fn missiles_leave_from_under_either_wing_and_home_on_the_lock() {
    let (mut guns, mut stores, mut rng) = (Guns::default(), Stores::default(), Rng::new(1));
    assert_eq!(guns.select(weapons::VIPER, &stores), None);
    guns.lock = Some(7);
    let fire = |guns: &mut Guns, stores: &mut Stores, rng: &mut Rng| {
        guns.step(false, false, 0.0, &level(), stores, rng);
        guns.step(true, false, 1.0 / 60.0, &level(), stores, rng).0.remove(0)
    };
    let a = fire(&mut guns, &mut stores, &mut rng).missiles[0];
    let b = fire(&mut guns, &mut stores, &mut rng).missiles[0];
    assert_eq!(a.position, [1.0, 9.0, 0.5]);
    assert_eq!(b.position, [-1.0, 9.0, 0.5]);
    assert_eq!((a.target, a.side, a.kind, a.damage, a.speed), (Some(7), Side::Player, 19, 1.0, 16.0));
    assert_eq!(stores.ammo[weapons::VIPER], 3);

    // It turns to a target off to the side and strikes it.
    let mut m = a;
    let goal = [40.0, 9.0, 30.0];
    let hit = |p: [f32; 3]| {
        let d = [p[0] - goal[0], p[1] - goal[1], p[2] - goal[2]];
        (d.iter().map(|v| v * v).sum::<f32>() < 4.0).then_some(7)
    };
    let mut struck = None;
    for _ in 0..(6 * 60) {
        struck = m.step_at(1.0 / 60.0, |i| (i == 7).then_some(goal), hit, |_| false);
        if struck.is_some() {
            break;
        }
    }
    assert_eq!(struck, Some(hb_sim::turret::Struck::Object(7)));

    // A Dead-On has no target and flies on along the nose.
    guns.select(weapons::DEAD_ON, &stores);
    let mut d = fire(&mut guns, &mut stores, &mut rng).missiles[0];
    assert_eq!(d.target, None);
    for _ in 0..60 {
        d.step_at(1.0 / 60.0, |_| Some(goal), |_| None, |_| false);
    }
    assert!(d.position[0].abs() == 1.0 && d.position[2] > 16.0, "{:?}", d.position);
    // The cruise missile flies ten seconds.
    guns.select(weapons::CRUISE, &stores);
    assert_eq!(fire(&mut guns, &mut stores, &mut rng).missiles[0].life, 10.0);
}

#[test]
fn a_mirv_breaks_into_ten_a_second_in() {
    let (mut guns, mut stores, mut rng) = (Guns::default(), Stores::default(), Rng::new(5));
    stores.ammo[weapons::MIRV] = 3;
    guns.select(weapons::MIRV, &stores);
    let mut m = guns.step(true, false, 1.0 / 60.0, &level(), &mut stores, &mut rng).0.remove(0).missiles[0];
    assert_eq!(m.target, None, "a MIRV is launched unguided");
    assert!(!m.splitting());
    // Fly it a second.
    for _ in 0..61 {
        m.step_at(1.0 / 60.0, |_| None, |_| None, |_| false);
    }
    assert!(m.splitting(), "{}", m.age);
    let children = m.split(&mut rng, &[]);
    assert_eq!(children.len(), hb_sim::turret::SPLIT_INTO);
    for c in &children {
        assert_eq!(c.kind, weapons::DEAD_ON as i32);
        assert_eq!(c.target, None);
        assert_eq!(c.position, m.position);
        assert!((c.speed - m.speed / 2.0).abs() < 1e-3);
        assert!((-16384.0..16384.0).contains(&c.pitch), "{}", c.pitch);
    }
    // They do not all go the same way.
    let spread = children.windows(2).filter(|w| w[0].heading != w[1].heading).count();
    assert!(spread >= children.len() - 2, "{spread}");
}

#[test]
fn a_guided_mirv_gives_each_of_its_ten_a_target() {
    let (mut guns, mut stores, mut rng) = (Guns::default(), Stores::default(), Rng::new(5));
    stores.ammo[weapons::GUIDED_MIRV] = 1;
    guns.select(weapons::GUIDED_MIRV, &stores);
    guns.lock = Some(4);
    let m = guns.step(true, false, 1.0 / 60.0, &level(), &mut stores, &mut rng).0.remove(0).missiles[0];
    assert_eq!(m.target, Some(4), "it is launched at the lock");
    let children = m.split(&mut rng, &[7, 8, 9]);
    assert_eq!(children.len(), hb_sim::turret::SPLIT_INTO);
    for c in &children {
        assert_eq!(c.kind, weapons::VIPER as i32);
        assert!([7, 8, 9].contains(&c.target.unwrap()), "{:?}", c.target);
    }
    // With nothing to go at, they fly on unguided.
    let children = m.split(&mut rng, &[]);
    assert!(children.iter().all(|c| c.target.is_none()));
}

#[test]
fn the_super_weapon_is_what_the_eight_pieces_make() {
    let (mut guns, mut stores, mut rng) = (Guns::default(), Stores::default(), Rng::new(2));
    // Not there until the pieces are.
    assert_eq!(guns.select(weapons::SUPER, &stores), None);
    assert_eq!(guns.selected, weapons::VALKYRIE);
    let mut pilot = hb_sim::combat::Pilot::default();
    let mut said = Vec::new();
    for kind in 23..=30 {
        hb_sim::powerup::collect(kind, &mut pilot, &mut stores, &mut said);
    }
    assert_eq!(stores.weapon, weapons::SUPER);
    assert_eq!(stores.ammo[weapons::SUPER], -1, "unlimited");
    assert_eq!(guns.select(weapons::SUPER, &stores), None);
    assert_eq!(guns.selected, weapons::SUPER);
    guns.lock = Some(2);
    let v = guns.step(true, false, 1.0 / 60.0, &level(), &mut stores, &mut rng).0.remove(0);
    let m = v.missiles[0];
    assert_eq!(m.kind, hb_sim::turret::SUPER);
    assert_eq!(m.target, Some(2), "it goes at the lock");
    assert_eq!(m.damage, 1.0);
    // Its reach is sixteen units, half the MIRV's blast.
    assert_eq!(hb_sim::turret::SUPER_REACH, 16.0);
    assert!(hb_sim::turret::SUPER_REACH < hb_sim::turret::SPLIT_REACH);
}

/// The cluster fires two at once, one out to each side, and spends one round
/// for the pair (`0x47cec0` calls the launcher twice; worklog 65).
#[test]
fn the_cluster_fires_a_pair() {
    let mut guns = Guns::default();
    let mut stores = Stores::default();
    stores.ammo[weapons::CLUSTER] = 4;
    guns.select(weapons::CLUSTER, &stores);
    let mut rng = Rng::new(1);
    let pose = level();

    let (volleys, _, _) = guns.step(true, false, 1.0, &pose, &mut stores, &mut rng);
    let fired: Vec<_> = volleys.iter().flat_map(|v| v.missiles.iter()).collect();
    assert_eq!(fired.len(), 2, "two missiles for one trigger");
    assert_eq!(stores.ammo[weapons::CLUSTER], 3, "and one round for the pair");

    // One each side of the nose, and both the same height.
    let (left, right) = (fired[0].position, fired[1].position);
    assert!((left[0] + right[0] - 2.0 * pose.position[0]).abs() < 1e-3, "{left:?} {right:?}");
    assert!((left[0] - right[0]).abs() > 0.5, "they leave from the same place");
    assert_eq!(left[1], right[1]);
    assert!(fired.iter().all(|m| m.kind == weapons::CLUSTER as i32));
}

/// The previous-weapon key walks a ring written out by hand, which is not
/// the reverse of the search the next-weapon key does.
#[test]
fn the_previous_weapon_key_walks_the_ring() {
    use hb_sim::weapons::{DISPERSION, RAPID_FIRE, SERVO_KINETIC, VALKYRIE};
    let mut stores = Stores::default();
    for slot in stores.ammo.iter_mut() {
        *slot = -1;
    }
    let mut guns = Guns::default();
    guns.selected = SERVO_KINETIC;
    guns.previous(&stores);
    assert_eq!(guns.selected, DISPERSION);
    guns.previous(&stores);
    assert_eq!(guns.selected, VALKYRIE);
    // And round to the start again: the twelfth is the rapid-fire laser and
    // the one before the servo-kinetic.
    guns.selected = RAPID_FIRE;
    guns.previous(&stores);
    assert_eq!(guns.selected, SERVO_KINETIC);
}

/// A weapon with nothing in it is stepped over, the way an empty one is on
/// the way forward.
#[test]
fn the_previous_weapon_key_steps_over_an_empty_one() {
    use hb_sim::weapons::{DISPERSION, SERVO_KINETIC, VALKYRIE};
    let mut stores = Stores::default();
    for slot in stores.ammo.iter_mut() {
        *slot = -1;
    }
    stores.ammo[DISPERSION] = 0;
    let mut guns = Guns::default();
    guns.selected = SERVO_KINETIC;
    guns.previous(&stores);
    assert_eq!(guns.selected, VALKYRIE, "the dispersion cannon is empty");
}
