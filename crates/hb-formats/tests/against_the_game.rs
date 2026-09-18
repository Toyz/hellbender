//! Every claim in `docs/formats/` that can be checked against the shipped
//! archives, checked against the shipped archives.
//!
//! These tests need the game. Point `HB_GAME` at the directory holding
//! `system/GAME.POD`; the default is `original/` at the repository root. When
//! the archives are not there the tests skip rather than fail, so the
//! workspace still builds and tests for someone without the disc.

use std::path::PathBuf;

use hb_formats::{act, colour, lvl, mrgl, raw, terrain, text};
use hb_pod::Pod;

fn game_dir() -> PathBuf {
    std::env::var_os("HB_GAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../original"))
}

fn pod(name: &str) -> Option<Pod> {
    let path = game_dir().join("system").join(name);
    if !path.exists() {
        eprintln!("skipping: {} is not there", path.display());
        return None;
    }
    Some(Pod::open(&path).expect("the archive should open"))
}

macro_rules! archive {
    ($name:expr) => {
        match pod($name) {
            Some(pod) => pod,
            None => return,
        }
    };
}

#[test]
fn both_archives_are_gapless() {
    // Every byte after the directory belongs to exactly one entry. If this
    // fails, the 40-byte entry stride or the 0x54 directory offset is wrong.
    for (name, count, comment) in [
        ("STARTUP.POD", 1206, "Startup Hellbender 1.0"),
        ("GAME.POD", 4341, "Game Pod - Hellbender Full Version - RC 1.0 JRS"),
    ] {
        let pod = archive!(name);
        assert_eq!(pod.entries().len(), count, "{name} entry count");
        assert_eq!(pod.comment(), comment, "{name} comment");
        assert!(pod.is_gapless(), "{name} has gaps or padding");
    }
}

#[test]
fn only_raw_entries_carry_a_palette_name() {
    let pod = archive!("GAME.POD");
    for e in pod.entries() {
        if !e.palette.is_empty() {
            assert_eq!(e.ext(), "raw", "{} carries a palette name", e.name);
        }
    }
    let with = pod.entries().iter().filter(|e| !e.palette.is_empty()).count();
    assert_eq!(with, 3314, "GAME.POD entries naming a palette");
}

#[test]
fn every_startup_texture_names_the_vga_palette() {
    let pod = archive!("STARTUP.POD");
    let named: Vec<&str> = pod
        .entries()
        .iter()
        .filter(|e| !e.palette.is_empty())
        .map(|e| e.palette.as_str())
        .collect();
    assert_eq!(named.len(), 574);
    assert!(named.iter().all(|p| *p == "VGA.ACT"));
}

#[test]
fn every_model_walks_to_the_byte() {
    // The MRGL size table is transcribed from a jump table in the binary. A
    // wrong size for any type that appears desynchronises the stream, so this
    // walking exactly is the whole proof that the table is right.
    let mut walked = 0;
    for name in ["STARTUP.POD", "GAME.POD"] {
        let pod = archive!(name);
        for e in pod.entries() {
            if e.dir() == "models" && e.ext() == "bin" {
                assert!(
                    mrgl::Model::walks_exactly(pod.bytes(e)),
                    "{}: MRGL stream does not end at end of file",
                    e.name
                );
                walked += 1;
            }
        }
    }
    assert_eq!(walked, 342, "models walked");
}

#[test]
fn cube_is_a_cube() {
    let pod = archive!("GAME.POD");
    let data = pod.read("models", "cube.bin").expect("cube.bin is in GAME.POD");
    assert_eq!(data.len(), 576);
    let model = mrgl::Model::parse(data).expect("cube.bin parses");
    assert_eq!(model.vertices.len(), 8);
    assert_eq!(model.materials, ["rustplat.raw"]);
    assert_eq!(model.polygons.len(), 6);
    assert!(model.polygons.iter().all(|p| p.corners.len() == 4));
    // The first face: normal (0, 0, -1), the four corners at z = -2595, and
    // the texture mapped corner to corner one texel in from each edge.
    let face = &model.polygons[0];
    assert_eq!(face.normal, [0, 0, -65536]);
    assert_eq!(
        face.corners.iter().map(|c| c.vertex).collect::<Vec<_>>(),
        [2, 3, 7, 6]
    );
    assert_eq!(
        face.corners.iter().map(|c| c.texel()).collect::<Vec<_>>(),
        [(1, 1), (255, 1), (255, 255), (1, 255)]
    );
    assert!(face.winding_matches(&model.vertices).unwrap());
    // A cube's six faces are the six axis directions.
    let mut normals: Vec<[i32; 3]> = model.polygons.iter().map(|p| p.normal).collect();
    normals.sort();
    assert_eq!(
        normals,
        [
            [-65536, 0, 0],
            [0, -65536, 0],
            [0, 0, -65536],
            [0, 0, 65536],
            [0, 65536, 0],
            [65536, 0, 0],
        ]
    );
    // Nodes: mesh start, vertex list, material, six polygons, end.
    assert_eq!(model.nodes.len(), 10);
    assert_eq!(model.nodes[0].kind, mrgl::MESH_START);
    assert_eq!(model.nodes.last().unwrap().kind, mrgl::END);
}

#[test]
fn a_group_node_names_its_children() {
    let pod = archive!("STARTUP.POD");
    let data = pod.read("models", "aliensh.bin").expect("aliensh.bin");
    assert_eq!(data.len(), 348, "the 344-byte group node plus the 4-byte end node");
    let model = mrgl::Model::parse(data).expect("aliensh.bin parses");
    assert_eq!(model.children.len(), 8);
    assert_eq!(model.children[0], "aliensh1.bin");
    assert_eq!(model.children[7], "aliensh8.bin");
    // The node's 344 bytes are fully accounted for: 24 of header, sixteen
    // 16-byte name slots to 0x118, then sixteen 4-byte pointer slots to 0x158.
    assert_eq!(
        mrgl::GROUP_POINTERS + mrgl::GROUP_CHILDREN * 4,
        344,
        "the group node has no unexplained tail"
    );
    // Only the first eight name slots are used and the rest are zero.
    assert!(data[0x98..0x158].iter().all(|&b| b == 0));
    // Every child is really in the archive.
    for child in &model.children {
        assert!(pod.find("models", child).is_some(), "{child} is missing");
    }
}

#[test]
fn polygons_carry_a_unit_normal_and_texel_coordinates() {
    let (mut unit, mut total) = (0usize, 0usize);
    let (mut wound, mut reversed) = (0usize, 0usize);
    for name in ["STARTUP.POD", "GAME.POD"] {
        let pod = archive!(name);
        for e in pod.entries() {
            if e.dir() != "models" || e.ext() != "bin" {
                continue;
            }
            let model = mrgl::Model::parse(pod.bytes(e)).unwrap();
            for poly in &model.polygons {
                total += 1;
                // Three or four corners, never anything else.
                assert!(
                    (3..=4).contains(&poly.corners.len()),
                    "{}: a polygon with {} corners",
                    e.name,
                    poly.corners.len()
                );
                for corner in &poly.corners {
                    // Every index resolves, and every texture coordinate is a
                    // whole texel in 0..=255.
                    assert!(
                        (corner.vertex as usize) < model.vertices.len(),
                        "{}: vertex index {} of {}",
                        e.name,
                        corner.vertex,
                        model.vertices.len()
                    );
                    assert_eq!(corner.u & 0xffff, 0, "{}: u is not a whole texel", e.name);
                    assert_eq!(corner.v & 0xffff, 0, "{}: v is not a whole texel", e.name);
                    assert!((0..=255 << 16).contains(&corner.u), "{}: u out of range", e.name);
                    assert!((0..=255 << 16).contains(&corner.v), "{}: v out of range", e.name);
                }
                let length: f64 = poly
                    .normal
                    .iter()
                    .map(|&n| (n as f64 / 65536.0).powi(2))
                    .sum::<f64>()
                    .sqrt();
                if (length - 1.0).abs() < 0.01 {
                    unit += 1;
                }
                match poly.winding_matches(&model.vertices) {
                    Some(true) => wound += 1,
                    Some(false) => reversed += 1,
                    None => {}
                }
            }
        }
    }
    assert_eq!(total, 33_728, "polygons across both archives");
    // Eighty have a normal that is not unit length.
    assert_eq!(total - unit, 80, "polygons with a non-unit normal");
    // 87 have collinear first corners and no winding to speak of. Of the rest,
    // exactly two disagree with their own normal.
    assert_eq!(wound + reversed, 33_641, "polygons with a testable winding");
    assert_eq!(wound, 33_639, "polygons wound along their normal");
    assert_eq!(reversed, 2, "polygons wound against their normal");
}

#[test]
fn every_palette_is_768_bytes_or_empty() {
    for name in ["STARTUP.POD", "GAME.POD"] {
        let pod = archive!(name);
        for e in pod.entries().iter().filter(|e| e.ext() == "act") {
            act::Palette::parse_lenient(pod.bytes(e))
                .unwrap_or_else(|why| panic!("{}: {why}", e.name));
        }
    }
}

#[test]
fn index_zero_is_the_transparent_colour() {
    let pod = archive!("STARTUP.POD");
    let image = raw::Image::parse_guessed(pod.read("art", "ckpt200.raw").unwrap())
        .unwrap()
        .expect("ckpt200.raw has pixels");
    assert_eq!(image.shape, raw::Shape::new(320, 200));
    // The viewport the cockpit frames is index 0 and nothing else.
    for y in 60..120 {
        for x in 40..280 {
            assert_eq!(image.pixels[y * 320 + x], 0, "pixel at {x},{y}");
        }
    }
    let palette = act::Palette::parse(pod.read("art", "vga.act").unwrap()).unwrap();
    assert_eq!(palette.rgb(0), [0, 0, 0]);
}

#[test]
fn all_26_levels_parse_and_name_files_that_exist() {
    let pod = archive!("GAME.POD");
    let levels: Vec<_> =
        pod.entries().iter().filter(|e| e.ext() == "lvl").cloned().collect();
    assert_eq!(levels.len(), 26);
    for e in &levels {
        let level = lvl::Level::parse(pod.bytes(e))
            .unwrap_or_else(|why| panic!("{}: {why}", e.name));
        assert_eq!(level.version, lvl::VERSION, "{}", e.name);
        assert_eq!(level.files.len(), 16, "{}", e.name);
        for ((slot, dir), file) in lvl::DIRS.iter().zip(&level.files) {
            assert!(
                pod.find(dir, file).is_some(),
                "{}: {slot} names {dir}\\{file}, which is not in the archive",
                e.name
            );
        }
        // Line 42 is the same constant in every shipped level.
        assert_eq!(level.weather_params[1], [983040, 1966080], "{}", e.name);
    }
}

#[test]
fn the_weather_field_matches_the_levels_it_is_set_on() {
    let pod = archive!("GAME.POD");
    let weather = |stem: &str| {
        let data = pod.read("levels", &format!("{stem}.lvl")).unwrap();
        lvl::Level::parse(data).unwrap().weather
    };
    for snowy in ["hoth", "hoth2", "hoth3"] {
        assert_eq!(weather(snowy), 1, "{snowy} should be snow");
    }
    for wet in ["iowah", "iowah2", "iowah3", "morbos", "morbos2", "morbos3"] {
        assert_eq!(weather(wet), 6, "{wet} should be rain and lightning");
    }
    assert_eq!(weather("float"), 0);
}

#[test]
fn every_level_loads_its_thirteen_terrain_grids() {
    let pod = archive!("GAME.POD");
    let stems: Vec<String> = pod
        .entries()
        .iter()
        .filter(|e| e.ext() == "lvl")
        .map(|e| {
            let data = pod.bytes(e);
            lvl::Level::parse(data).unwrap().stem().to_string()
        })
        .collect();
    assert_eq!(stems.len(), 26);
    for stem in &stems {
        let t = terrain::Terrain::load(|ext| {
            pod.read("data", &format!("{stem}.{ext}")).ok().map(<[u8]>::to_vec)
        })
        .unwrap_or_else(|why| panic!("{stem}: {why}"));
        assert_eq!(t.ground.values.len(), terrain::CELLS);
        assert_eq!(t.colour.values.len(), terrain::CELLS);
        assert_eq!(t.boxes_a.textures.values.len(), terrain::CELLS * terrain::BOX_TEXTURES);
        assert_eq!(
            t.chambers.textures.values.len(),
            terrain::CELLS * terrain::CHAMBER_TEXTURES
        );
        // The ground and box set A scale upward: 0 to 255 << 7, in steps of
        // 128. Chambers and box set B scale downward and are never positive.
        for up in [&t.ground, &t.boxes_a.bottom, &t.boxes_a.top] {
            assert_eq!(up.scale, terrain::Scale::Up, "{stem}");
            assert!(up.values.iter().all(|&v| (0..=255 << 7).contains(&v)), "{stem}");
            assert!(up.values.iter().all(|&v| v % 128 == 0), "{stem}");
        }
        for down in [
            &t.chambers.floor,
            &t.chambers.ceiling,
            &t.boxes_b.bottom,
            &t.boxes_b.top,
        ] {
            assert_eq!(down.scale, terrain::Scale::Down, "{stem}");
            assert!(down.values.iter().all(|&v| (-(255 << 7)..=0).contains(&v)), "{stem}");
            assert!(down.values.iter().all(|&v| v % 128 == 0), "{stem}");
        }
    }
}

#[test]
fn the_ramps_are_sixteen_rows_that_fade_to_opposite_ends() {
    let pod = archive!("GAME.POD");
    let lte = colour::Ramp::parse(pod.read("fog", "float.lte").unwrap()).unwrap();
    let fog = colour::Ramp::parse(pod.read("fog", "float.fog").unwrap()).unwrap();
    for ramp in [&lte, &fog] {
        assert_eq!(ramp.levels, colour::Ramp::LEVELS);
        // Row 0 is the identity in both.
        for i in 0..=255u8 {
            assert_eq!(ramp.shade(0, i), i);
        }
    }
    // .LTE fades the shadeable range to black and leaves the reserved indices
    // exactly as they are, at every level.
    assert!((0..colour::SHADED).all(|i| lte.shade(15, i as u8) == 0));
    for level in 0..colour::Ramp::LEVELS {
        for i in colour::SHADED..256 {
            assert_eq!(lte.shade(level, i as u8), i as u8, "level {level}, index {i}");
        }
    }
    // .FOG makes no exception: everything becomes the fog colour.
    assert!((0..=255u8).all(|i| fog.shade(15, i) == 255));
}

#[test]
fn the_blend_table_is_symmetric_and_refuses_the_reserved_indices() {
    let pod = archive!("STARTUP.POD");
    let mix = colour::BlendTable::parse(pod.read("fog", "vga.mix").unwrap()).unwrap();
    assert!(mix.is_symmetric());
    // Blending with index 0 is a no-op over the shadeable range.
    for i in 0..colour::SHADED {
        assert_eq!(mix.blend(0, i as u8), i as u8);
    }
    // The reserved indices are not blendable at all.
    for i in colour::SHADED..256 {
        assert_eq!(mix.blend(0, i as u8), 0, "index {i}");
    }
}

#[test]
fn the_colour_map_resolves_to_a_plausible_colour() {
    let pod = archive!("STARTUP.POD");
    let map = colour::ColourMap::parse(pod.read("fog", "vga.map").unwrap()).unwrap();
    let palette = act::Palette::parse(pod.read("art", "vga.act").unwrap()).unwrap();
    // Not an exact nearest match - the table was built by something cheaper
    // than a full search, and individual colours can be far out - so the claim
    // is about the average, which was measured at about 3,500 against an
    // exhaustive search's 1,800.
    let (mut total, mut samples) = (0i64, 0i64);
    for r in (0..256).step_by(17) {
        for g in (0..256).step_by(17) {
            for b in (0..256).step_by(17) {
                let [pr, pg, pb] = palette.rgb(map.lookup(r as u8, g as u8, b as u8));
                let d = (pr as i32 - r).pow(2) + (pg as i32 - g).pow(2) + (pb as i32 - b).pow(2);
                total += d as i64;
                samples += 1;
            }
        }
    }
    let mean = total / samples;
    assert!(mean < 6_000, "mean squared distance was {mean}");
}

#[test]
fn every_level_lists_textures_that_exist_and_enemies_that_have_models() {
    let pod = archive!("GAME.POD");
    let mut enemies = 0usize;
    for e in pod.entries().iter().filter(|e| e.ext() == "lvl") {
        let level = lvl::Level::parse(pod.bytes(e)).unwrap();
        let stem = level.stem().to_string();

        let tex = pod.read("data", &format!("{stem}.tex")).unwrap();
        let names = text::name_list(tex, "TEX").unwrap_or_else(|w| panic!("{stem}: {w}"));
        for name in &names {
            assert!(pod.find("art", name).is_some(), "{stem}: texture {name} is missing");
        }

        let def = pod.read("data", &format!("{stem}.def")).unwrap();
        let defs = text::enemy_defs(def).unwrap_or_else(|w| panic!("{stem}: {w}"));
        for d in &defs {
            assert!(pod.find("models", &d.model).is_some(), "{stem}: {} missing", d.model);
            assert!(pod.find("models", &d.wreck).is_some(), "{stem}: {} missing", d.wreck);
            assert!(d.damage.iter().all(|&v| v >= 0));
        }
        enemies += defs.len();
    }
    assert_eq!(enemies, 1848, "enemy and object definitions across all levels");
}
