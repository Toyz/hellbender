//! The engine's own recorded flight, used as ground truth for the terrain.
//!
//! `DEMO1.DMO` is 50.8 seconds of the original game flying over `IOWAH2`, pose
//! by pose. If the port's terrain, height scale and coordinate system are
//! right, none of those poses is underground. If any of them is wrong, poses
//! sink - and the controls below show by how many, so a pass means something.

use std::path::PathBuf;

use hb_formats::demo::Demo;
use hb_formats::terrain::{Layer, Terrain};
use hb_pod::Pod;
use hb_world::Grid;

fn game() -> Option<Pod> {
    let dir = std::env::var_os("HB_GAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../original"));
    let path = dir.join("system/GAME.POD");
    if !path.exists() {
        eprintln!("skipping: {} is not there", path.display());
        return None;
    }
    Some(Pod::open(path).unwrap())
}

fn startup() -> Option<Pod> {
    let dir = std::env::var_os("HB_GAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../original"));
    let path = dir.join("system/STARTUP.POD");
    path.exists().then(|| Pod::open(path).unwrap())
}

fn terrain(pod: &Pod, stem: &str) -> Terrain {
    Terrain::load(|ext| pod.read("data", &format!("{stem}.{ext}")).ok().map(<[u8]>::to_vec))
        .unwrap()
}

#[test]
fn every_demo_parses_to_the_last_line() {
    let Some(startup) = startup() else { return };
    let mut seen = Vec::new();
    for e in startup.entries().iter().filter(|e| e.ext() == "dmo") {
        let demo = Demo::parse(startup.bytes(e)).unwrap_or_else(|w| panic!("{}: {w}", e.name));
        let times: Vec<i32> = demo
            .records
            .iter()
            .map(|r| match r {
                hb_formats::demo::Record::Pose(p) => p.time,
                hb_formats::demo::Record::Key(k) => k.time,
            })
            .collect();
        assert!(times.windows(2).all(|w| w[0] <= w[1]), "{}: time runs backwards", e.name);
        // Every key the demos press is fire, except one of the `1` key and one
        // of F1.
        for k in demo.keys() {
            assert!([57, 2, 59].contains(&k.scan_code), "{}: key {}", e.name, k.scan_code);
        }
        seen.push((e.file_name(), demo.level.clone(), demo.records.len()));
    }
    seen.sort();
    assert_eq!(
        seen,
        [
            ("demo1.dmo".to_string(), "iowah2.lvl".to_string(), 1025),
            ("demo2.dmo".to_string(), "red.lvl".to_string(), 1025),
            ("demo3.dmo".to_string(), "atmos-t2.lvl".to_string(), 811),
        ]
    );
}

#[test]
fn two_demos_were_recorded_on_levels_that_did_not_ship() {
    let (Some(game), Some(startup)) = (game(), startup()) else { return };
    let mut missing = Vec::new();
    for e in startup.entries().iter().filter(|e| e.ext() == "dmo") {
        let demo = Demo::parse(startup.bytes(e)).unwrap();
        if game.find("levels", &demo.level).is_none() {
            missing.push(demo.level);
        }
    }
    missing.sort();
    assert_eq!(missing, ["atmos-t2.lvl", "red.lvl"]);
}

#[test]
fn the_recorded_flight_never_goes_underground() {
    let (Some(game), Some(startup)) = (game(), startup()) else { return };
    let demo = Demo::parse(startup.read("demo", "demo1.dmo").unwrap()).unwrap();
    assert_eq!(demo.level, "iowah2.lvl");
    let terrain = terrain(&game, "iowah2");
    let grid = Grid::new(&terrain);

    let poses: Vec<_> = demo.poses().copied().collect();
    assert_eq!(poses.len(), 890);
    assert!((demo.seconds() - 50.8).abs() < 0.1, "{} seconds", demo.seconds());

    let mut lowest = i32::MAX;
    for p in &poses {
        let ground = grid.height_at(Layer::Ground, p.x, p.z).unwrap();
        assert!(
            p.y >= ground,
            "at t={:.2}s the pose is {:.2} units under the ground",
            p.time as f32 / 65536.0,
            (ground - p.y) as f32 / 65536.0
        );
        lowest = lowest.min(p.y - ground);
    }
    // The closest the pilot came is about five units, not exactly five.
    let lowest = lowest as f32 / 65536.0;
    assert!((4.5..5.5).contains(&lowest), "closest approach {lowest} units");
}

#[test]
fn the_ground_truth_test_can_fail() {
    // The same check against deliberately wrong terrain. If these passed, the
    // test above would be proving nothing.
    let (Some(game), Some(startup)) = (game(), startup()) else { return };
    let demo = Demo::parse(startup.read("demo", "demo1.dmo").unwrap()).unwrap();
    let raw = game.read("data", "iowah2.raw").unwrap();
    let hoth = game.read("data", "hoth.raw").unwrap();
    let sunk = |bytes: &[u8], shift: u32, swap: bool| {
        demo.poses()
            .filter(|p| {
                let (mut cx, mut cz) = (((p.x >> 19) & 127) as usize, ((p.z >> 19) & 127) as usize);
                if swap {
                    std::mem::swap(&mut cx, &mut cz);
                }
                p.y < (bytes[cz * 128 + cx] as i32) << shift
            })
            .count()
    };
    // The port's reading at the cell corner: nothing underground.
    assert_eq!(sunk(raw, 15, false), 0);
    // Twice the height scale.
    assert_eq!(sunk(raw, 16, false), 265);
    // x and z swapped.
    assert_eq!(sunk(raw, 15, true), 99);
    // The wrong level entirely.
    assert_eq!(sunk(hoth, 15, false), 746);
}
