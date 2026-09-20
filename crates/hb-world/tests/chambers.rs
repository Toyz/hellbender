//! The chambers: the second pair of heightfields the tunnels are cut from,
//! and what the ship's collision reads from them.

use hb_formats::terrain::{Layer, Terrain};
use hb_pod::Pod;
use hb_world::{Cell, Grid};

fn game() -> Option<Pod> {
    let dir = std::env::var_os("HB_GAME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../original"));
    let path = dir.join("system/GAME.POD");
    if !path.exists() {
        eprintln!("skipping: {} is not there", path.display());
        return None;
    }
    Some(Pod::open(path).unwrap())
}

fn terrain(pod: &Pod, stem: &str) -> Terrain {
    Terrain::load(|ext| pod.read("data", &format!("{stem}.{ext}")).ok().map(<[u8]>::to_vec)).unwrap()
}

/// A chamber cell has its ceiling above its floor and both below the ground,
/// so the ship is underground while it is in one; elsewhere the two meet,
/// which is the rock the collision backs out of.
#[test]
fn a_chamber_is_a_floor_under_a_ceiling_below_zero() {
    let Some(pod) = game() else { return };
    let terrain = terrain(&pod, "hoth");
    let grid = Grid::new(&terrain);
    let (mut chambers, mut open, mut solid) = (0, 0, 0);
    let (mut lowest, mut highest) = (i32::MAX, i32::MIN);
    for x in 0..128 {
        for z in 0..128 {
            let cell = Cell::new(x, z);
            let floor = grid.height_at_grid(Layer::ChamberFloor, x, z).unwrap();
            let roof = grid.height_at_grid(Layer::ChamberCeiling, x, z).unwrap();
            if !grid.has_chamber(cell) {
                continue;
            }
            chambers += 1;
            if roof > floor {
                open += 1;
                lowest = lowest.min(floor);
                highest = highest.max(roof);
                // Below the world's zero: the collision tells chamber from
                // ground by the sign.
                assert!(floor < 0, "floor {floor} at ({x}, {z})");
                assert!(roof <= 0, "ceiling {roof} at ({x}, {z})");
            } else {
                solid += 1;
            }
        }
    }
    assert_eq!(chambers, 5_414, "cells flagged as chamber");
    assert!(open > 4_000, "{open} of {chambers} are open");
    assert!(solid > 0, "some are rock: {solid}");
    // HOTH's tunnels: about a hundred units down, tens of units tall.
    assert!((-110.0..-100.0).contains(&(lowest as f32 / 65536.0)), "{lowest}");
    assert!(highest <= 0);
}

/// Every level either has chambers or has none at all; the ones that do put
/// them under ground that is above them.
#[test]
fn levels_with_chambers_keep_them_under_the_ground() {
    let Some(pod) = game() else { return };
    let mut with = 0;
    for e in pod.entries().iter().filter(|e| e.ext() == "lvl") {
        let level = hb_formats::lvl::Level::parse(pod.bytes(e)).unwrap();
        let terrain = terrain(&pod, level.stem());
        let grid = Grid::new(&terrain);
        let mut cells = 0;
        for x in 0..128 {
            for z in 0..128 {
                let cell = Cell::new(x, z);
                if !grid.has_chamber(cell) {
                    continue;
                }
                let floor = grid.height_at_grid(Layer::ChamberFloor, x, z).unwrap();
                let roof = grid.height_at_grid(Layer::ChamberCeiling, x, z).unwrap();
                if roof <= floor {
                    continue;
                }
                cells += 1;
                let ground = grid.height_at_grid(Layer::Ground, x, z).unwrap();
                assert!(roof <= ground, "{}: ceiling {roof} over ground {ground}", level.stem());
            }
        }
        with += usize::from(cells > 0);
    }
    assert!(with >= 8, "{with} levels have chambers");
}
