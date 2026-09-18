//! Whole frames of a real level. These need the game and skip without it.

use hb_formats::Angle;
use hb_render::{Camera, Level, Target};

fn level(stem: &str) -> Option<Level> {
    let dir = std::env::var_os("HB_GAME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../original"));
    let game = dir.join("system/GAME.POD");
    if !game.exists() {
        eprintln!("skipping: {} is not there", game.display());
        return None;
    }
    let game = hb_pod::Pod::open(game).unwrap();
    let startup = hb_pod::Pod::open(dir.join("system/STARTUP.POD")).ok();
    Some(Level::load(&game, startup.as_ref(), stem).unwrap())
}

fn frame(level: &Level, x: i32, y: i32, z: i32) -> (Vec<u8>, hb_render::scene::Drawn) {
    let (w, h) = Target::MODE_200;
    let mut target = Target::new(w, h);
    target.clear(0);
    let mut camera = Camera::looking_at(x, y, z, Angle(0));
    camera.pitch = Angle(3000);
    let drawn = hb_render::draw_world(&mut target, &level.scene(), &camera);
    (target.colour, drawn)
}

/// The same place in the wrapping world, reached from either side, is the
/// same picture - and the ground is in it. Until this held, a camera at a
/// negative coordinate drew the terrain 1,024 units away and saw nothing.
#[test]
fn a_view_is_the_same_from_either_side_of_the_wrap() {
    let Some(level) = level("hoth") else { return };
    // The HOTH radar base's spike gun, from 40 units south and 13 above:
    // negative in both x and z.
    let (x, y, z) = (-172 << 16, 113 << 16, -260 << 16);
    let (signed, drawn) = frame(&level, x, y, z);
    let (unwrapped, _) = frame(&level, x + (1024 << 16), y, z + (1024 << 16));
    let differ = signed.iter().zip(&unwrapped).filter(|(a, b)| a != b).count();
    assert_eq!(differ, 0, "the two copies of the place disagree on {differ} pixels");
    assert!(drawn.ground > 1000 && drawn.models > 50, "{drawn:?}");
    // Below the horizon something was drawn nearly everywhere.
    let (w, h) = Target::MODE_200;
    let bottom = &signed[w * h * 3 / 4..];
    let empty = bottom.iter().filter(|&&c| c == 0).count();
    assert!(empty * 20 < bottom.len(), "{empty} of {} pixels empty", bottom.len());
}
