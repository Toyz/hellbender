//! Every claim in `docs/formats/` that can be checked against the shipped
//! archives, checked against the shipped archives.
//!
//! These tests need the game. Point `HB_GAME` at the directory holding
//! `system/GAME.POD`; the default is `original/` at the repository root. When
//! the archives are not there the tests skip rather than fail, so the
//! workspace still builds and tests for someone without the disc.

use std::path::PathBuf;

use hb_formats::{act, anim, colour, course, font, lvl, mrgl, raw, terrain, text};
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
fn every_polygon_binds_to_a_material_that_exists() {
    let (mut bound, mut unbound) = (0usize, 0usize);
    for name in ["STARTUP.POD", "GAME.POD"] {
        let pod = archive!(name);
        for e in pod.entries() {
            if e.dir() != "models" || e.ext() != "bin" {
                continue;
            }
            let model = mrgl::Model::parse(pod.bytes(e)).unwrap();
            for poly in &model.polygons {
                match poly.material {
                    Some(m) => {
                        assert!(
                            m < model.materials.len(),
                            "{}: material {m} of {}",
                            e.name,
                            model.materials.len()
                        );
                        bound += 1;
                    }
                    None => unbound += 1,
                }
            }
        }
    }
    // Seven models are untextured throughout and account for every polygon
    // that has no material before it. They carry a flat-colour node instead.
    assert_eq!(unbound, 1_030, "polygons with no material before them");
    assert_eq!(bound + unbound, 33_728);
}

#[test]
fn an_untextured_polygon_carries_a_flat_colour_instead() {
    let mut untextured: Vec<(String, usize)> = Vec::new();
    let mut bare_and_colourless: Vec<(String, usize)> = Vec::new();
    for name in ["STARTUP.POD", "GAME.POD"] {
        let pod = archive!(name);
        for e in pod.entries() {
            if e.dir() != "models" || e.ext() != "bin" {
                continue;
            }
            let model = mrgl::Model::parse(pod.bytes(e)).unwrap();
            let bare = model
                .polygons
                .iter()
                .filter(|p| p.material.is_none())
                .collect::<Vec<_>>();
            if bare.is_empty() {
                continue;
            }
            // Where a colour is given it is in the palette's shadeable range
            // once the flag bit is masked off.
            for poly in &bare {
                if let Some(colour) = poly.colour {
                    assert!((colour & 0xff) < 240, "{}: colour {colour}", e.name);
                }
            }
            let colourless = bare.iter().filter(|p| p.colour.is_none()).count();
            untextured.push((e.file_name(), bare.len()));
            if colourless > 0 {
                bare_and_colourless.push((e.file_name(), colourless));
            }
        }
    }
    untextured.sort();
    assert_eq!(
        untextured,
        [
            ("fanbody.bin".to_string(), 28),
            ("globe.bin".to_string(), 576),
            ("iris1.bin".to_string(), 168),
            ("iris4.bin".to_string(), 168),
            ("jaw1.bin".to_string(), 30),
            ("jaw2.bin".to_string(), 28),
            ("shell.bin".to_string(), 32),
        ]
    );

    // Four of those seven have polygons with no colour node before them
    // either, so nothing in the stream says what colour they are. Pinned as a
    // measurement: 118 polygons that a renderer has to decide about.
    bare_and_colourless.sort();
    assert_eq!(
        bare_and_colourless,
        [
            ("fanbody.bin".to_string(), 28),
            ("jaw1.bin".to_string(), 30),
            ("jaw2.bin".to_string(), 28),
            ("shell.bin".to_string(), 32),
        ]
    );
}

#[test]
fn every_animated_model_parses_to_the_last_line() {
    let pod = archive!("GAME.POD");
    let (mut files, mut parts, mut frames, mut polys) = (0usize, 0usize, 0usize, 0usize);
    let mut bare = 0usize;
    for e in pod.entries() {
        if e.dir() != "models" || e.ext() != "txt" {
            continue;
        }
        let model = anim::parse(pod.bytes(e))
            .unwrap_or_else(|why| panic!("{}: {why}", e.name));
        files += 1;
        parts += model.parts.len();
        frames += model.frames;
        polys += model.polygon_count();
        assert!(model.frames > 0 && !model.parts.is_empty(), "{}", e.name);
        for part in &model.parts {
            // Every part carries a keyframe per frame, in both lists.
            assert_eq!(part.angles.len(), model.frames, "{} {}", e.name, part.name);
            assert_eq!(part.centres.len(), model.frames, "{} {}", e.name, part.name);
            // A parent is either the root marker or an earlier part.
            assert!(part.parent == -1 || (part.parent as usize) < model.parts.len(),
                "{}: {} has parent {}", e.name, part.name, part.parent);
            for poly in &part.polygons {
                assert!((3..=4).contains(&poly.corners.len()), "{}", e.name);
                for c in &poly.corners {
                    assert!(
                        (c.vertex as usize) < part.vertices.len(),
                        "{} {}: vertex {} of {}",
                        e.name, part.name, c.vertex, part.vertices.len()
                    );
                }
                match poly.material {
                    Some(m) => assert!(m < model.materials.len(), "{}: material {m}", e.name),
                    None => bare += 1,
                }
            }
        }
    }
    assert_eq!(files, 18, "animated models in GAME.POD");
    assert_eq!(parts, 219);
    assert_eq!(frames, 917, "keyframes summed over the 18 models");
    assert_eq!(polys, 4_392);
    // Twelve faces use the 255 sentinel instead of naming a material.
    assert_eq!(bare, 12, "faces with no material");
}

#[test]
fn an_animated_models_rest_pose_is_its_raw_vertices() {
    let pod = archive!("GAME.POD");
    let trex = anim::parse(pod.read("models", "trex.txt").unwrap()).unwrap();
    // A Tyrannosaurus with twenty parts and sixty-one keyframes.
    assert_eq!(trex.parts.len(), 20);
    assert_eq!(trex.frames, 61);
    assert_eq!(trex.materials.len(), 43);
    let (vertices, polygons) = trex.rest_pose();
    assert_eq!(vertices.len(), trex.vertex_count());
    assert_eq!(polygons.len(), trex.polygon_count());
    // The pose rebases each part's corner indices onto the merged list.
    for poly in &polygons {
        for c in &poly.corners {
            assert!((c.vertex as usize) < vertices.len());
        }
    }
    // Its body, mid sections and tail chain along x, which is how you can tell
    // the raw vertices are already in model space.
    let span = |p: &anim::Part| {
        let lo = p.vertices.iter().map(|v| v.x).min().unwrap();
        let hi = p.vertices.iter().map(|v| v.x).max().unwrap();
        (lo, hi)
    };
    let (_, front_hi) = span(&trex.parts[0]);
    let (mid2_lo, _) = span(&trex.parts[2]);
    let (tail_lo, _) = span(&trex.parts[3]);
    assert!(front_hi < mid2_lo, "the body should end before the mid section");
    assert!(mid2_lo < tail_lo, "the mid section should come before the tail");
}

#[test]
fn animated_models_normalise_to_twice_the_binary_ones() {
    let pod = archive!("GAME.POD");
    let mut checked = 0;
    for e in pod.entries() {
        if e.dir() != "models" || e.ext() != "txt" {
            continue;
        }
        let model = anim::parse(pod.bytes(e)).unwrap();
        let reach = model
            .parts
            .iter()
            .flat_map(|p| p.vertices.iter())
            .flat_map(|v| [v.x.abs(), v.y.abs(), v.z.abs()])
            .max()
            .unwrap();
        // Raw, with no part offsets applied, every model fills exactly the
        // normalisation box - which is twice the binary models' 16,384.
        assert_eq!(reach, anim::MODEL_ONE, "{}", e.name);
        checked += 1;
    }
    assert_eq!(checked, 18);
    assert_eq!(anim::MODEL_ONE, 2 * mrgl::MODEL_ONE - 1);
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
fn a_texture_word_is_twelve_bits_of_index_and_four_of_orientation() {
    let pod = archive!("GAME.POD");
    let (mut cells, mut raw_out_of_range, mut oriented) = (0usize, 0usize, 0usize);
    for e in pod.entries().iter().filter(|e| e.ext() == "lvl") {
        let level = lvl::Level::parse(pod.bytes(e)).unwrap();
        let stem = level.stem().to_string();
        let names =
            text::name_list(pod.read("data", &format!("{stem}.tex")).unwrap(), "TEX").unwrap();
        let clr = terrain::Indices::parse(
            pod.read("data", &format!("{stem}.clr")).unwrap(),
            1,
            "ground colour",
        )
        .unwrap();
        for &word in &clr.values {
            let texture = terrain::TextureRef(word);
            cells += 1;
            // The masked index always resolves; the raw word does not.
            assert!(
                (texture.index() as usize) < names.len(),
                "{stem}: index {} of {}",
                texture.index(),
                names.len()
            );
            if word as usize >= names.len() {
                raw_out_of_range += 1;
            }
            if !texture.is_upright() {
                oriented += 1;
            }
        }
    }
    assert_eq!(cells, 26 * terrain::CELLS);
    // If the raw word were the index, this many cells would point past the end
    // of their level's texture list. That is what makes the 12-bit mask a
    // measurement rather than a reading of the disassembly alone.
    assert_eq!(raw_out_of_range, 3_658);
    assert_eq!(oriented, 3_658);
}

#[test]
fn every_course_parses_and_stays_inside_the_world() {
    let pod = archive!("GAME.POD");
    let (mut files, mut courses, mut points) = (0usize, 0usize, 0usize);
    let (mut kind_points, mut kind_segments) = (0usize, 0usize);
    let (mut max_x, mut max_z, mut min_y, mut max_y) = (0i32, 0i32, 0i32, 0i32);
    for e in pod.entries().iter().filter(|e| e.ext() == "crs") {
        files += 1;
        let parsed = course::parse(pod.bytes(e))
            .unwrap_or_else(|why| panic!("{}: {why}", e.name));
        for c in &parsed {
            courses += 1;
            match c {
                course::Course::Points { .. } => kind_points += 1,
                course::Course::Segments { .. } => kind_segments += 1,
            }
            for p in c.points() {
                points += 1;
                max_x = max_x.max(p.x.abs());
                max_z = max_z.max(p.z.abs());
                min_y = min_y.min(p.y);
                max_y = max_y.max(p.y);
            }
        }
    }
    assert_eq!(files, 24, "levels with a .CRS");
    assert_eq!(courses, 90);
    assert_eq!(kind_points, 86, "point-list courses");
    assert_eq!(kind_segments, 4, "segment courses");
    // A segment course's segments meet end to start, so the shared endpoints
    // are counted once: 16 segments across the four of them give 20 points,
    // not 32.
    assert_eq!(points, 1_648);

    // The world is 1024 units square and centred, so no course leaves it.
    let half = 512 << 16;
    assert!(max_x < half, "x reaches {} units", max_x >> 16);
    assert!(max_z < half, "z reaches {} units", max_z >> 16);
    // And it really is centred rather than starting at zero: courses use the
    // negative half as much as the positive one.
    assert!(max_x > (500 << 16), "x only reaches {} units", max_x >> 16);
    // Height stays within the terrain's own span plus the structures on it.
    assert!(min_y > -(200 << 16) && max_y < (200 << 16), "{min_y}..{max_y}");
}

#[test]
fn the_def_file_also_holds_the_instance_list() {
    let pod = archive!("GAME.POD");
    let (mut levels, mut instances) = (0usize, 0usize);
    let (mut max_xz, mut min_y, mut max_y) = (0i32, 0i32, 0i32);
    for e in pod.entries().iter().filter(|e| e.ext() == "lvl") {
        let level = lvl::Level::parse(pod.bytes(e)).unwrap();
        let stem = level.stem().to_string();
        let def = pod.read("data", &format!("{stem}.def")).unwrap();
        let types = text::enemy_defs(def).unwrap();
        let placed = text::placements(def)
            .unwrap_or_else(|why| panic!("{stem}: {why}"));
        levels += 1;
        instances += placed.len();
        // The engine caps the list at 500.
        assert!(placed.len() <= 500, "{stem} places {}", placed.len());
        for p in &placed {
            assert!(p.kind < types.len(), "{stem}: type {} of {}", p.kind, types.len());
            // Always zero in the shipped data.
            assert_eq!(p.unknown_5, 0);
            assert_eq!(p.unknown_6, 0);
            max_xz = max_xz.max(p.x.abs()).max(p.z.abs());
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
    }
    assert_eq!(levels, 26);
    assert_eq!(instances, 7_606);
    // The world's own bounds: objects reach to within a hundredth of a unit of
    // 512 either side of the origin, and never past it.
    assert!(max_xz < 512 << 16, "{} units", max_xz >> 16);
    assert!(max_xz > 511 << 16, "only reaches {} units", max_xz >> 16);
    // And the terrain's own vertical span: the top is exactly 127.5 units,
    // which is altitude byte 255 scaled by 2^15.
    assert_eq!(max_y, 255 << 15);
    assert!(min_y >= -(126 << 16), "{} units", min_y >> 16);
}

#[test]
fn every_placed_object_has_a_model_in_the_archives() {
    let pod = archive!("GAME.POD");
    let mut placed_models = 0usize;
    for e in pod.entries().iter().filter(|e| e.ext() == "lvl") {
        let level = lvl::Level::parse(pod.bytes(e)).unwrap();
        let stem = level.stem().to_string();
        let def = pod.read("data", &format!("{stem}.def")).unwrap();
        let types = text::enemy_defs(def).unwrap();
        for p in text::placements(def).unwrap() {
            let kind = &types[p.kind];
            assert!(
                pod.find("models", &kind.model).is_some(),
                "{stem}: placed {} which is not in the archive",
                kind.model
            );
            placed_models += 1;
        }
    }
    assert_eq!(placed_models, 7_606);
}

#[test]
fn the_font_is_ninety_five_glyphs_twenty_three_tall() {
    let pod = archive!("STARTUP.POD");
    let index = pod.read("startup", "font.ndx").unwrap();
    let bitmap = pod.read("startup", "font.bin").unwrap();
    let font = font::Font::parse(index, bitmap).expect("the font parses");

    // One glyph per printable ASCII character, and the height falls out of
    // the arithmetic: the widths sum to 1,137 and the bitmap is 26,151.
    assert_eq!(font.widths.len(), font::GLYPHS);
    assert_eq!(font.widths.iter().sum::<usize>(), 1_137);
    assert_eq!(bitmap.len(), 1_137 * font::HEIGHT);
    assert_eq!(font::HEIGHT, 23);

    // Every printable character has a glyph of its stated width, and space is
    // blank while a letter is not.
    for ch in ' '..='~' {
        let (glyph, w) = font.glyph(ch).unwrap_or_else(|| panic!("no glyph for {ch:?}"));
        assert_eq!(glyph.len(), w * font::HEIGHT, "{ch:?}");
        assert!(w >= 3, "{ch:?} is {w} wide");
    }
    assert!(font.glyph(' ').unwrap().0.iter().all(|&p| p == 0));
    assert!(font.glyph('A').unwrap().0.iter().any(|&p| p != 0));
    assert_eq!(font.glyph('A').unwrap().1, 15);
    assert_eq!(font.glyph('H').unwrap().1, 17);
    assert!(font.glyph('\u{7f}').is_none());

    // The ink is almost all index 255 - the reserved range, which the shade
    // and blend tables leave alone. A typeface has to come out the colour it
    // was drawn in.
    let ink = font.pixels.iter().filter(|&&p| p != 0).count();
    let reserved = font.pixels.iter().filter(|&&p| p >= 240).count();
    assert!(
        reserved * 100 / ink > 40,
        "only {}% of the ink is in the reserved range",
        reserved * 100 / ink
    );
}

#[test]
fn animation_frames_mostly_live_outside_the_levels_texture_list() {
    let pod = archive!("GAME.POD");
    let (mut total, mut outside, mut files) = (0usize, 0usize, 0usize);
    for e in pod.entries().iter().filter(|e| e.ext() == "ani") {
        let stem = e.file_name().trim_end_matches(".ani").to_string();
        let Ok(tex) = pod.read("data", &format!("{stem}.tex")) else { continue };
        let names = text::name_list(tex, "TEX").unwrap();
        let listed = |n: &str| names.iter().any(|m| m.eq_ignore_ascii_case(n));

        let cycles = text::animations(pod.bytes(e))
            .unwrap_or_else(|why| panic!("{}: {why}", e.name));
        files += 1;
        for cycle in &cycles {
            total += 1;
            assert!(cycle.frames.len() >= 2, "{}: a cycle of one frame", e.name);
            assert!(cycle.delay > 0, "{}: a cycle with no delay", e.name);
            // Every frame is in the archive even when it is not in the .TEX.
            for frame in &cycle.frames {
                assert!(
                    pod.find("art", frame).is_some(),
                    "{}: frame {frame} is not in the archive",
                    e.name
                );
            }
            if !listed(&cycle.base) || cycle.frames.iter().any(|f| !listed(f)) {
                outside += 1;
            }
        }
    }
    assert_eq!(files, 11, "levels with an .ANI");
    assert_eq!(total, 146);
    // The texture list is the terrain's set; animation frames are separate
    // textures the engine registers into the same 1,024-entry table.
    assert_eq!(outside, 140);
}

#[test]
fn a_sky_palette_is_a_gradient_band_and_the_texture_indexes_it_biased() {
    let pod = archive!("GAME.POD");
    let mut with_texture = 0usize;
    for e in pod.entries().iter().filter(|e| e.ext() == "lvl") {
        let level = lvl::Level::parse(pod.bytes(e)).unwrap();
        let (dir, sky) = level.slot("sky").unwrap();
        let Ok(bytes) = pod.read(dir, sky) else { continue };
        // Six levels name a zero-length .VOX: those are the ones set in space.
        let Some(texture) = raw::Image::parse_guessed(bytes).unwrap() else {
            continue;
        };
        assert_eq!(texture.shape, raw::Shape::new(64, 64), "{}", e.name);
        let (dir, name) = level.slot("sky_palette").unwrap();
        let palette = act::Palette::parse(pod.read(dir, name).unwrap()).unwrap();

        // The palette is a band starting at 192 and black elsewhere.
        for i in 0..192u16 {
            assert_eq!(palette.rgb(i as u8), [0, 0, 0], "{} entry {i}", e.name);
        }
        // Every index the texture uses lands inside 192..=223 once biased.
        for &index in &texture.pixels {
            let entry = index.wrapping_sub(48);
            assert!(
                (192..=223).contains(&entry),
                "{}: texture index {index} biases to {entry}",
                e.name
            );
        }
        with_texture += 1;
    }
    assert_eq!(with_texture, 20, "levels with a sky texture");
}

#[test]
fn a_level_palette_is_vga_with_sixteen_entries_changed() {
    let game = archive!("GAME.POD");
    let startup = archive!("STARTUP.POD");
    let vga = act::Palette::parse(startup.read("art", "vga.act").unwrap()).unwrap();
    let mut levels = 0;
    for e in game.entries().iter().filter(|e| e.ext() == "lvl") {
        let level = lvl::Level::parse(game.bytes(e)).unwrap();
        let (dir, name) = level.slot("ground_palette").unwrap();
        let Ok(bytes) = game.read(dir, name).or_else(|_| startup.read(dir, name)) else {
            continue;
        };
        let palette = act::Palette::parse(bytes).unwrap();
        let same = (0..256).filter(|&i| palette.rgb(i as u8) == vga.rgb(i as u8)).count();
        // Close enough to VGA.ACT that art drawn in one can be blitted into a
        // frame in the other, which is what lets the cockpit work.
        assert!(same >= 200, "{}: only {same} entries match VGA.ACT", e.name);
        // And the reserved range matches exactly, in every level.
        for i in 240..256u16 {
            assert_eq!(palette.rgb(i as u8), vga.rgb(i as u8), "{} entry {i}", e.name);
        }
        levels += 1;
    }
    assert_eq!(levels, 26);
}

#[test]
fn a_colour_map_is_named_after_its_palette_not_its_level() {
    let pod = archive!("GAME.POD");
    let (mut by_palette, mut by_level, mut levels) = (0, 0, 0);
    for e in pod.entries().iter().filter(|e| e.ext() == "lvl") {
        let level = lvl::Level::parse(pod.bytes(e)).unwrap();
        let (_, palette) = level.slot("ground_palette").unwrap();
        let palette_stem = palette.split('.').next().unwrap();
        if pod.find("fog", &format!("{palette_stem}.map")).is_some() {
            by_palette += 1;
        }
        if pod.find("fog", &format!("{}.map", level.stem())).is_some() {
            by_level += 1;
        }
        levels += 1;
    }
    assert_eq!(levels, 26);
    // A .MAP answers "nearest index in this palette", so it follows the
    // palette. Named after the level, only 10 would be found.
    assert_eq!(by_palette, 26);
    assert_eq!(by_level, 10);
}

#[test]
fn ground_textures_never_touch_the_reserved_range() {
    let pod = archive!("GAME.POD");
    let names = text::name_list(pod.read("data", "hoth.tex").unwrap(), "TEX").unwrap();
    let (mut pixels, mut highest) = (0usize, 0u8);
    for name in &names {
        let Ok(bytes) = pod.read("art", name) else { continue };
        for &index in bytes {
            pixels += 1;
            highest = highest.max(index);
        }
    }
    assert_eq!(names.len(), 205);
    assert_eq!(pixels, 839_680);
    // A third confirmation of the 0..239 renderer range, from the art this
    // time rather than from the shade and blend tables.
    assert_eq!(highest, 239);
}

#[test]
fn the_shipped_data_has_dangling_course_references() {
    // The engine has a diagnostic for this - "Bad course ID for enemy" - and
    // the shipped levels trip it. Six of the 26 name a course their own .CRS
    // does not contain, so a port must survive a dangling id rather than
    // assume the data is sound. The set is pinned here so that a change in the
    // parser shows up as a change in the list.
    let pod = archive!("GAME.POD");
    let mut dangling: Vec<(String, usize)> = Vec::new();
    for e in pod.entries().iter().filter(|e| e.ext() == "lvl") {
        let level = lvl::Level::parse(pod.bytes(e)).unwrap();
        let stem = level.stem().to_string();
        let courses = pod
            .read("data", &level.courses)
            .ok()
            .map(|b| course::parse(b).unwrap())
            .unwrap_or_default();
        let defs =
            text::enemy_defs(pod.read("data", &format!("{stem}.def")).unwrap()).unwrap();
        let bad = defs
            .iter()
            .filter(|d| d.course != -1 && (d.course as usize) >= courses.len())
            .count();
        if bad > 0 {
            dangling.push((stem, bad));
        }
    }
    dangling.sort();
    assert_eq!(
        dangling,
        [
            ("hoth2".to_string(), 1),
            ("kreash2".to_string(), 3),
            ("netlvl1".to_string(), 9),
            ("netlvl2".to_string(), 9),
            ("netlvl3".to_string(), 9),
            ("roid2".to_string(), 1),
            ("ship".to_string(), 50),
        ]
    );
}

#[test]
fn every_level_ships_a_shading_database_of_seven_bytes_a_cell() {
    let pod = archive!("GAME.POD");
    let mut levels = 0;
    for e in pod.entries().iter().filter(|e| e.ext() == "lvl") {
        let level = lvl::Level::parse(pod.bytes(e)).unwrap();
        let stem = level.stem().to_string();
        let lte = pod
            .read("data", &format!("{stem}.lte"))
            .unwrap_or_else(|_| panic!("{stem} has no data\\{stem}.lte"));
        assert_eq!(lte.len(), terrain::Shading::BYTES, "{stem}");
        let shading = terrain::Shading::parse(lte).unwrap();
        assert_eq!(shading.ground.len(), terrain::CELLS);
        assert_eq!(shading.box_a.len(), terrain::CELLS);
        assert_eq!(shading.chambers.len(), terrain::CELLS);
        assert_eq!(shading.box_b.len(), terrain::CELLS);
        levels += 1;
    }
    assert_eq!(levels, 26);
}

#[test]
fn the_two_lte_extensions_are_different_formats() {
    // FOG\<stem>.lte is a 16-row colour ramp; DATA\<stem>.lte is the shading
    // database. The .LVL names the first; the terrain loader opens the second.
    let pod = archive!("GAME.POD");
    let fog = pod.read("fog", "float.lte").unwrap();
    let data = pod.read("data", "float.lte").unwrap();
    assert_eq!(fog.len(), colour::Ramp::BYTES);
    assert_eq!(data.len(), terrain::Shading::BYTES);
    colour::Ramp::parse(fog).expect("the fog one is a ramp");
    terrain::Shading::parse(data).expect("the data one is a shading database");
    // And each refuses the other. 114,688 is a multiple of 256, so a ramp
    // parser that only checked that would read the shading database as a
    // 448-row ramp without complaining.
    assert_eq!(data.len() % 256, 0, "the trap is real");
    assert!(colour::Ramp::parse(data).is_err());
    assert!(terrain::Shading::parse(fog).is_err());

    let level = lvl::Level::parse(pod.read("levels", "float.lvl").unwrap()).unwrap();
    assert_eq!(level.slot("light"), Some(("fog", "float.lte")));
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
