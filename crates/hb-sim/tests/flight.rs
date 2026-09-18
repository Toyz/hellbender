//! The player's flight model against the numbers read from the engine and the
//! recorded demo.

use hb_sim::flight::{Controls, Ship};

const FPS: f32 = 30.0;

fn fly(ship: &mut Ship, controls: Controls, seconds: f32) {
    for _ in 0..(seconds * FPS) as usize {
        ship.step(&controls, 1.0 / FPS);
    }
}

fn s16(angle: f32) -> f32 {
    (angle + 32768.0).rem_euclid(65536.0) - 32768.0
}

#[test]
fn full_throttle_settles_at_sixteen_units_a_second() {
    let mut ship = Ship::new([0.0; 3], 0.0);
    fly(&mut ship, Controls { throttle_up: true, ..Controls::default() }, 2.0);
    assert_eq!(ship.throttle, 1.0);
    fly(&mut ship, Controls::default(), 1.0);
    assert!((ship.speed() - 16.0).abs() < 0.01, "{}", ship.speed());
    // Straight along +z at heading 0.
    let before = ship.position;
    fly(&mut ship, Controls::default(), 1.0);
    assert!((ship.position[2] - before[2] - 16.0).abs() < 0.05);
    assert!(ship.position[0].abs() < 1e-3 && ship.position[1].abs() < 1e-3);
}

#[test]
fn the_afterburner_triples_it_and_half_throttle_halves_it() {
    let mut ship = Ship::new([0.0; 3], 0.0);
    fly(&mut ship, Controls { afterburner: true, ..Controls::default() }, 1.0);
    assert!((ship.speed() - 48.0).abs() < 0.01);
    let mut ship = Ship::new([0.0; 3], 0.0);
    ship.throttle = 0.5;
    fly(&mut ship, Controls::default(), 1.0);
    assert!((ship.speed() - 8.0).abs() < 0.01);
}

#[test]
fn the_speed_does_not_depend_on_the_frame_rate() {
    for fps in [15.0f32, 30.0, 60.0, 144.0] {
        let mut ship = Ship::new([0.0; 3], 0.0);
        ship.throttle = 1.0;
        for _ in 0..(3.0 * fps) as usize {
            ship.step(&Controls::default(), 1.0 / fps);
        }
        assert!((ship.speed() - 16.0).abs() < 0.01, "{fps} fps: {}", ship.speed());
    }
}

#[test]
fn a_held_key_turns_at_two_sevenths_of_a_turn_a_second() {
    let mut ship = Ship::new([0.0; 3], 0.0);
    ship.auto_level = false;
    // Hold right until the key has ramped and the rate settled.
    fly(&mut ship, Controls { right: true, ..Controls::default() }, 1.0);
    assert!((ship.rates[2] - 2.0 / 7.0 * 9362.0 * 7.0 / 65536.0).abs() < 1e-3, "{:?}", ship.rates);
    // In the engine's units that is 18,724 a second - the demo's 90th
    // percentile heading rate is 18,002.
    assert!((ship.rates[2] * 65536.0 - 18724.0).abs() < 2.0);
}

#[test]
fn right_turns_right_and_banks_right_and_up_dives() {
    let mut ship = Ship::new([0.0; 3], 0.0);
    fly(&mut ship, Controls { right: true, ..Controls::default() }, 0.5);
    let [_, roll, heading] = ship.angles();
    // Heading rises toward +x; the right wing goes down, which is negative
    // roll - as the demo records in its right turns.
    assert!(s16(heading) > 1000.0, "{heading}");
    assert!(s16(roll) < -500.0, "{roll}");

    let mut ship = Ship::new([0.0; 3], 0.0);
    fly(&mut ship, Controls { up: true, ..Controls::default() }, 0.5);
    let [pitch, _, _] = ship.angles();
    // The up key raises the pitch angle, which is nose down.
    assert!(s16(pitch) > 1000.0, "{pitch}");
    assert!(ship.forward[1] < -0.1);
}

#[test]
fn auto_level_brings_the_wings_back() {
    let mut ship = Ship::new([0.0; 3], 0.0);
    fly(&mut ship, Controls { roll_left: true, ..Controls::default() }, 0.6);
    let [_, rolled, _] = ship.angles();
    assert!(s16(rolled) > 3000.0, "{rolled}");
    fly(&mut ship, Controls::default(), 6.0);
    let [_, roll, _] = ship.angles();
    assert!(s16(roll).abs() < 200.0, "still rolled {}", s16(roll));
    // And without it the roll stays.
    let mut ship = Ship::new([0.0; 3], 0.0);
    ship.auto_level = false;
    fly(&mut ship, Controls { roll_left: true, ..Controls::default() }, 0.6);
    fly(&mut ship, Controls::default(), 6.0);
    assert!(s16(ship.angles()[1]) > 3000.0);
}
