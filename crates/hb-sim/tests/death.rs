//! The ship's last few seconds.

use hb_sim::death::{Wants, Wreck, BLAST, DRIFT, PITCH_LIMIT, TURN, WAIT};

#[test]
fn the_wreck_noses_over_and_drifts_until_it_hits() {
    let mut wreck = Wreck::new(0.0, 0.0, 0.0);
    let mut at = [0.0, 60.0, 0.0];
    let dt = 1.0 / 30.0;
    let mut seconds = 0.0;
    let mut wants = None;
    while wants.is_none() && seconds < 30.0 {
        wants = wreck.step(&mut at, dt, 0.0);
        seconds += dt;
    }
    assert_eq!(wants, Some(Wants::Explode));
    // It blows up two units over the ground.
    assert!((at[1] - BLAST).abs() < 1.0, "{at:?}");
    // Nose down forty-five degrees and no further, rolling all the way.
    assert_eq!(wreck.pitch, PITCH_LIMIT);
    assert!(wreck.roll > TURN * seconds - TURN * dt, "{}", wreck.roll);
    // Drifting forward the while: at 45 degrees it falls and flies equally.
    assert!(at[2] > 20.0, "{at:?}");
    assert!((seconds - 58.0 / (DRIFT * 0.7071)).abs() < 2.0, "{seconds} s");
}

#[test]
fn the_level_is_over_five_seconds_after_it_blows_up() {
    let mut wreck = Wreck::new(0.0, 0.0, 0.0);
    let mut at = [0.0, 1.0, 0.0];
    // On the ground already: it goes at once.
    assert_eq!(wreck.step(&mut at, 1.0 / 30.0, 0.0), Some(Wants::Explode));
    assert!(wreck.exploded);
    let fell = at;
    let dt = 1.0 / 30.0;
    let mut seconds = 0.0;
    let mut over = None;
    while over.is_none() && seconds < 20.0 {
        over = wreck.step(&mut at, dt, 0.0);
        seconds += dt;
    }
    assert_eq!(over, Some(Wants::Over));
    assert!((seconds - WAIT).abs() < 0.1, "{seconds} s");
    // It stays where it fell.
    assert_eq!(at, fell);
}
