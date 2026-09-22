//! The player's flight model against the numbers read from the engine and the
//! recorded demo.

use hb_sim::flight::{Controls, Ship};

const FPS: f32 = 30.0;

fn fly(ship: &mut Ship, controls: Controls, seconds: f32) {
    for _ in 0..(seconds * FPS) as usize {
        ship.step(&controls, 1.0 / FPS);
    }
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
    assert!(hb_formats::fixed::signed(heading) > 1000.0, "{heading}");
    assert!(hb_formats::fixed::signed(roll) < -500.0, "{roll}");

    let mut ship = Ship::new([0.0; 3], 0.0);
    fly(&mut ship, Controls { up: true, ..Controls::default() }, 0.5);
    let [pitch, _, _] = ship.angles();
    // The up key raises the pitch angle, which is nose down.
    assert!(hb_formats::fixed::signed(pitch) > 1000.0, "{pitch}");
    assert!(ship.forward[1] < -0.1);
}

#[test]
fn auto_level_brings_the_wings_back() {
    let mut ship = Ship::new([0.0; 3], 0.0);
    fly(&mut ship, Controls { roll_left: true, ..Controls::default() }, 0.6);
    let [_, rolled, _] = ship.angles();
    assert!(hb_formats::fixed::signed(rolled) > 3000.0, "{rolled}");
    fly(&mut ship, Controls::default(), 6.0);
    let [_, roll, _] = ship.angles();
    assert!(hb_formats::fixed::signed(roll).abs() < 200.0, "still rolled {}", hb_formats::fixed::signed(roll));
    // And without it the roll stays.
    let mut ship = Ship::new([0.0; 3], 0.0);
    ship.auto_level = false;
    fly(&mut ship, Controls { roll_left: true, ..Controls::default() }, 0.6);
    fly(&mut ship, Controls::default(), 6.0);
    assert!(hb_formats::fixed::signed(ship.angles()[1]) > 3000.0);
}

/// A stick held all the way over turns the ship the same amount as the key
/// that has been held long enough to reach the top of its ramp, because the
/// ramp is what a stick would have fed in the first place.
#[test]
fn a_stick_fully_over_turns_like_a_key_already_held() {
    let key = {
        let mut ship = Ship::new([0.0; 3], 0.0);
        // Half a second gets the ramp to 1.0, then a second of turning.
        fly(&mut ship, Controls { right: true, ..Controls::default() }, 1.5);
        hb_formats::fixed::signed(ship.angles()[2])
    };
    let stick = {
        let mut ship = Ship::new([0.0; 3], 0.0);
        fly(&mut ship, Controls { stick: Some([0.0, 1.0, 0.0]), ..Controls::default() }, 1.5);
        hb_formats::fixed::signed(ship.angles()[2])
    };
    // The key spends its first half second climbing; the stick is there from
    // the start, so it is further round, and by that half second's worth.
    assert!(stick > key, "stick {stick}, key {key}");
    assert!(stick - key < key * 0.6, "stick {stick}, key {key}");
}

/// Half over is half the turn, which a key cannot ask for at all.
#[test]
fn a_stick_half_over_turns_half_as_far() {
    let turn = |deflection: f32| {
        let mut ship = Ship::new([0.0; 3], 0.0);
        fly(&mut ship, Controls { stick: Some([0.0, deflection, 0.0]), ..Controls::default() }, 1.0);
        hb_formats::fixed::signed(ship.angles()[2])
    };
    let (half, full) = (turn(0.5), turn(1.0));
    assert!((half / full - 0.5).abs() < 0.02, "half {half}, full {full}");
}

/// The lever sets the throttle outright, where the keys walk it.
#[test]
fn a_throttle_lever_sets_the_throttle_where_it_is() {
    let mut ship = Ship::new([0.0; 3], 0.0);
    fly(&mut ship, Controls { lever: Some(0.25), ..Controls::default() }, 1.0);
    assert!((ship.throttle - 0.25).abs() < 1e-6, "{}", ship.throttle);
    fly(&mut ship, Controls { lever: Some(1.0), ..Controls::default() }, 0.1);
    assert_eq!(ship.throttle, 1.0);
}

/// A stick reading centred must not take the keyboard away, which is what a
/// device that is not a joystick at all reports.
#[test]
fn a_centred_stick_leaves_the_keys_alone() {
    let mut ship = Ship::new([0.0; 3], 0.0);
    let controls = Controls { right: true, stick: Some([0.0; 3]), ..Controls::default() };
    fly(&mut ship, controls, 1.0);
    let with = hb_formats::fixed::signed(ship.angles()[2]);

    let mut ship = Ship::new([0.0; 3], 0.0);
    fly(&mut ship, Controls { right: true, ..Controls::default() }, 1.0);
    let without = hb_formats::fixed::signed(ship.angles()[2]);

    assert!((with - without).abs() < 1e-3, "with {with}, without {without}");
}

/// And a stick pushed the other way does not cancel a key either: whichever
/// is asking for more of a direction wins.
#[test]
fn a_key_and_a_stick_do_not_fight() {
    let mut ship = Ship::new([0.0; 3], 0.0);
    let controls = Controls { right: true, stick: Some([0.0, -0.3, 0.0]), ..Controls::default() };
    fly(&mut ship, controls, 1.0);
    assert!(hb_formats::fixed::signed(ship.angles()[2]) > 0.0, "the key still turns right");
}
