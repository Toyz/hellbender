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

#[test]
fn a_flash_brightens_a_colour_as_far_as_its_brightest_channel_allows() {
    use hb_render::level::brighten;
    assert_eq!(brighten([10, 20, 30]), [20, 40, 60]);
    // 200 cannot double: the factor stops where it reaches 255.
    assert_eq!(brighten([200, 100, 0]), [254, 127, 0]);
    assert_eq!(brighten([255, 255, 255]), [255, 255, 255]);
}

#[test]
fn a_flash_lights_iowahs_sky_and_its_bolt() {
    let Some(level) = level("iowah") else { return };
    let (Some(dim), Some(lit)) = (level.sky_remap.as_ref(), level.sky_remap_lit.as_ref()) else {
        panic!("iowah has a sky");
    };
    let luma = |i: u8| {
        let [r, g, b] = level.palette.rgb(i);
        29 * r as u32 + 58 * g as u32 + 15 * b as u32
    };
    let (d, l): (u32, u32) = (dim.iter().map(|&i| luma(i)).sum(), lit.iter().map(|&i| luma(i)).sum());
    println!("sky brightness {d} -> {l}");
    // `PURPSKY.ACT` tops out at a very dark purple, which `IOWA.ACT` can
    // only call black or `(14, 0, 0)`, and twice that is still nearest the
    // same red: the storm levels' sky hardly changes. Never darker.
    assert!(l >= d);

    // A frame mid-flash, with a bolt coming down in front of the eye. (A real
    // one starts at the sky layer, well above a level eye's view, and each
    // step goes nowhere or one unit toward -x and -z: `(rand() << 16) &
    // 0x1ffff` keeps only rand's lowest bit.)
    let grid = hb_world::Grid::new(&level.terrain);
    let ground = grid.ceiling_of_solid(0, 0);
    let mut camera = hb_render::Camera::looking_at(0, ground + (10 << 16), 0, hb_formats::Angle(0));
    camera.pitch = hb_formats::Angle(0);
    let mut scene = level.scene();
    scene.sun_ambient = 1.0;
    scene.sky_remap = Some(lit);
    let mut target = hb_render::Target::new(320, 200);
    target.clear(0);
    hb_render::draw_world(&mut target, &scene, &camera);
    let sky = (level.sky_height() * 65536.0) as i32;
    let _ = sky;
    let top = [10 << 16, ground + (40 << 16), 60 << 16];
    let segments = hb_sim::weather::bolt(top, |p| grid.ceiling_of_solid(p[0], p[2]), &mut Rng::new(4));
    let before = target.colour.clone();
    hb_render::scene::draw_bolt(&mut target, &camera, &segments);
    let drawn = before.iter().zip(&target.colour).filter(|(a, b)| a != b).count();
    println!("bolt: {} segments, {drawn} pixels", segments.len());
    let rgb: Vec<u8> = target.colour.iter().flat_map(|&i| level.palette.rgb(i)).collect();
    std::fs::write("/tmp/flash.png", hb_formats::png::rgb(320, 200, &rgb)).unwrap();
    assert!(drawn > 20);
}

#[test]
fn a_flash_would_light_a_sky_with_colour_in_it() {
    // `HOTH` has no lightning, but its sky has a real ramp to brighten.
    let Some(level) = level("hoth") else { return };
    let (dim, lit) = (level.sky_remap.unwrap(), level.sky_remap_lit.unwrap());
    let luma = |i: u8| {
        let [r, g, b] = level.palette.rgb(i);
        29 * r as u32 + 58 * g as u32 + 15 * b as u32
    };
    let used: Vec<u8> = level.sky.as_ref().unwrap().pixels.iter().copied().collect();
    let d: u32 = used.iter().map(|&t| luma(dim[t as usize])).sum();
    let l: u32 = used.iter().map(|&t| luma(lit[t as usize])).sum();
    println!("hoth sky {d} -> {l}");
    assert!(l > d + d / 2, "a flash should nearly double it");
}
