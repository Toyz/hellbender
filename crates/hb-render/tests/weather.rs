//! Snow and rain over a real level, drawn the way `hb-fly` draws them.

use hb_sim::turret::Rng;
use hb_sim::weather::{self, Weather};

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

/// A frame of `stem` from ten units above the middle of the map, after a
/// second of weather at thirty frames a second, flying forward a unit a
/// frame. Returns how many pixels the weather drew.
fn frame(stem: &str, out: &str) -> Option<usize> {
    let level = level(stem)?;
    let grid = hb_world::Grid::new(&level.terrain);
    let ground = grid.ceiling_of_solid(0, 0);
    let mut camera = hb_render::Camera::looking_at(0, ground + (10 << 16), 0, hb_formats::Angle(0));
    camera.pitch = hb_formats::Angle(0);
    let at = [camera.x, camera.y, camera.z];
    let mut w = Weather::new(at, &mut Rng::new(0x1996));
    let sky = (level.sky_height() * 65536.0) as i32;
    assert!(Weather::falls(at[1], sky, None), "{stem}: the eye is not where weather falls");
    for _ in 0..30 {
        Weather::step(&mut w.snow, 1.0 / 30.0, at);
        Weather::step(&mut w.rain, 1.0 / 30.0, at);
    }
    let mut target = hb_render::Target::new(320, 200);
    target.clear(0);
    let scene = level.scene();
    hb_render::draw_world(&mut target, &scene, &camera);
    let before = target.colour.clone();
    if level.manifest.has_snow() {
        for p in w.snow.iter().filter(|p| p.shown) {
            hb_render::scene::draw_flake(&mut target, &camera, p.position, weather::SNOW_INDEX);
        }
    }
    if level.manifest.has_rain() {
        for p in w.rain.iter().filter(|p| p.shown) {
            let end = Weather::streak_end(p, [0, 0, 1 << 16]);
            hb_render::scene::draw_streak(&mut target, &camera, p.position, end, weather::RAIN_INDEX);
        }
    }
    let rgb: Vec<u8> = target.colour.iter().flat_map(|&i| level.palette.rgb(i)).collect();
    std::fs::write(out, hb_formats::png::rgb(320, 200, &rgb)).unwrap();
    Some(before.iter().zip(&target.colour).filter(|(a, b)| a != b).count())
}

#[test]
fn it_rains_on_iowah() {
    let Some(n) = frame("iowah", "/tmp/rain.png") else { return };
    println!("rain drew {n} pixels");
    assert!(n > 100);
}

#[test]
fn it_snows_on_hoth() {
    let Some(n) = frame("hoth", "/tmp/snow.png") else { return };
    println!("snow drew {n} pixels");
    assert!(n > 20);
}

#[test]
fn nothing_falls_on_a_clear_level() {
    let Some(level) = level("jurasic") else { return };
    assert!(!level.manifest.has_snow() && !level.manifest.has_rain());
}
