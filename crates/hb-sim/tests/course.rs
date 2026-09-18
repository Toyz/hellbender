//! Following a course, with a synthetic course and then the game's own.

use std::path::PathBuf;

use hb_formats::course::{self, Course, Point};
use hb_sim::{Follower, Phase};

fn square(periodic: i32) -> Course {
    let p = |x: i32, z: i32| Point { x: x << 16, y: 0, z: z << 16 };
    Course::Points {
        ground: 0,
        periodic,
        points: vec![p(0, 0), p(0, 10), p(10, 10), p(10, 0)],
    }
}

#[test]
fn an_actor_joins_its_course_at_the_nearest_point() {
    // Placed well away from the course, nearest to its third point. The
    // engine's phase 0 picks the nearest point, not the first.
    let f = Follower::new(&square(1), [12 << 16, 0, 13 << 16]).unwrap();
    assert_eq!(f.target, 2);
    assert_eq!(f.phase, Phase::Joining);
}

#[test]
fn it_reaches_the_course_then_follows_it_in_order() {
    let mut f = Follower::new(&square(1), [0, 0, -5 << 16]).unwrap();
    assert_eq!(f.target, 0);
    // Five units to the first point at 20 units a second is a quarter second.
    f.step(0.3);
    assert_eq!(f.phase, Phase::Following);
    assert_eq!(f.target, 1);
    // Heading for (0, 10) from near (0, 0) is straight along +z: heading 0.
    assert!(f.heading() < 200 || f.heading() > 65_336, "heading {}", f.heading());
}

#[test]
fn a_periodic_course_loops_and_a_plain_one_stops() {
    let mut looping = Follower::new(&square(1), [0, 0, 0]).unwrap();
    let mut once = Follower::new(&square(0), [0, 0, 0]).unwrap();
    // Forty units of course at twenty a second, twice round.
    for _ in 0..400 {
        looping.step(0.01);
        once.step(0.01);
    }
    assert_ne!(looping.phase, Phase::Finished, "a periodic course never ends");
    assert_eq!(once.phase, Phase::Finished, "a plain course runs out");
    // The plain one stops at its last point.
    assert_eq!(once.position, [10.0, 0.0, 0.0]);
}

#[test]
fn a_course_with_no_points_is_refused() {
    let empty = Course::Points { ground: 0, periodic: 0, points: vec![] };
    assert!(Follower::new(&empty, [0, 0, 0]).is_none());
}

#[test]
fn the_heading_follows_the_engines_convention() {
    // 0 along +z, a quarter turn along +x - as the recorded demo measured.
    let p = |x: i32, z: i32| Point { x: x << 16, y: 0, z: z << 16 };
    let east = Course::Points { ground: 0, periodic: 0, points: vec![p(0, 0), p(10, 0)] };
    let mut f = Follower::new(&east, [0, 0, 0]).unwrap();
    f.step(0.01);
    let h = f.heading() as i32;
    assert!((h - 0x4000).abs() < 50, "heading {h:#x}");
}

#[test]
fn every_shipped_course_that_something_follows_can_be_followed() {
    let dir = std::env::var_os("HB_GAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../original"));
    let path = dir.join("system/GAME.POD");
    if !path.exists() {
        eprintln!("skipping: {} is not there", path.display());
        return;
    }
    let pod = hb_pod::Pod::open(path).unwrap();
    let mut followed = 0usize;
    for e in pod.entries().iter().filter(|e| e.ext() == "lvl") {
        let level = hb_formats::lvl::Level::parse(pod.bytes(e)).unwrap();
        let stem = level.stem().to_string();
        let Ok(crs) = pod.read("data", &level.courses) else { continue };
        let courses = course::parse(crs).unwrap();
        let def = pod.read("data", &format!("{stem}.def")).unwrap();
        let kinds = hb_formats::text::enemy_defs(def).unwrap();
        for p in hb_formats::text::placements(def).unwrap() {
            let c = kinds[p.kind].course;
            if c < 0 || c as usize >= courses.len() {
                continue;
            }
            let mut f = Follower::new(&courses[c as usize], [p.x, p.y, p.z])
                .unwrap_or_else(|| panic!("{stem}: course {c} is empty"));
            // A minute of simulated time never produces a NaN or leaves the
            // world by more than the course itself does.
            for _ in 0..60 {
                f.step(1.0);
                assert!(f.position.iter().all(|v| v.is_finite()), "{stem}");
                assert!(f.position[0].abs() < 1100.0 && f.position[2].abs() < 1100.0, "{stem}");
            }
            followed += 1;
        }
    }
    assert_eq!(followed, 1_301);
}
