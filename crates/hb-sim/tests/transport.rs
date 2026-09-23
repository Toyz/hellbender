//! The transports, classes 50 to 52, on the shipped levels.

use hb_formats::course;
use hb_formats::terrain::Terrain;
use hb_sim::transport::{Gone, Kind, Transport};

const DT: f32 = 1.0 / 30.0;

/// Every shipped transport, with its level's name and terrain.
fn shipped() -> Vec<(String, Terrain, Transport, f32)> {
    let Some(pod) = hb_pod::game_pod("GAME.POD") else { return Vec::new() };
    let mut out = Vec::new();
    for e in pod.entries().iter().filter(|e| e.ext() == "lvl") {
        let level = hb_formats::lvl::Level::parse(pod.bytes(e)).unwrap();
        let stem = level.stem().to_string();
        let Ok(crs) = pod.read("data", &level.courses) else { continue };
        let courses = course::parse(crs).unwrap();
        let def = pod.read("data", &format!("{stem}.def")).unwrap();
        let kinds = hb_formats::text::enemy_defs(def).unwrap();
        let sky = hb_formats::fixed::to_units((level.sky_height << 15) as i32);
        for (i, p) in hb_formats::text::placements(def).unwrap().iter().enumerate() {
            let kind = &kinds[p.kind];
            let Some(c) = courses.get(kind.course.max(0) as usize) else { continue };
            if let Some(t) = Transport::new(c, p, kind, i) {
                let terrain =
                    Terrain::load(|ext| pod.read("data", &format!("{stem}.{ext}")).ok().map(<[u8]>::to_vec)).unwrap();
                out.push((stem.clone(), terrain, t, sky));
            }
        }
    }
    out
}

#[test]
fn every_transport_reaches_its_ending() {
    let all = shipped();
    if all.is_empty() {
        return;
    }
    assert_eq!(all.len(), 16);
    for (stem, terrain, mut t, sky) in all {
        let grid = hb_world::Grid::new(&terrain);
        let player = [0.0, 1000.0, 0.0];
        let mut phases = vec![t.phase];
        let mut ending = None;
        for _ in 0..(20.0 * 60.0 / DT) as usize {
            if let Some(end) = t.step(DT, player, sky, &grid) {
                ending = Some(end);
                break;
            }
            if phases.last() != Some(&t.phase) {
                phases.push(t.phase);
            }
        }
        println!("{stem} {:?}: {phases:?} -> {ending:?}", t.kind);
        match t.kind {
            Kind::Disappear => assert_eq!(ending, Some(Gone::Away), "{stem}: {phases:?}"),
            Kind::Leave => assert_eq!(ending, Some(Gone::Escaped), "{stem}: {phases:?}"),
            // Out, down, round and up - and then, on a plain course, never
            // back: the target is still the last point, the leg behind it
            // folds onto that same point, and a leg of no length is never
            // passed. All three shipped ones are plain.
            Kind::Shuttle => {
                assert_eq!(ending, None);
                assert!(!t.walk.periodic);
                assert_eq!(phases, [0, 400, 2, 3, 1001, 1000, 2000, 400, 2, 3], "{stem}");
                assert!(t.walk.at_last() && t.walk.direction == -1, "{stem}");
            }
        }
    }
}
