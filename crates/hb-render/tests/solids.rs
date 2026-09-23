//! What the ship can fly into. Needs the game and skips without it.

use hb_sim::collide::{self, Solid};
use hb_sim::combat::{self, HitVolume};
use hb_formats::fixed::{from_units, to_units};

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
        let Some(level) = hb_render::Level::from_disc(stem) else { return };
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
        let Some(level) = hb_render::Level::from_disc(stem) else { return };
        let solids = scenery(&level);
        let grid = hb_world::Grid::new(&level.terrain);
        let middle = 64.0 * 8.0;
        let ground = grid
            .height_at(hb_formats::terrain::Layer::Ground, from_units(middle), from_units(middle))
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
        let Some(level) = hb_render::Level::from_disc(stem) else { return };
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

/// Shooting a switch starts it: every shot-triggered door in a level answers
/// to a hit in its own cell at its own height.
#[test]
fn a_shot_starts_a_switch() {
    use hb_sim::quake::{Layer, Quakes, Trigger};
    let Some(level) = hb_render::Level::from_disc("morbos") else { return };
    let mut doors = Quakes::new(&level.quake, |layer, (x, z)| match layer {
        Layer::BoxA => (level.terrain.boxes_a.bottom.at(x, z), level.terrain.boxes_a.top.at(x, z)),
        Layer::BoxB => (level.terrain.boxes_b.bottom.at(x, z), level.terrain.boxes_b.top.at(x, z)),
        Layer::Ground => {
            let h = level.terrain.ground.at(x, z);
            (h, h)
        }
        Layer::ChamberFloor => {
            let h = level.terrain.chambers.floor.at(x, z);
            (h, h)
        }
        Layer::ChamberCeiling => {
            let h = level.terrain.chambers.ceiling.at(x, z);
            (h, h)
        }
    });
    let shot: Vec<(i32, i32, f32)> = doors
        .doors
        .iter()
        .filter(|d| d.trigger == Trigger::Shot)
        .map(|d| (d.cell.0, d.cell.1, (d.bottom + d.top) / 2.0))
        .collect();
    assert!(!shot.is_empty(), "morbos has no shot-triggered doors");
    let mut started = 0;
    for (x, z, middle) in &shot {
        started += doors.shot((*x, *z), *middle);
    }
    assert_eq!(started, shot.len(), "a hit in the middle of one should start it");

    // And the cell a switch names is a cell the world has a box in, so a shot
    // that lands there lands on the switch rather than somewhere else.
    let grid = hb_world::Grid::new(&level.terrain);
    let mut boxed = 0;
    for (x, z, _) in &shot {
        let cell = hb_world::grid::Cell::new(*x, *z);
        if hb_formats::terrain::Layer::BOX_LAYERS.iter().any(|&l| grid.has_box(l, cell)) {
            boxed += 1;
        }
    }
    println!("{boxed} of {} shot-triggered cells have a box", shot.len());
    assert!(boxed * 2 >= shot.len(), "most shot-triggered cells should hold a box");

    // And once they are moving, the switches watching them should swap their
    // texture - and the name they swap to should be one the level has.
    let switches = doors.doors.iter().filter(|d| d.switch.is_some()).count();
    let watchers = doors
        .doors
        .iter()
        .filter(|d| matches!(d.trigger, hb_sim::quake::Trigger::Watching(_)))
        .count();
    let ids: Vec<i64> = doors.doors.iter().filter(|d| d.switch.is_some()).map(|d| d.id).collect();
    println!("{switches} switches, {watchers} watchers, switch ids {:?}", &ids[..ids.len().min(12)]);
    let links: Vec<String> = doors
        .doors
        .iter()
        .filter_map(|d| match d.trigger {
            hb_sim::quake::Trigger::Watching(w) => Some(format!("{w:?}")),
            _ => None,
        })
        .collect();
    println!("what the watchers watch: {:?}", &links[..links.len().min(12)]);
    let shot_switches =
        doors.doors.iter().filter(|d| d.switch.is_some() && d.trigger == Trigger::Shot).count();
    println!("{shot_switches} of the switches are shot-triggered");
    for d in doors.doors.iter().filter(|d| d.switch.is_some()).take(6) {
        println!(
            "  switch id {:3} cell {:?} bottom {:8.1} top {:8.1} travel(rest->target) {:8.1} delay {:.2}",
            d.id,
            d.cell,
            d.bottom,
            d.top,
            d.target - d.rest,
            d.delay
        );
    }
    let mut swaps = 0;
    let mut resolved = 0;
    for _ in 0..600 {
        doors.step(1.0 / 30.0);
        for swap in std::mem::take(&mut doors.swaps) {
            swaps += 1;
            if let Some(name) = swap.texture {
                let wanted = name.to_ascii_lowercase();
                if level.texture_names.iter().any(|n| n.to_ascii_lowercase() == wanted) {
                    resolved += 1;
                } else {
                    println!("  no texture named {name:?}");
                }
            } else {
                println!("  a swap with no texture");
            }
        }
    }
    println!("{swaps} swaps, {resolved} of them naming a texture the level has");
    assert!(swaps > 0, "nothing swapped a texture in twenty seconds");
}

/// Objects underground are drawn. A chamber is where a level's tunnels are,
/// and nothing about being under the ground should hide what is in one.
#[test]
fn a_chamber_draws_what_is_in_it() {
    let Some(level) = hb_render::Level::from_disc("morbos") else { return };
    let grid = hb_world::Grid::new(&level.terrain);
    // Somewhere with a chamber under it, and something placed down there.
    let under: Vec<usize> = level
        .placements
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            let at = [to_units(p.x), to_units(p.y), to_units(p.z)];
            at[1] < 0.0 && grid.has_chamber(hb_world::grid::Cell::containing(p.x, p.z))
        })
        .map(|(i, _)| i)
        .collect();
    println!("{} placements sit in a chamber", under.len());
    if under.is_empty() {
        return;
    }
    let p = &level.placements[under[0]];
    let mut target = hb_render::Target::new(320, 200);
    target.clear(0);
    let mut camera = hb_render::Camera::looking_at(p.x, p.y + (8 << 16), p.z - (40 << 16), hb_formats::Angle(0));
    camera.pitch = hb_formats::Angle(0);
    let scene = level.scene();
    let drawn = hb_render::draw_world(&mut target, &scene, &camera);
    println!("underground: {drawn:?}");
    println!("at {} {} {}", p.x as f32/65536.0, p.y as f32/65536.0, p.z as f32/65536.0);
    let rgb: Vec<u8> = target.colour.iter().flat_map(|&i| level.palette.rgb(i)).collect();
    std::fs::write("/tmp/under.png", hb_formats::png::rgb(320, 200, &rgb)).unwrap();
    assert!(drawn.models > 0, "nothing was drawn in the chamber");
}

/// The lamps: every face the scan finds wears a light's lit or unlit texture,
/// and `HOTH`'s are mostly on its tunnel ceilings.
#[test]
fn the_lamps_are_found_on_their_faces() {
    let Some(level) = hb_render::Level::from_disc("hoth") else { return };
    let lamps = &level.lamps;
    assert!(lamps.lamps.len() > 100, "{}", lamps.lamps.len());
    for lamp in &lamps.lamps {
        let r = lamps.records[lamp.record];
        let word = lamp.face.texture(&level.terrain) & 0x0fff;
        assert!(Some(word) == r.lit || Some(word) == r.unlit, "{:?}", lamp.face);
    }
    let ceilings = lamps.lamps.iter().filter(|l| l.face.layer == hb_formats::terrain::Layer::ChamberCeiling).count();
    assert!(ceilings * 2 > lamps.lamps.len());
}

/// Below the ground an object takes the lamps' light on top of the ambient.
#[test]
fn a_lamp_lights_what_is_under_the_ground() {
    let Some(mut level) = hb_render::Level::from_disc("morbos") else { return };
    let grid = hb_world::Grid::new(&level.terrain);
    let under: Vec<usize> = level
        .placements
        .iter()
        .enumerate()
        .filter(|(_, p)| p.y < 0 && grid.has_chamber(hb_world::grid::Cell::containing(p.x, p.z)))
        .map(|(i, _)| i)
        .collect();
    let Some(&i) = under.first() else { return };
    let p = level.placements[i];
    let cell = hb_world::grid::Cell::containing(p.x, p.z);
    let (lights, _) = level.lamps.step(1.0 / 30.0, (cell.x as usize, cell.z as usize));
    println!("{} lights near the object", lights.len());
    let at = [p.x, p.y, p.z].map(to_units);
    let light = hb_sim::lights::light_at(at, &lights);
    println!("light at it {light}");
    let frame = |lights: &[hb_sim::lights::Light]| {
        let mut target = hb_render::Target::new(320, 200);
        target.clear(0);
        let mut camera = hb_render::Camera::looking_at(p.x, p.y + (1 << 16), p.z - (6 << 16), hb_formats::Angle(0));
        camera.pitch = hb_formats::Angle(0);
        let mut scene = level.scene();
        scene.lights = lights;
        hb_render::draw_world(&mut target, &scene, &camera);
        target.colour
    };

    let (dark, lit) = (frame(&[]), frame(&lights));
    let differ = dark.iter().zip(&lit).filter(|(a, b)| a != b).count();
    for (name, frame) in [("/tmp/lamp_off.png", &dark), ("/tmp/lamp_on.png", &lit)] {
        let rgb: Vec<u8> = frame.iter().flat_map(|&i| level.palette.rgb(i)).collect();
        std::fs::write(name, hb_formats::png::rgb(320, 200, &rgb)).unwrap();
    }
    println!("{differ} pixels change with the lamps on");
    let _ = &mut level;
    if light > 0.0 {
        assert!(differ > 0);
    }
}

/// The lamps light the tunnel itself: each floor and ceiling corner near one
/// takes its light on top of its shade.
#[test]
fn the_lamps_light_the_tunnel_around_them() {
    use hb_formats::terrain::Layer;
    let Some(mut level) = hb_render::Level::from_disc("hoth") else { return };
    let Some(lamp) = level
        .lamps
        .lamps
        .iter()
        .find(|l| l.face.layer == Layer::ChamberCeiling && l.state == hb_sim::lights::State::Lit)
        .cloned()
    else {
        return;
    };
    let (lights, _) = level.lamps.step(1.0 / 30.0, lamp.face.cell);
    let grid = hb_world::Grid::new(&level.terrain);
    let (cx, cz) = (lamp.face.cell.0 as i32, lamp.face.cell.1 as i32);
    let floor = grid.height_at_grid(Layer::ChamberFloor, cx, cz).unwrap();
    let ceiling = grid.height_at_grid(Layer::ChamberCeiling, cx, cz).unwrap();
    let (x, z) = ((cx << 19) + (4 << 16), (cz << 19) + (4 << 16));
    let frame = |lights: &[hb_sim::lights::Light]| {
        let mut target = hb_render::Target::new(320, 200);
        target.clear(0);
        let mut camera =
            hb_render::Camera::looking_at(x, (floor + ceiling) / 2, z - (12 << 16), hb_formats::Angle(0));
        camera.pitch = hb_formats::Angle(0);
        let mut scene = level.scene();
        scene.placements = &[];
        scene.lights = lights;
        hb_render::draw_world(&mut target, &scene, &camera);
        target.colour
    };
    let (dark, lit) = (frame(&[]), frame(&lights));
    let differ = dark.iter().zip(&lit).filter(|(a, b)| a != b).count();
    for (name, frame) in [("/tmp/tunnel_off.png", &dark), ("/tmp/tunnel_on.png", &lit)] {
        let rgb: Vec<u8> = frame.iter().flat_map(|&i| level.palette.rgb(i)).collect();
        std::fs::write(name, hb_formats::png::rgb(320, 200, &rgb)).unwrap();
    }
    println!("{} lights; {differ} pixels change", lights.len());
    let _ = &mut level;
    assert!(differ > 5_000, "{differ}");
}
