//! The explosion's puffs.

use hb_sim::explosion::{Blasts, Puff, FRAMES, FRAME_TIME, LIFE, PUFFS, SLOTS};
use hb_sim::turret::Rng;

#[test]
fn an_explosion_is_ten_puffs_around_one() {
    let mut rng = Rng::new(9);
    let mut blasts = Blasts::default();
    blasts.burst([10.0, 20.0, -30.0], 4.0, &mut rng);
    assert_eq!(blasts.puffs.len(), PUFFS + 1);
    let centre = blasts.puffs.last().unwrap();
    assert_eq!(centre.position, [10.0, 20.0, -30.0]);
    assert_eq!(centre.size, 16.0, "the centre is four times the size");
    for p in &blasts.puffs[..PUFFS] {
        assert_eq!(p.size, 8.0, "the rest are twice");
        // Scattered within the size it was given.
        for k in 0..3 {
            let d = p.position[k] - [10.0, 20.0, -30.0][k];
            assert!(d.abs() <= 4.0, "{d}");
        }
    }
    // Each runs at a quarter to three quarters of real time.
    for p in &blasts.puffs {
        assert!((0.25..0.75).contains(&p.rate), "{}", p.rate);
        assert_eq!(p.frame(), Some(0));
    }
}

#[test]
fn a_puff_runs_through_sixteen_frames_and_stops() {
    let mut p = Puff { position: [0.0; 3], size: 2.0, age: 0.0, rate: 0.5 };
    let dt = 1.0 / 60.0;
    let mut seen = Vec::new();
    while p.step(dt) {
        let frame = p.frame().unwrap();
        if seen.last() != Some(&frame) {
            seen.push(frame);
        }
    }
    assert_eq!(seen, (0..FRAMES).collect::<Vec<_>>());
    // Sixteen frames of a sixteenth of its own second, at half speed.
    assert!((p.age / 0.5 - FRAMES as f32 * FRAME_TIME / 0.5).abs() < 0.1, "{}", p.age);
    assert!(p.age < LIFE);
    // The slowest a puff runs, it still ends on its frames, not its life.
    let mut slow = Puff { position: [0.0; 3], size: 1.0, age: 0.0, rate: 0.25 };
    while slow.step(dt) {}
    assert!(slow.age < LIFE, "{}", slow.age);
}

#[test]
fn the_pool_holds_sixteen() {
    let mut rng = Rng::new(3);
    let mut blasts = Blasts::default();
    for _ in 0..4 {
        blasts.burst([0.0; 3], 1.0, &mut rng);
    }
    assert_eq!(blasts.puffs.len(), SLOTS);
    blasts.step(10.0);
    assert!(blasts.puffs.is_empty(), "they all burn out");
}

/// Most things in the engine make one puff, not eleven: `0x476f90` is called
/// straight from the shot's mark, the missile's, and what a destroyed actor
/// leaves behind.
#[test]
fn one_puff_is_one_slot_at_its_own_size() {
    use hb_sim::explosion::Blasts;
    let mut blasts = Blasts::default();
    blasts.puff([1.0, 2.0, 3.0], 5.0);
    assert_eq!(blasts.puffs.len(), 1);
    let puff = blasts.puffs[0];
    assert_eq!(puff.position, [1.0, 2.0, 3.0]);
    assert_eq!(puff.size, 5.0, "the size it was given, not a multiple of it");
    assert_eq!(puff.rate, 1.0, "only a burst draws a random rate");
}

/// And a burst is eleven of them, which is what a big radius made of every
/// destroyed building until it was read properly.
#[test]
fn a_burst_is_eleven_puffs_and_much_wider() {
    use hb_sim::explosion::Blasts;
    use hb_sim::turret::Rng;
    let mut rng = Rng::new(9);
    let mut blasts = Blasts::default();
    blasts.burst([0.0; 3], 5.0, &mut rng);
    assert_eq!(blasts.puffs.len(), 11);
    let widest = blasts.puffs.iter().map(|p| p.size).fold(0.0f32, f32::max);
    assert_eq!(widest, 20.0, "the middle one is four times over");

    let mut one = Blasts::default();
    one.puff([0.0; 3], 5.0);
    assert!(widest > one.puffs[0].size * 3.0, "a burst dwarfs a puff");
}
