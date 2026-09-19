//! The player's weapons and energy.

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
    let dot = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]);
    assert!((dot + 1.0).abs() < 1e-4, "{dot}");
    // The first step of the pattern is 1,024 off in pitch: about 5.6 degrees.
    let up = -a[1] / (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
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
    assert_eq!(guns.selected, weapons::VALKYRIE);
    // The next-weapon key goes round the ones with a stock.
    stores.ammo[weapons::DISPERSION] = 5;
    guns.next(&stores);
    assert_eq!(guns.selected, weapons::SERVO_KINETIC);
    guns.next(&stores);
    assert_eq!(guns.selected, weapons::DISPERSION);
    guns.next(&stores);
    assert_eq!(guns.selected, weapons::VALKYRIE);
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
