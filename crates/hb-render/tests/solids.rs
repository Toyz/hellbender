//! What the ship can fly into. Needs the game and skips without it.

use hb_sim::collide::{self, Solid};
use hb_sim::combat::{self, HitVolume};

fn level(stem: &str) -> Option<hb_render::Level> {
    let dir = std::env::var_os("HB_GAME").map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../original")
    });
    let game = dir.join("system/GAME.POD");
    if !game.exists() {
        eprintln!("skipping: {} is not there", game.display());
        return None;
    }
    let game = hb_pod::Pod::open(game).unwrap();
    let startup = hb_pod::Pod::open(dir.join("system/STARTUP.POD")).ok();
    Some(hb_render::Level::load(&game, startup.as_ref(), stem).unwrap())
}

fn scenery(level: &hb_render::Level) -> Vec<Solid> {
    level
        .placements
        .iter()
        .filter(|p| !combat::rammable(level.kinds[p.kind].class()))
        .map(|p| {
            let volume = HitVolume::for_type(
                &level.kinds[p.kind],
                level.meshes.get(p.kind).and_then(Option::as_ref),
            );
            collide::solid_of(&volume, combat::position_of(p), p.heading as u16)
        })
        .collect()
}

/// Every level has scenery the engine would let the ship through, and the port
/// gives each of it a box the ship can be pushed out of.
#[test]
fn scenery_becomes_solid() {
    for stem in ["morbos", "hoth", "float"] {
        let Some(level) = level(stem) else { return };
        let solids = scenery(&level);
        assert!(!solids.is_empty(), "{stem} has no solid scenery");
        for s in &solids {
            assert!((0..3).all(|k| s.max[k] >= s.min[k]), "{stem}: an inside-out box {s:?}");
            let middle = std::array::from_fn(|k| (s.min[k] + s.max[k]) / 2.0);
            let (out, push) = collide::push_out(middle, collide::SHIP, std::slice::from_ref(s));
            assert!(push.is_some(), "{stem}: nothing pushed the ship out of {s:?}");
            assert_ne!(out, middle, "{stem}: the ship stayed inside {s:?}");
        }
    }
}

/// And the level still starts in open air: nothing the port made solid holds
/// the ship where it comes in.
#[test]
fn the_start_is_not_inside_anything() {
    for stem in ["morbos", "hoth", "float"] {
        let Some(level) = level(stem) else { return };
        let solids = scenery(&level);
        let grid = hb_world::Grid::new(&level.terrain);
        let middle = 64.0 * 8.0;
        let ground = grid
            .height_at(hb_formats::terrain::Layer::Ground, (middle * 65536.0) as i32, (middle * 65536.0) as i32)
            .unwrap_or(0) as f32
            / 65536.0;
        let at = [middle, ground + 16.0, middle];
        let (_, push) = collide::push_out(at, collide::SHIP, &solids);
        assert_eq!(push, None, "{stem} starts inside its own scenery");
    }
}

/// The boxes are the objects' own size, not the whole map: the biggest is a
/// reactor, and it is under a hundred units across.
#[test]
fn nothing_is_absurdly_big() {
    for stem in ["morbos", "hoth", "float"] {
        let Some(level) = level(stem) else { return };
        let mut widest = 0.0f32;
        for s in scenery(&level) {
            for k in 0..3 {
                widest = widest.max(s.max[k] - s.min[k]);
            }
        }
        println!("{stem}: widest solid {widest:.1} units");
        assert!(widest < 128.0, "{stem}: a solid {widest} units across");
    }
}
