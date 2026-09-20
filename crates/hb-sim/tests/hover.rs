//! Class 26: bobbing by a quarter of its own size, turning once in eight
//! seconds.

use hb_sim::hover::{Hover, CIRCLE, RATE};

#[test]
fn it_bobs_a_quarter_of_its_radius_about_where_it_started() {
    let start = [10.0, 20.0, 30.0];
    let mut hover = Hover::new(start, 0.0);
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    // Eight seconds is one full bob.
    for _ in 0..(8 * 60) {
        let (at, _) = hover.step(1.0 / 60.0, 8.0);
        assert_eq!([at[0], at[2]], [start[0], start[2]], "it stays over its spot");
        low = low.min(at[1]);
        high = high.max(at[1]);
    }
    assert!((high - 22.0).abs() < 0.05, "the top was {high}");
    assert!((low - 18.0).abs() < 0.05, "the bottom was {low}");
}

#[test]
fn it_turns_once_in_eight_seconds() {
    let mut hover = Hover::new([0.0; 3], 0.0);
    for _ in 0..(4 * 60) {
        hover.step(1.0 / 60.0, 1.0);
    }
    // Half a turn in four seconds.
    assert!((hover.heading - CIRCLE / 2.0).abs() < 100.0, "half way is {}", hover.heading);
    for _ in 0..(4 * 60) {
        hover.step(1.0 / 60.0, 1.0);
    }
    // And back to where it started, wrapped rather than run away.
    assert!(hover.heading < 100.0 || hover.heading > CIRCLE - 100.0, "{}", hover.heading);
}

#[test]
fn the_rate_is_the_frame_time_over_eight() {
    // The engine adds frameTime / 8 to both the phase and the heading each
    // frame, with the frame time in 16.16 seconds.
    assert_eq!(RATE, 65536.0 / 8.0);
}
