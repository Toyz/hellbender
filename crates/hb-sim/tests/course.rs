//! Following a course - class 47, `0x421240` - with a synthetic course and
//! then the game's own.


use hb_formats::course::{self, Course, Point};
use hb_formats::text::{EnemyDef, Placement};
use hb_sim::course::{bias, COURSE_CLASSES, REACHED};
use hb_sim::{Follower, Phase};

fn square(periodic: i32) -> Course {
    let p = |x: i32, z: i32| Point { x: x << 16, y: 0, z: z << 16 };
    Course::Points {
        ground: 0,
        periodic,
        points: vec![p(0, 0), p(0, 40), p(40, 40), p(40, 0)],
    }
}

fn placed(x: i32, z: i32) -> Placement {
    Placement { kind: 0, hit_points: 4096, x: x << 16, y: 0, z: z << 16, pitch: 0, roll: 0, heading: 0 }
}

/// A `hoth` car: eight units a second, turning at 1.53.
fn car() -> EnemyDef {
    EnemyDef { move_rate: 550_000, turn_rate: 100_000, ..EnemyDef::default() }
}

fn flat(_: [f32; 3]) -> f32 {
    0.0
}

/// Run a follower for `seconds` at 30 frames a second, recording each point
/// it takes as its target.
fn run(f: &mut Follower, seconds: f32) -> Vec<usize> {
    let mut targets = vec![f.target];
    for _ in 0..(seconds * 30.0) as usize {
        f.step(1.0 / 30.0, flat);
        if targets.last() != Some(&f.target) {
            targets.push(f.target);
        }
    }
    targets
}

#[test]
fn it_is_put_on_the_nearest_point_on_its_third_frame() {
    // Placed well away from the course, nearest its third point. Phase 0
    // chooses the point, phase 2 moves the actor onto it - no flight there.
    let mut f = Follower::new(&square(1), &placed(52, 55), &car(), 0).unwrap();
    assert_eq!(f.phase, Phase::Start);
    f.step(1.0 / 30.0, flat);
    assert_eq!((f.phase, f.target), (Phase::Placed, 2));
    assert_eq!(f.position, [52 << 16, 0, 55 << 16], "phase 0 does not move it");
    f.step(1.0 / 30.0, flat);
    assert_eq!(f.phase, Phase::Following);
    assert_eq!(f.position, [40 << 16, 0, 40 << 16]);
}

#[test]
fn the_nearest_point_is_measured_across_the_worlds_edge() {
    // 500 units east is 524 units west of the far point the short way round.
    let p = |x: i32| Point { x: x << 16, y: 0, z: 0 };
    let course = Course::Points { ground: 0, periodic: 0, points: vec![p(0), p(-500)] };
    let mut f = Follower::new(&course, &placed(500, 0), &car(), 0).unwrap();
    f.step(1.0 / 30.0, flat);
    assert_eq!(f.target, 1);
}

#[test]
fn a_looping_course_goes_round_and_a_plain_one_comes_back() {
    let mut looping = Follower::new(&square(1), &placed(0, 0), &car(), 0).unwrap();
    let mut plain = Follower::new(&square(0), &placed(0, 0), &car(), 0).unwrap();
    let round = run(&mut looping, 60.0);
    assert_eq!(&round[..6], &[0, 1, 2, 3, 0, 1], "{round:?}");
    // At the last point it turns round: 3 is taken again, then 2.
    let back = run(&mut plain, 60.0);
    assert_eq!(&back[..6], &[0, 1, 2, 3, 2, 1], "{back:?}");
}

#[test]
fn it_steers_along_its_heading_rather_than_sliding_to_the_point() {
    // Facing +z on a course whose next point is due east: the first frames
    // carry it north while the heading comes round.
    let p = |x: i32, z: i32| Point { x: x << 16, y: 0, z: z << 16 };
    let east = Course::Points { ground: 0, periodic: 0, points: vec![p(0, 0), p(40, 0)] };
    let mut f = Follower::new(&east, &placed(0, 0), &car(), 0).unwrap();
    f.step(1.0 / 30.0, flat);
    f.step(1.0 / 30.0, flat);
    // Within eight units of point 0, so it takes point 1 at once.
    f.step(1.0 / 30.0, flat);
    assert_eq!(f.target, 1);
    assert!(f.position[2] > 0, "it moved along its old heading, +z");
    assert!(f.heading > 0 && f.heading < 0x4000, "and is turning toward +x: {:#x}", f.heading);
    for _ in 0..300 {
        f.step(1.0 / 30.0, flat);
    }
    assert!((f.heading - 0x4000).abs() < 0x400 || f.target == 0, "{:#x}", f.heading);
}

#[test]
fn it_sits_on_the_floor_plus_its_types_height() {
    let mut kind = car();
    kind.fields[4] = 3 << 16;
    let mut f = Follower::new(&square(1), &placed(0, 0), &kind, 0).unwrap();
    for _ in 0..10 {
        f.step(1.0 / 30.0, |_| 5.0);
    }
    assert_eq!(f.position[1], 8 << 16);
}

#[test]
fn eight_units_in_x_and_z_is_arrival() {
    assert_eq!(REACHED, 0x80000);
}

#[test]
fn the_placement_index_sets_a_small_difference_in_speed() {
    // `(index << 30) >> 16`: 0, +1/4, -1/2, -1/4 of a unit a second.
    let got: Vec<i32> = (0..5).map(|i| bias(i, 100_000).0).collect();
    assert_eq!(got, [0, 16_384, -32_768, -16_384, 0]);
    // The turn takes half when the whole would leave it at zero or below.
    assert_eq!(bias(2, 20_000), (-32_768, -16_384));
    let f = Follower::new(&square(1), &placed(0, 0), &car(), 1).unwrap();
    assert_eq!(f.speed, 550_000 + 16_384);
}

#[test]
fn a_course_with_no_points_is_refused() {
    let empty = Course::Points { ground: 0, periodic: 0, points: vec![] };
    assert!(Follower::new(&empty, &placed(0, 0), &car(), 0).is_none());
}

#[test]
fn every_shipped_follower_follows() {
    let Some(pod) = hb_pod::game_pod("GAME.POD") else { return };
    let (mut followed, mut named, mut ignored) = (0usize, 0usize, 0usize);
    for e in pod.entries().iter().filter(|e| e.ext() == "lvl") {
        let level = hb_formats::lvl::Level::parse(pod.bytes(e)).unwrap();
        let stem = level.stem().to_string();
        let courses = match pod.read("data", &level.courses) {
            Ok(crs) => course::parse(crs).unwrap(),
            Err(_) => Vec::new(),
        };
        let def = pod.read("data", &format!("{stem}.def")).unwrap();
        let kinds = hb_formats::text::enemy_defs(def).unwrap();
        for (i, p) in hb_formats::text::placements(def).unwrap().iter().enumerate() {
            let kind = &kinds[p.kind];
            if kind.course < 0 {
                continue;
            }
            if !COURSE_CLASSES.contains(&kind.class()) {
                // Names a course; its routine never reads one.
                ignored += 1;
                continue;
            }
            named += 1;
            let Some(course) = courses.get(kind.course as usize) else { continue };
            let mut f = Follower::new(course, p, kind, i)
                .unwrap_or_else(|| panic!("{stem}: course {} is empty", kind.course));
            let points = course.points();
            // A minute: it never strays far from the leg it is driving -
            // from the point it last took to the one it is heading for.
            let mut from = None;
            for _ in 0..1800 {
                let was = f.target;
                f.step(1.0 / 30.0, flat);
                if f.phase != Phase::Following {
                    continue;
                }
                if was != f.target || from.is_none() {
                    from = Some(was);
                }
                let off = off_leg(f.position_fixed(), points[from.unwrap()], points[f.target]);
                assert!(off < 50.0, "{stem}: {} is {off} units off course {}", kind.name, kind.course);
            }
            followed += 1;
        }
    }
    // 302 placements of the seven course classes name a course, and every
    // one of those courses exists - the dangling ids all belong to classes
    // that never look. 1,258 more name a course their routine never reads.
    assert_eq!((named, followed, ignored), (302, 302, 1_258));
}

/// How far `at` is from the leg between two course points, in units, with
/// the world's wrap.
fn off_leg(at: [i32; 3], a: Point, b: Point) -> f32 {
    let rel = |q: Point| {
        let d = |u: i32, v: i32| hb_formats::fixed::wrap(u.wrapping_sub(v)) as f32 / 65536.0;
        [d(q.x, at[0]), d(q.z, at[2])]
    };
    let (a, b) = (rel(a), rel(b));
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len == 0.0 { 0.0 } else { (-(a[0] * ab[0] + a[1] * ab[1]) / len).clamp(0.0, 1.0) };
    let q = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    (q[0] * q[0] + q[1] * q[1]).sqrt()
}
