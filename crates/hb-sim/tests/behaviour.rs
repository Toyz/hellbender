//! Class 26: bobbing by a quarter of its own size, turning once in eight
//! seconds.

use hb_sim::behaviour::{Hover, CIRCLE, RATE};

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

use hb_sim::behaviour::{Around, Motion};
use hb_sim::turret::Rng;

fn around() -> Around {
    Around { radius: 8.0, sky: 127.5, ground: 10.0 }
}

#[test]
fn class_seventeen_rises_turning_and_is_gone_past_twice_the_sky() {
    let mut rng = Rng::new(1);
    let mut ship = Motion::of(17, [0.0, 20.0, 0.0], [0.0; 3]).unwrap();
    // Four units a second: a minute takes it 240 units up, past twice 127.5.
    let mut turned = 0.0;
    let mut gone_at = None;
    for frame in 0..(70 * 30) {
        let moved = ship.step(1.0 / 30.0, around(), &mut rng);
        turned = moved.angles[2];
        if moved.gone && gone_at.is_none() {
            gone_at = Some((frame as f32 / 30.0, moved.at[1]));
        }
    }
    let (when, height) = gone_at.expect("it never left");
    assert!((height - 255.0).abs() < 1.0, "it left at {height}");
    // From 20 units at four a second, 255 units is about 59 seconds.
    assert!((when - 58.8).abs() < 0.5, "it left after {when}s");
    assert!(turned.is_finite());
}

#[test]
fn class_eighteen_falls_from_half_the_sky_and_explodes_on_the_ground() {
    let mut rng = Rng::new(3);
    let start = [100.0, 0.0, 200.0];
    let mut rock = Motion::of(18, start, [0.0; 3]).unwrap();

    // The first frame places it: high above the ground, and scattered.
    let first = rock.step(1.0 / 30.0, around(), &mut rng);
    assert!((first.at[1] - (10.0 + 127.5 / 2.0)).abs() < 3.0, "it starts at {}", first.at[1]);
    assert!((first.at[0] - start[0]).abs() <= 16.0 && first.at[0] != start[0], "it scatters in x");
    assert!((first.at[2] - start[2]).abs() <= 16.0, "and in z");

    // Then it falls at 64 units a second, tumbling, and explodes on the
    // ground about a second later.
    let mut blast = None;
    for frame in 1..(4 * 30) {
        let moved = rock.step(1.0 / 30.0, around(), &mut rng);
        if let Some(at) = moved.blast {
            blast = Some((frame as f32 / 30.0, at));
            break;
        }
        assert!(moved.angles.iter().all(|a| (0.0..65536.0).contains(a)), "it tumbles in range");
    }
    let (when, at) = blast.expect("it never landed");
    assert!((when - 1.15).abs() < 0.2, "it landed after {when}s");
    assert!((at[1] - 10.0).abs() < 3.0, "it landed at {}", at[1]);

    // And it starts again from the top.
    let next = rock.step(1.0 / 30.0, around(), &mut rng);
    assert!(next.at[1] > 60.0, "it did not start again: {}", next.at[1]);
}
