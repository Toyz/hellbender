//! Snow and rain (`0x49c9b0`, `0x49ce80`).

use hb_sim::turret::Rng;
use hb_sim::weather::{self, Particle, Weather, DROPS, FLAKES, HALF, RAIN_FALL, SNOW_FALL, STREAK};

const EYE: [i32; 3] = [100 << 16, 20 << 16, -300 << 16];

#[test]
fn the_pools_are_the_engines_sizes_and_start_around_the_eye() {
    let w = Weather::new(EYE, &mut Rng::new(7));
    assert_eq!((w.snow.len(), w.rain.len()), (FLAKES, DROPS));
    assert_eq!((FLAKES, DROPS), (300, 200));
    for p in w.snow.iter().chain(&w.rain) {
        for k in 0..3 {
            let d = p.position[k] - EYE[k];
            assert!((-HALF..HALF).contains(&d), "{d:#x}");
        }
        // Down, with a drift of under a sixty-fourth of a unit a second.
        assert!(p.velocity[0].abs() <= 0x400 && p.velocity[2].abs() <= 0x400);
    }
    assert!(w.snow.iter().all(|p| p.velocity[1] == SNOW_FALL));
    assert!(w.rain.iter().all(|p| p.velocity[1] == RAIN_FALL));
}

#[test]
fn snow_falls_at_about_five_units_a_second_and_rain_at_about_ten() {
    assert_eq!(SNOW_FALL, -320_000);
    assert_eq!(RAIN_FALL, -640_000);
    let mut pool = vec![Particle { position: EYE, velocity: [0, SNOW_FALL, 0], up: [0; 3], shown: false }];
    Weather::step(&mut pool, 1.0, EYE);
    assert_eq!(pool[0].position[1], EYE[1] + SNOW_FALL);
    assert!(pool[0].shown);
}

#[test]
fn a_particle_leaving_the_cube_comes_in_the_other_side_unseen() {
    // Just under eight units below the eye and falling: the next second
    // takes it past the bottom, and it reappears near the top.
    let start = [EYE[0], EYE[1] - HALF + 0x100, EYE[2]];
    let mut pool = vec![Particle { position: start, velocity: [0, RAIN_FALL, 0], up: [0; 3], shown: true }];
    Weather::step(&mut pool, 0.1, EYE);
    let d = pool[0].position[1] - EYE[1];
    assert!(d > 0 && d < HALF, "{d:#x}");
    assert!(!pool[0].shown, "not drawn on the frame it moves");
    Weather::step(&mut pool, 0.1, EYE);
    assert!(pool[0].shown);
}

#[test]
fn it_falls_only_between_the_ground_and_the_sky_under_nothing() {
    let sky = 128 << 16;
    assert!(Weather::falls(10 << 16, sky, None));
    assert!(!Weather::falls(-1, sky, None), "under the ground");
    assert!(!Weather::falls(sky + 1, sky, None), "above the sky layer");
    assert!(!Weather::falls(10 << 16, sky, Some(20 << 16)), "under an overhang");
    assert!(Weather::falls(30 << 16, sky, Some(20 << 16)), "above what is below");
}

#[test]
fn a_drop_streaks_up_at_rest_and_leans_along_the_ships_travel() {
    let w = Weather::new(EYE, &mut Rng::new(3));
    let drop = w.rain[0];
    let still = Weather::streak_end(&drop, [0; 3]);
    let rise = still[1] - drop.position[1];
    assert!((rise - STREAK).abs() < 64, "{rise:#x}");
    // Flying along +z a unit a frame: the streak tips toward +z.
    let flying = Weather::streak_end(&drop, [0, 0, 1 << 16]);
    assert!(flying[2] - drop.position[2] > STREAK / 2);
}

#[test]
fn they_are_drawn_in_colour_ramp_one() {
    // Ramp 1 runs from index 0 to 15; snow is its top and rain 0x6000 of the
    // way up (`0x4566c0`).
    assert_eq!(weather::SNOW_INDEX, (15 * 0xffff >> 16) as u8);
    assert_eq!(weather::RAIN_INDEX, (15 * 0x6000 >> 16) as u8);
}
