//! Missile smoke (`0x478d90`, `0x478f00`, `0x479480`).

use hb_sim::smoke::{Segment, Smoke, Trail, DELAY, FIRST, LIFE, SEGMENTS, WIDTH};

const DT: f32 = 1.0 / 30.0;

#[test]
fn a_missile_starts_smoking_three_sixteenths_in_and_then_every_frame() {
    let mut trail = Trail::new([0.0; 3]);
    let mut laid = Vec::new();
    for frame in 1..=20 {
        let at = [0.0, 0.0, frame as f32];
        if let Some(seg) = trail.step(19, at, DT) {
            laid.push((frame, seg));
        }
    }
    let first = laid[0].0;
    assert_eq!(first, (FIRST / DT).ceil() as usize);
    // Every frame after, each from where the last ended.
    assert_eq!(laid.len(), 20 - first + 1);
    assert_eq!(laid[0].1 .0, [0.0; 3], "the first starts at the launch");
    for w in laid.windows(2) {
        assert_eq!(w[0].1 .1, w[1].1 .0);
    }
}

#[test]
fn only_the_viper_the_cruise_and_the_cluster_smoke_this_way() {
    for (kind, smokes) in [(18, false), (19, true), (24, true), (25, true), (26, false), (30, false)] {
        let mut trail = Trail::new([0.0; 3]);
        let any = (0..30).any(|f| trail.step(kind, [f as f32, 0.0, 0.0], DT).is_some());
        assert_eq!(any, smokes, "kind {kind}");
    }
}

#[test]
fn a_segment_shows_after_an_eighth_thins_and_is_gone_at_two_seconds() {
    let mut s = Segment { from: [0.0; 3], to: [1.0, 0.0, 0.0], clock: 0.0, life: LIFE };
    assert_eq!(s.radius(), None, "not straight away");
    s.clock = DELAY + 0.01;
    let r = s.radius().unwrap();
    assert!((r - WIDTH * (LIFE - s.clock) / LIFE).abs() < 1e-6);
    s.clock = 1.0;
    assert!((s.radius().unwrap() - WIDTH / 2.0).abs() < 1e-6, "half as thick at half its life");
    let mut smoke = Smoke::default();
    smoke.lay([0.0; 3], [1.0, 0.0, 0.0]);
    for _ in 0..61 {
        smoke.step(DT);
    }
    assert_eq!(smoke.shown().count(), 0);
}

#[test]
fn the_pool_is_a_hundred_taken_in_turn() {
    let mut smoke = Smoke::default();
    for i in 0..SEGMENTS + 5 {
        smoke.lay([i as f32, 0.0, 0.0], [i as f32 + 1.0, 0.0, 0.0]);
    }
    assert_eq!(smoke.segments.len(), 100);
    // The first five slots hold the last five laid.
    assert_eq!(smoke.segments[0].from[0], SEGMENTS as f32);
    assert_eq!(smoke.segments[5].from[0], 5.0);
}
