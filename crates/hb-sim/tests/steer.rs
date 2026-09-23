//! The steering, `0x4944c0`, on flat ground.

use hb_sim::steer::{lead, Body, Flat, Mode, Order, THRUST};

const DT: f32 = 1.0 / 30.0;

fn order(target: [f32; 3]) -> Order {
    Order {
        target,
        mode: Mode::Toward,
        speed: 20.0,
        // A hornet's turn rate, circle units a second.
        turn: 30_000.0,
        thrust: 1.0,
        class: 7,
        clearance: 3.0,
        breaking: false,
        aim: None,
    }
}

fn run(body: &mut Body, order: &Order, seconds: f32) {
    for _ in 0..(seconds / DT).round() as usize {
        body.steer(order, &Flat(0.0), DT);
    }
}

#[test]
fn it_speeds_up_at_the_thrust_and_settles_on_the_speed() {
    let mut body = Body::new([0.0, 50.0, 0.0], 0.0);
    let far = order([0.0, 50.0, 400.0]);
    run(&mut body, &far, 1.0);
    let v = body.velocity[2];
    assert!((v - THRUST).abs() < 0.5, "a second at 8 units a second squared: {v}");
    run(&mut body, &far, 5.0);
    let v = body.velocity[2];
    assert!((v - 20.0).abs() < 1.0, "held at the speed asked for: {v}");
}

#[test]
fn twice_the_thrust_gets_there_in_half_the_time() {
    let mut body = Body::new([0.0, 50.0, 0.0], 0.0);
    run(&mut body, &Order { thrust: 2.0, ..order([0.0, 50.0, 400.0]) }, 1.0);
    assert!((body.velocity[2] - 2.0 * THRUST).abs() < 0.5, "{}", body.velocity[2]);
}

#[test]
fn asked_to_stop_it_coasts_down() {
    // Speed zero: the thrust is minus the forward speed, so it falls by a
    // factor of e a second.
    let mut body = Body::new([0.0, 50.0, 0.0], 0.0);
    body.velocity = [0.0, 0.0, 20.0];
    run(&mut body, &Order { speed: 0.0, ..order([0.0, 50.0, 400.0]) }, 1.0);
    let v = body.velocity[2];
    assert!((v - 20.0 / std::f32::consts::E).abs() < 0.5, "{v}");
}

#[test]
fn it_banks_into_a_turn() {
    let mut body = Body::new([0.0, 50.0, 0.0], 0.0);
    body.velocity = [0.0, 0.0, 20.0];
    // Hard to the right: +x from a heading of 0.
    run(&mut body, &order([200.0, 50.0, 0.0]), 0.5);
    let roll = hb_formats::fixed::signed(body.roll);
    assert!(roll.abs() > 2000.0, "banked: {roll}");
    let heading = hb_formats::fixed::signed(body.heading);
    assert!(heading > 1000.0, "and came round toward +x: {heading}");
    // Pointed at it, it levels out again.
    run(&mut body, &order([200.0, 50.0, 0.0]), 6.0);
    let roll = hb_formats::fixed::signed(body.roll);
    assert!(roll.abs() < 1500.0, "level again: {roll}");
}

#[test]
fn away_opens_the_distance() {
    let mut body = Body::new([0.0, 50.0, 0.0], 0.0);
    let target = [0.0, 50.0, 30.0];
    run(&mut body, &Order { mode: Mode::Away, ..order(target) }, 6.0);
    let d = (0..3).map(|k| (body.position[k] - target[k]).powi(2)).sum::<f32>().sqrt();
    assert!(d > 60.0, "{d}");
}

#[test]
fn a_flyer_is_held_above_the_floor_and_a_transport_is_not() {
    // Diving at a point underground: the look-ahead lifts the target, and
    // after moving the height is held at the floor plus the clearance.
    let mut flyer = Body::new([0.0, 4.0, 0.0], 0.0);
    flyer.velocity = [0.0, 0.0, 20.0];
    run(&mut flyer, &order([0.0, -50.0, 60.0]), 3.0);
    assert!(flyer.position[1] >= 3.0 - 1e-3, "{}", flyer.position[1]);

    // Class 50 neither looks ahead nor is held above anything.
    let mut shuttle = Body::new([0.0, 4.0, 0.0], 0.0);
    shuttle.velocity = [0.0, 0.0, 20.0];
    run(&mut shuttle, &Order { class: 50, ..order([0.0, -50.0, 60.0]) }, 3.0);
    assert!(shuttle.position[1] < 0.0, "{}", shuttle.position[1]);
}

#[test]
fn the_lead_point_is_ahead_of_a_moving_player() {
    // Straight ahead and still: aim straight at him.
    let (h, p) = lead([0.0; 3], [0.0, 0.0, 100.0], [0.0; 3], 50.0, DT);
    assert!(h.abs() < 1.0 && p.abs() < 1.0, "{h} {p}");
    // Crossing to the right at 20 a second with shots at 50: lead him.
    let (h, _) = lead([0.0; 3], [0.0, 0.0, 100.0], [20.0, 0.0, 0.0], 50.0, DT);
    let expected = (20.0f32 * 2.0).atan2(100.0) / std::f32::consts::TAU * 65536.0;
    assert!((h - expected).abs() < 100.0, "{h} against {expected}");
    // No shot speed, no lead.
    let (h, _) = lead([0.0; 3], [0.0, 0.0, 100.0], [20.0, 0.0, 0.0], 0.0, DT);
    assert!(h.abs() < 1.0, "{h}");
}
