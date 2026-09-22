//! `hb` - inspect and extract Hellbender's data.
//!
//! Points at the game directory, defaulting to `original/` next to the repo
//! root, and works out of the PODs rather than out of extracted files.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod view;

use hb_formats::{act, colour, lvl, mrgl, png, raw, terrain, text};
use hb_pod::Pod;

const USAGE: &str = "\
hb - inspect Hellbender's data

  hb list <pod> [--long]            every entry
  hb stats <pod>...                 counts and bytes by extension
  hb extract <pod> <dir> [--only G] unpack, optionally filtering by glob
  hb level <name>                   parse a .LVL and say what it names
  hb nav <name>                     a level's mission, point by point
  hb terrain <name>                 load a level's thirteen terrain grids
  hb model <name.bin>               walk a model's MRGL nodes
  hb png <pod> <entry> <out.png>    a .RAW plus its palette, as a PNG
  hb view <name.bin> <out.png> [at] a model, flat shaded, as a PNG;
                                    [at] poses a .TXT model at that time
  hb heightmap <level> <out.png>    a level's ground, lit, from above
  hb ground <level> <out.png> [--authored]
                                    a level's ground, textured, from above;
                                    --authored ignores the orientation codes
  hb fly <level> <out.png> [x z yaw height pitch] [--bare] [--labels]
                                    one frame from in-level; --bare skips the cockpit
                                    and the HUD, --labels names every readout
  hb bench <level> [mode]           frames a second drawing a turn in place
  hb look <level> <n> <out.png> [distance] [--powerup K] [--at S]
                                    placement n, framed from the south and above;
                                    --powerup draws powerup kind K there instead,
                                    --at sets the clock for flipbook textures,
                                    --blast draws explosion frame N there
  hb font <out.png> [text]          a specimen of the front end's typeface
  hb brief <level> <out.png>        a level's mission briefing, as a PNG
  hb hudfont <out.png> [text]       a specimen of the HUD's font, from the EXE
  hb demo <n> <seconds> <out.png>   a frame of a recorded attract-mode flight
  hb check                          parse everything and report what fails

<pod> is a path, or one of `game` and `startup` to use $HB_GAME (default
`original`). <name> is a level stem such as `float`.
";

fn game_dir() -> PathBuf {
    std::env::var_os("HB_GAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("original"))
}

fn open_pod(spec: &str) -> Result<Pod, String> {
    let path = match spec {
        "game" => game_dir().join("system/GAME.POD"),
        "startup" => game_dir().join("system/STARTUP.POD"),
        other => PathBuf::from(other),
    };
    Pod::open(&path).map_err(|e| format!("{}: {e}", path.display()))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    match run(&refs) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("hb: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[&str]) -> Result<(), String> {
    match args {
        [] | ["-h"] | ["--help"] => {
            print!("{USAGE}");
            Ok(())
        }
        ["list", pod, rest @ ..] => cmd_list(pod, rest.contains(&"--long")),
        ["stats", pods @ ..] if !pods.is_empty() => cmd_stats(pods),
        ["extract", pod, dir, rest @ ..] => {
            let only = rest.windows(2).find(|w| w[0] == "--only").map(|w| w[1]);
            cmd_extract(pod, Path::new(dir), only)
        }
        ["level", name] => cmd_level(name),
        ["nav", name] => cmd_nav(name),
        ["terrain", name] => cmd_terrain(name),
        ["model", name] => cmd_model(name),
        ["png", pod, entry, out] => cmd_png(pod, entry, Path::new(out)),
        ["view", name, out] => cmd_view(name, Path::new(out), 0.0),
        ["view", name, out, at] => {
            cmd_view(name, Path::new(out), at.parse().map_err(|_| "bad time")?)
        }
        ["heightmap", name, out] => cmd_heightmap(name, Path::new(out)),
        ["ground", name, out, rest @ ..] => {
            cmd_ground(name, Path::new(out), !rest.contains(&"--authored"))
        }
        ["fly", name, out, rest @ ..] => cmd_fly(name, Path::new(out), rest),
        ["look", name, index, out, rest @ ..] => cmd_look(name, index, Path::new(out), rest),
        ["bench", name, rest @ ..] => cmd_bench(name, rest),
        ["font", out, rest @ ..] => cmd_font(Path::new(out), rest),
        ["brief", name, out] => cmd_brief(name, Path::new(out)),
        ["hudfont", out, rest @ ..] => cmd_hud_font(Path::new(out), rest),
        ["demo", n, at, out] => cmd_demo(n, at, Path::new(out)),
        ["check"] => cmd_check(),
        _ => Err(format!("unknown command\n\n{USAGE}")),
    }
}

fn cmd_list(spec: &str, long: bool) -> Result<(), String> {
    let pod = open_pod(spec)?;
    println!("# {:?}, {} entries", pod.comment(), pod.entries().len());
    for e in pod.entries() {
        if long {
            println!("{:10} {:9}  {:<24} {}", e.offset, e.size, e.name, e.palette);
        } else {
            println!("{}", e.name);
        }
    }
    Ok(())
}

fn cmd_stats(specs: &[&str]) -> Result<(), String> {
    for spec in specs {
        let pod = open_pod(spec)?;
        let mut by_ext: Vec<(String, usize, u64)> = Vec::new();
        for e in pod.entries() {
            let ext = e.ext();
            match by_ext.iter_mut().find(|(k, _, _)| *k == ext) {
                Some(slot) => {
                    slot.1 += 1;
                    slot.2 += e.size as u64;
                }
                None => by_ext.push((ext, 1, e.size as u64)),
            }
        }
        by_ext.sort_by_key(|(_, n, _)| std::cmp::Reverse(*n));
        println!("== {} {:?}", pod.path().display(), pod.comment());
        println!("   {} entries, gapless: {}", pod.entries().len(), pod.is_gapless());
        for (ext, n, bytes) in by_ext {
            println!("   {:<8} {:5}  {:>12} bytes", ext, n, bytes);
        }
    }
    Ok(())
}

fn cmd_extract(spec: &str, dest: &Path, only: Option<&str>) -> Result<(), String> {
    let pod = open_pod(spec)?;
    let mut written = 0usize;
    for e in pod.entries() {
        if let Some(pattern) = only {
            if !glob_match(&pattern.to_ascii_lowercase(), &e.path().to_ascii_lowercase()) {
                continue;
            }
        }
        let target = dest.join(e.path());
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&target, pod.bytes(e)).map_err(|e| e.to_string())?;
        written += 1;
    }
    println!("{written} files -> {}", dest.display());
    Ok(())
}

/// `*` and `?` only, which is all the filtering this needs.
fn glob_match(pattern: &str, text: &str) -> bool {
    fn go(p: &[u8], t: &[u8]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some(b'*'), _) => go(&p[1..], t) || (!t.is_empty() && go(p, &t[1..])),
            (Some(b'?'), Some(_)) => go(&p[1..], &t[1..]),
            (Some(a), Some(b)) if a == b => go(&p[1..], &t[1..]),
            _ => false,
        }
    }
    go(pattern.as_bytes(), text.as_bytes())
}

fn cmd_level(name: &str) -> Result<(), String> {
    let pod = open_pod("game")?;
    let file = format!("{name}.lvl");
    let data = pod.read("levels", &file).map_err(|e| e.to_string())?;
    let level = lvl::Level::parse(data).map_err(|e| e.to_string())?;
    println!("{file}: version {}, stem {:?}", level.version, level.stem());
    for ((slot, dir), file) in lvl::DIRS.iter().zip(&level.files) {
        let present = pod.find(dir, file).is_some();
        println!("  {slot:<16} {dir}\\{file}{}", if present { "" } else { "   MISSING" });
    }
    println!(
        "  weather          {} (snow {}, rain {}, lightning {})",
        level.weather,
        level.has_snow(),
        level.has_rain(),
        level.has_lightning()
    );
    println!("  redbook track    {}", level.redbook_track);
    println!("  briefing movie   {:?}", level.briefing_movie);
    println!("  death movie      {:?}", level.death_movie);
    let story: Vec<&str> =
        level.story_movies.iter().filter_map(|m| m.as_deref()).collect();
    println!("  story movies     {story:?}");
    Ok(())
}

fn cmd_nav(name: &str) -> Result<(), String> {
    use hb_formats::nav::{self, Data};
    let pod = open_pod("game")?;
    let data = pod.read("levels", &format!("{name}.lvl")).map_err(|e| e.to_string())?;
    let level = lvl::Level::parse(data).map_err(|e| e.to_string())?;
    let (dir, file) = level.slot("navigation").ok_or("no navigation slot")?;
    let navs = nav::navs(pod.read(dir, file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("{file}: {} points", navs.len());
    for (i, n) in navs.iter().enumerate() {
        let [x, y, z] = n.position.map(|v| v as f64 / 65536.0);
        let mut flags = String::new();
        if !n.required() {
            flags += " optional";
        }
        if n.time != 0 {
            flags += &format!(" {:.1} s", n.time as f64 / 65536.0);
        }
        println!("{i:3} {:<11} ({x:7.1},{y:6.1},{z:7.1}){flags}  {}", format!("{:?}", n.kind), n.text);
        let detail = match &n.data {
            Data::None => String::new(),
            Data::Targets(t) => format!("targets {t:?}"),
            Data::Guardian { actor, music, shields } => {
                format!("guardian {actor}, music {music}, shields {shields:?}")
            }
            Data::Start { angles } => format!("angles {angles:?}"),
            Data::Warp { to, name } => format!("warp to {to:?} {name}"),
            Data::Actor(a) => format!("placement {a}"),
            Data::Pod(p) => format!("pod {p}"),
        };
        let sounds: Vec<String> = [("done", &n.completion_sound), ("near", &n.proximity_sound)]
            .iter()
            .filter_map(|(what, s)| s.as_ref().map(|s| format!("{what} {s}")))
            .collect();
        if !detail.is_empty() || !sounds.is_empty() {
            println!("      {detail}{}{}", if detail.is_empty() || sounds.is_empty() { "" } else { "; " }, sounds.join(", "));
        }
    }
    Ok(())
}

fn cmd_terrain(name: &str) -> Result<(), String> {
    let pod = open_pod("game")?;
    let data = pod
        .read("levels", &format!("{name}.lvl"))
        .map_err(|e| e.to_string())?;
    let level = lvl::Level::parse(data).map_err(|e| e.to_string())?;
    let stem = level.stem().to_string();
    let t = terrain::Terrain::load(|ext| {
        pod.read("data", &format!("{stem}.{ext}")).ok().map(<[u8]>::to_vec)
    })
    .map_err(|e| e.to_string())?;

    let span = |a: &terrain::Altitudes| {
        let (lo, hi) = a.span();
        format!("{lo}..{hi}")
    };
    println!("{stem}: {} x {} cells", terrain::SIDE, terrain::SIDE);
    println!("  ground altitude   {}", span(&t.ground));
    println!("  box A bottom/top  {} / {}", span(&t.boxes_a.bottom), span(&t.boxes_a.top));
    println!("  chamber floor/ceil {} / {}", span(&t.chambers.floor), span(&t.chambers.ceiling));
    println!("  box B bottom/top  {} / {}", span(&t.boxes_b.bottom), span(&t.boxes_b.top));
    match &t.shading {
        Some(sh) => println!("  shading           {} cells, 7 bytes each", sh.ground.len()),
        None => println!("  shading           none shipped; the engine computes it"),
    }
    let grid = hb_world::Grid::new(&t);
    let count = |f: &dyn Fn(hb_world::Cell) -> bool| {
        (0..terrain::SIDE as i32)
            .flat_map(|z| (0..terrain::SIDE as i32).map(move |x| hb_world::Cell::new(x, z)))
            .filter(|c| f(*c))
            .count()
    };
    println!(
        "  cells with a box A: {}, box B: {}, chamber: {}",
        count(&|c| grid.has_box_a(c)),
        count(&|c| grid.has_box_b(c)),
        count(&|c| grid.has_chamber(c))
    );

    if let Ok(tex) = pod.read("data", &format!("{stem}.tex")) {
        let names = text::name_list(tex, "TEX").map_err(|e| e.to_string())?;
        println!("  textures          {}", names.len());
    }
    if let Ok(def) = pod.read("data", &format!("{stem}.def")) {
        let defs = text::enemy_defs(def).map_err(|e| e.to_string())?;
        println!("  enemy defs        {}", defs.len());
        for d in defs.iter().take(5) {
            println!("     {:<28} {} -> {}", d.name, d.model, d.wreck);
        }
    }
    Ok(())
}

fn cmd_model(name: &str) -> Result<(), String> {
    let pod = open_pod(if name.starts_with("startup:") { "startup" } else { "game" })?;
    let name = name.trim_start_matches("startup:");
    let data = pod.read("models", name).map_err(|e| e.to_string())?;
    for node in mrgl::Walk::new(data) {
        let node = node.map_err(|e| e.to_string())?;
        println!("{:#08x}  type {:#04x}  size {}", node.offset, node.kind, node.size);
    }
    let model = mrgl::Model::parse(data).map_err(|e| e.to_string())?;
    println!(
        "{} bytes: {} vertices, {} materials {:?}, {} polygons, {} children {:?}",
        data.len(),
        model.vertices.len(),
        model.materials.len(),
        model.materials,
        model.polygons.len(),
        model.children.len(),
        model.children
    );
    Ok(())
}

fn cmd_png(spec: &str, entry: &str, out: &Path) -> Result<(), String> {
    let pod = open_pod(spec)?;
    let (dir, file) = entry.split_once(['\\', '/']).unwrap_or(("art", entry));
    let e = pod
        .find(dir, file)
        .ok_or_else(|| format!("no entry {dir}\\{file}"))?;
    let image = raw::Image::parse_guessed(pod.bytes(e))
        .map_err(|e| e.to_string())?
        .ok_or("that entry is zero-length")?;
    // The POD directory records the palette the texture was authored against.
    let palette_name = if e.palette.is_empty() { "vga.act" } else { &e.palette };
    let palette_bytes = pod
        .read("art", &palette_name.to_ascii_lowercase())
        .map_err(|_| format!("palette {palette_name} is not in this archive"))?;
    let palette = act::Palette::parse(palette_bytes).map_err(|e| e.to_string())?;
    let rgb = image.to_rgb(&palette);
    let bytes = png::rgb(image.shape.width, image.shape.height, &rgb);
    std::fs::write(out, bytes).map_err(|e| e.to_string())?;
    println!(
        "{} {}x{} in {} -> {}",
        e.name,
        image.shape.width,
        image.shape.height,
        palette_name,
        out.display()
    );
    Ok(())
}

fn cmd_view(name: &str, out: &Path, at: f32) -> Result<(), String> {
    let startup = name.starts_with("startup:");
    let pod = open_pod(if startup { "startup" } else { "game" })?;
    let name = name.trim_start_matches("startup:");
    let data = pod.read("models", name).map_err(|e| e.to_string())?;
    let model = if name.to_ascii_lowercase().ends_with(".txt") {
        // An animated model, flattened to one frame.
        let animated = hb_formats::anim::parse(data).map_err(|e| e.to_string())?;
        println!(
            "{name}: {} parts, {} frames at {:.3}s, {} materials",
            animated.parts.len(),
            animated.frames,
            animated.time_per_frame as f32 / 65536.0,
            animated.materials.len()
        );
        let (frame, fraction) = animated.frame_at(at);
        println!("{name}: {at}s is frame {frame} plus {fraction:.2}");
        let (vertices, polygons) = animated.pose(at);
        mrgl::Model { vertices, polygons, materials: animated.materials, ..Default::default() }
    } else {
        mrgl::Model::parse(data).map_err(|e| e.to_string())?
    };
    let view = view::View::default();
    let (pixels, drawn, culled) = view::render(&model, &view);
    std::fs::write(out, png::rgb(view.width, view.height, &pixels))
        .map_err(|e| e.to_string())?;
    println!(
        "{name}: {} vertices, {} polygons - {drawn} drawn, {culled} back-facing -> {}",
        model.vertices.len(),
        model.polygons.len(),
        out.display()
    );
    Ok(())
}

fn cmd_heightmap(name: &str, out: &Path) -> Result<(), String> {
    let pod = open_pod("game")?;
    let data = pod
        .read("levels", &format!("{name}.lvl"))
        .map_err(|e| e.to_string())?;
    let level = lvl::Level::parse(data).map_err(|e| e.to_string())?;
    let stem = level.stem().to_string();
    let t = terrain::Terrain::load(|ext| {
        pod.read("data", &format!("{stem}.{ext}")).ok().map(<[u8]>::to_vec)
    })
    .map_err(|e| e.to_string())?;
    let grid = hb_world::Grid::new(&t);
    let scale = 5;
    let (pixels, boxes, chambers) = view::heightmap(&grid, scale);
    let side = terrain::SIDE * scale;
    std::fs::write(out, png::rgb(side, side, &pixels)).map_err(|e| e.to_string())?;
    println!("{stem}: {side}x{side}, {boxes} box cells, {chambers} roofed cells -> {}", out.display());
    Ok(())
}

fn cmd_ground(name: &str, out: &Path, oriented: bool) -> Result<(), String> {
    let game = open_pod("game")?;
    let startup = open_pod("startup")?;
    let level = hb_render::Level::load(&game, Some(&startup), name)?;
    let grid = hb_world::Grid::new(&level.terrain);
    let scale = 5;
    let (pixels, missing) = view::textured(
        &grid,
        &level.textures,
        &level.palette,
        level.light.as_ref(),
        scale,
        oriented,
    );
    let side = terrain::SIDE * scale;
    std::fs::write(out, png::rgb(side, side, &pixels)).map_err(|e| e.to_string())?;
    println!(
        "{}: {}/{} textures resolved, {missing} cells without one -> {}",
        level.stem,
        level.resolved_textures(),
        level.texture_names.len(),
        out.display()
    );
    Ok(())
}

/// Draw a full turn in place from the middle of a level and report the rate:
/// the renderer's cost with nothing else in the way.
fn cmd_bench(name: &str, rest: &[&str]) -> Result<(), String> {
    let game = open_pod("game")?;
    let startup = open_pod("startup")?;
    let level = hb_render::Level::load(&game, Some(&startup), name)?;
    let (w, h) = match rest.first().copied() {
        Some("400") => hb_render::Target::MODE_400,
        Some("480") => hb_render::Target::MODE_480,
        _ => hb_render::Target::MODE_200,
    };
    let grid = hb_world::Grid::new(&level.terrain);
    let (x, z) = (64 * terrain::CELL_SIZE, 64 * terrain::CELL_SIZE);
    let y = grid.ceiling_of_solid(x, z) + (20 << 16);
    let mut target = hb_render::Target::new(w, h);
    let scene = level.scene();
    let frames = 120u32;
    let started = std::time::Instant::now();
    let mut triangles = 0usize;
    for i in 0..frames {
        target.clear(0);
        let yaw = (i as u32 * 65536 / frames) as u16;
        let mut camera = hb_render::Camera::looking_at(x, y, z, hb_formats::Angle(yaw));
        camera.pitch = hb_formats::Angle(2000);
        let d = hb_render::draw_world(&mut target, &scene, &camera);
        triangles += d.ground + d.boxes + d.chambers;
    }
    let seconds = started.elapsed().as_secs_f32();
    println!(
        "{}: {w}x{h}, {frames} frames in {seconds:.2} s - {:.1} fps, {} terrain triangles a frame",
        level.stem,
        frames as f32 / seconds,
        triangles / frames as usize
    );
    Ok(())
}

/// One placed object seen from `distance` units along -z and a third of that
/// above, looking at it: a direct way to see what size things are drawn.
fn cmd_look(name: &str, index: &str, out: &Path, rest: &[&str]) -> Result<(), String> {
    let game = open_pod("game")?;
    let startup = open_pod("startup")?;
    let level = hb_render::Level::load(&game, Some(&startup), name)?;
    let n: usize = index.parse().map_err(|_| "n must be a placement index")?;
    let p = *level
        .placements
        .get(n)
        .ok_or_else(|| format!("{name} places {}", level.placements.len()))?;
    let kind = &level.kinds[p.kind];
    // `--powerup K` draws powerup kind K where the placement stands, alone.
    let powerup: Option<usize> = match rest.iter().position(|a| *a == "--powerup") {
        Some(i) => Some(
            rest.get(i + 1)
                .and_then(|k| k.parse().ok())
                .filter(|&k: &usize| k < hb_sim::powerup::KINDS.len())
                .ok_or("--powerup takes a kind, 0 to 30")?,
        ),
        None => None,
    };
    let mut rest = rest.to_vec();
    if let Some(i) = rest.iter().position(|a| *a == "--powerup") {
        rest.drain(i..(i + 2).min(rest.len()));
    }
    // `--blast N` draws explosion frame N where the placement stands.
    let mut blast: Option<usize> = None;
    if let Some(i) = rest.iter().position(|a| *a == "--blast") {
        blast = Some(rest.get(i + 1).and_then(|v| v.parse().ok()).ok_or("--blast takes a frame, 0 to 15")?);
        rest.drain(i..(i + 2).min(rest.len()));
    }
    // `--at S` sets the clock that turns flipbook textures.
    let mut seconds = 0.0f32;
    if let Some(i) = rest.iter().position(|a| *a == "--at") {
        seconds = rest.get(i + 1).and_then(|v| v.parse().ok()).ok_or("--at takes seconds")?;
        rest.drain(i..(i + 2).min(rest.len()));
    }
    let distance: f32 = match rest.first() {
        Some(d) => d.parse().map_err(|_| "distance must be world units")?,
        None => (kind.radius() as f32 / 65536.0 * 4.0).max(10.0),
    };
    let (x, y, z) = (p.x, p.y + ((distance / 3.0) * 65536.0) as i32, p.z - (distance * 65536.0) as i32);
    let grid = hb_world::Grid::new(&level.terrain);
    println!(
        "object at ({:.1}, {:.1}, {:.1}), solid top there {:.1}; eye at y {:.1}, solid top under it {:.1}",
        p.x as f32 / 65536.0,
        p.y as f32 / 65536.0,
        p.z as f32 / 65536.0,
        grid.ceiling_of_solid(p.x, p.z) as f32 / 65536.0,
        y as f32 / 65536.0,
        grid.ceiling_of_solid(x, z) as f32 / 65536.0
    );
    let mut camera = hb_render::Camera::looking_at(x, y, z, hb_formats::Angle(0));
    // Nose down by the angle to the object: atan(1/3), in the 16-bit circle.
    camera.pitch = hb_formats::Angle(((1.0f32 / 3.0).atan() / std::f32::consts::TAU * 65536.0) as u16);
    let (w, h) = hb_render::Target::MODE_200;
    let mut target = hb_render::Target::new(w, h);
    target.clear(0);
    let mut scene = level.scene();
    scene.seconds = seconds;
    let alone;
    if let Some(k) = powerup {
        let mesh = level.powerup_mesh[k].ok_or("that powerup's model did not load")?;
        alone = [hb_formats::text::Placement { kind: mesh, heading: 0, ..p }];
        scene.placements = &alone;
        println!(
            "powerup {k} {:?}: size {:.2} units",
            hb_sim::powerup::KINDS[k].0,
            level.powerup_size[k]
        );
    }
    let drawn = hb_render::draw_world(&mut target, &scene, &camera);
    if let Some(frame) = blast {
        let texture = level
            .blast
            .get(frame)
            .and_then(Option::as_ref)
            .ok_or("that explosion frame did not load")?;
        let at = [p.x, p.y, p.z].map(|v| v as f32 / 65536.0);
        let size = (kind.radius() as f32 / 65536.0).clamp(0.5, 8.0) * 2.0;
        hb_render::scene::draw_sprite(&mut target, &scene, &camera, at, size, texture);
        println!("explosion frame {frame} at size {size:.1}");
    }
    let rgb = target.to_rgb(&level.palette);
    std::fs::write(out, png::rgb(w, h, &rgb)).map_err(|e| e.to_string())?;
    println!(
        "{}: #{n} {} ({:?}), radius {:.1}, hit points {:.3}, from {distance:.0} units - {} objects drawn -> {}",
        level.stem,
        kind.model,
        kind.name.trim(),
        kind.radius() as f32 / 65536.0,
        p.hit_points as f32 / 65536.0,
        drawn.models,
        out.display()
    );
    Ok(())
}

fn cmd_fly(name: &str, out: &Path, rest: &[&str]) -> Result<(), String> {
    let bare = rest.contains(&"--bare");
    let labels = rest.contains(&"--labels");
    let rest: Vec<&str> =
        rest.iter().copied().filter(|a| *a != "--bare" && *a != "--labels").collect();
    let rest = rest.as_slice();
    let game = open_pod("game")?;
    let startup = open_pod("startup")?;
    let level = hb_render::Level::load(&game, Some(&startup), name)?;
    let grid = hb_world::Grid::new(&level.terrain);

    // Default to the middle of the map, a little above the ground.
    let cell = |n: i32| n * terrain::CELL_SIZE;
    let mut x = cell(64);
    let mut z = cell(64);
    let mut yaw = 0x2000u16;
    let mut height = 6i32;
    let mut pitch = 0i32;
    if let [sx, sz, syaw, more @ ..] = rest {
        x = cell(sx.parse().map_err(|_| "x must be a cell index")?);
        z = cell(sz.parse().map_err(|_| "z must be a cell index")?);
        yaw = syaw.parse().map_err(|_| "yaw must be 0..65535")?;
        if let [h, rest @ ..] = more {
            height = h.parse().map_err(|_| "height must be world units")?;
            if let [p, ..] = rest {
                pitch = p.parse().map_err(|_| "pitch must be -32768..32767")?;
            }
        }
    }
    let ground = hb_render::scene::ground_height(&grid, x, z);
    let y = ground + (height << 16);

    let (w, h) = hb_render::Target::MODE_200;
    let mut target = hb_render::Target::new(w, h);
    // Index 0 is the transparent colour and also, here, the sky.
    target.clear(0);
    let mut camera = hb_render::Camera::looking_at(x, y, z, hb_formats::Angle(yaw));
    camera.pitch = hb_formats::Angle(pitch as u16);
    let scene = level.scene();
    let drawn = hb_render::draw_world(&mut target, &scene, &camera);
    // The cockpit, in VGA.ACT, over a frame in the level's palette - unless
    // asked for the bare view.
    let cockpit = startup
        .read("art", "ckpt200.raw")
        .ok()
        .and_then(|b| raw::Image::parse_guessed(b).ok().flatten());
    if let (Some(art), false) = (cockpit, bare) {
        target.overlay(&art);
    }
    // The readout, so the layout can be looked at without a window. The
    // numbers are a sample; what feeds them in flight is `hb-fly`.
    if !bare {
        if let Ok(exe) = std::fs::read(game_dir().join("HELLBEND.EXE")) {
            if let Ok(font) = hb_formats::hud_font::HudFont::read(&exe) {
                // Every placement, as the radar would see it from here.
                let heading = camera.yaw.0 as f32 * std::f32::consts::TAU / 65536.0;
                let (sin, cos) = heading.sin_cos();
                let blips: Vec<hb_render::hud::Blip> = level
                    .placements
                    .iter()
                    .filter(|p| {
                        let class = level.kinds[p.kind].class();
                        class != 18 && class != 33
                    })
                    .map(|p| {
                        // The world wraps at 1024 units, which is what the
                        // engine's shl 6 / sar 6 does to the offset.
                        let wrap = |d: f32| d - (d / 1024.0).round() * 1024.0;
                        let dx = wrap((p.x - x) as f32 / 65536.0);
                        let dz = wrap((p.z - z) as f32 / 65536.0);
                        let class = level.kinds[p.kind].class();
                        hb_render::hud::Blip {
                            right: dx * cos - dz * sin,
                            forward: dx * sin + dz * cos,
                            colour: if hb_render::hud::is_target(class) {
                                hb_render::hud::BLIP_TARGET
                            } else {
                                hb_render::hud::BLIP_OTHER
                            },
                            below: p.y < y,
                        }
                    })
                    .collect();
                let readout = hb_render::hud::Readout {
                    weapon: "VAL",
                    ammo: None,
                    objective: Some("TGT"),
                    distance: Some(144),
                    gauges: [0.7, 1.0, 0.35, 0.5, 0.85, 1.0],
                    countdown: Some(57),
                    labels,
                    blips: &blips,
                };
                hb_render::hud::draw(&mut target, &font, &readout);
                if let Some(art) = startup
                    .read("art", &format!("{}.raw", hb_render::hud::ICONS[0]))
                    .ok()
                    .and_then(|b| raw::Image::parse_guessed(b).ok().flatten())
                {
                    hb_render::hud::icon(&mut target, &art);
                }
                hb_render::hud::arrow(&mut target, 0x2000);
                if let Some(model) = startup
                    .read("models", "target.bin")
                    .ok()
                    .and_then(|b| hb_formats::mrgl::Model::parse(&b).ok())
                {
                    hb_render::hud::reticle(&mut target, &model, 0x30);
                }
            }
        }
    }
    let rgb = target.to_rgb(&level.palette);
    std::fs::write(out, png::rgb(w, h, &rgb)).map_err(|e| e.to_string())?;
    println!(
        "{}: cell ({}, {}) yaw {yaw:#06x} - {} ground, {} box, {} chamber triangles, \
         {} of {} objects drawn ({} placed, {} drawable), {} clipped -> {}",
        level.stem,
        x / terrain::CELL_SIZE,
        z / terrain::CELL_SIZE,
        drawn.ground,
        drawn.boxes,
        drawn.chambers,
        drawn.models,
        level.placements.len(),
        level.placements.len(),
        level.drawable_placements(),
        drawn.clipped,
        out.display()
    );
    Ok(())
}

fn cmd_font(out: &Path, rest: &[&str]) -> Result<(), String> {
    use hb_formats::font::{Font, HEIGHT};
    let startup = open_pod("startup")?;
    let index = startup.read("startup", "font.ndx").map_err(|e| e.to_string())?;
    let bitmap = startup.read("startup", "font.bin").map_err(|e| e.to_string())?;
    let font = Font::parse(index, bitmap).map_err(|e| e.to_string())?;
    let palette = act::Palette::parse(
        startup.read("art", "vga.act").map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    let default = [
        "ABCDEFGHIJKLM".to_string(),
        "NOPQRSTUVWXYZ".to_string(),
        "abcdefghijklm".to_string(),
        "nopqrstuvwxyz".to_string(),
        "0123456789 !?".to_string(),
        "HELLBENDER".to_string(),
    ];
    let rows: Vec<String> = if rest.is_empty() {
        default.to_vec()
    } else {
        vec![rest.join(" ")]
    };

    let width = rows.iter().map(|r| font.measure(r)).max().unwrap_or(1) + 8;
    let height = rows.len() * (HEIGHT + 2) + 6;
    let mut pixels = vec![0u8; width * height];
    for (i, row) in rows.iter().enumerate() {
        font.draw(
            &mut pixels,
            width,
            height,
            4,
            (3 + i * (HEIGHT + 2)) as isize,
            row,
            None,
        );
    }
    let mut rgb = Vec::with_capacity(pixels.len() * 3);
    for &index in &pixels {
        rgb.extend_from_slice(&palette.rgb(index));
    }
    std::fs::write(out, png::rgb(width, height, &rgb)).map_err(|e| e.to_string())?;
    println!(
        "{} glyphs, {} tall, widths {}..{} -> {}",
        font.widths.len(),
        HEIGHT,
        font.widths.iter().min().unwrap(),
        font.widths.iter().max().unwrap(),
        out.display()
    );
    Ok(())
}

/// The mission briefing: the screen every level draws it on, with its prose
/// over it.
fn cmd_brief(name: &str, out: &Path) -> Result<(), String> {
    use hb_formats::brief::{Brief, BACKDROP};
    use hb_formats::hud_font::{HudFont, LINE};
    let game = open_pod("game")?;
    let startup = open_pod("startup")?;
    let file = format!("{name}.txt");
    let brief = Brief::parse(game.read("data", &file).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    // Every briefing is drawn on the same screen; the name in the file is
    // the texture the globe wears, not the background.
    let art = startup.read("art", BACKDROP).map_err(|e| format!("{BACKDROP}: {e}"))?;
    let image = raw::Image::parse_guessed(art)
        .map_err(|e| e.to_string())?
        .ok_or("brief.raw is not a .RAW this port knows")?;
    let palette =
        act::Palette::parse(startup.read("art", "brief.act").map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;

    // `FONT.BIN` is 23 pixels a line and a briefing runs to seventeen
    // lines, which does not fit a 200-line screen, so this is in the small
    // font. Which font the engine uses here is not read.
    let exe = std::fs::read(game_dir().join("HELLBEND.EXE"))
        .map_err(|e| format!("HELLBEND.EXE: {e}"))?;
    let font = HudFont::read(&exe).map_err(|e| e.to_string())?;

    let (w, h) = (image.shape.width, image.shape.height);
    let mut pixels = image.pixels.clone();
    let ink = (0..=255u8)
        .max_by_key(|&i| palette.rgb(i).iter().map(|&c| c as u32).sum::<u32>())
        .unwrap_or(255);
    // Inside the panel the frame leaves, wrapped to it.
    let [px, py, pw, ph] = hb_formats::brief::PANEL;
    let mut rows: Vec<String> = Vec::new();
    for line in &brief.lines {
        if line.trim().is_empty() {
            rows.push(String::new());
            continue;
        }
        let mut row = String::new();
        for word in line.split_whitespace() {
            let candidate = if row.is_empty() { word.to_string() } else { format!("{row} {word}") };
            if font.width(&candidate) > pw && !row.is_empty() {
                rows.push(std::mem::take(&mut row));
                row = word.to_string();
            } else {
                row = candidate;
            }
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }
    // A still shows the top of it; the game types it out and scrolls.
    let shown = (ph / LINE).min(rows.len());
    for (i, line) in rows[..shown].iter().enumerate() {
        font.draw(&mut pixels, w, h, px as isize, (py + i * LINE) as isize, line, ink);
    }

    let mut rgb = Vec::with_capacity(pixels.len() * 3);
    for &i in &pixels {
        rgb.extend_from_slice(&palette.rgb(i));
    }
    std::fs::write(out, png::rgb(w, h, &rgb)).map_err(|e| e.to_string())?;
    let (planet, mission) = brief.headline();
    println!(
        "{file}: {} wearing {}, {} lines - {} / {} -> {}",
        brief.model,
        brief.texture,
        brief.lines.len(),
        planet.unwrap_or("?"),
        mission.unwrap_or("?"),
        out.display()
    );
    Ok(())
}

/// The HUD's own font, out of the executable.
fn cmd_hud_font(out: &Path, rest: &[&str]) -> Result<(), String> {
    use hb_formats::hud_font::{HudFont, LINE};
    let exe = std::fs::read(game_dir().join("HELLBEND.EXE"))
        .map_err(|e| format!("HELLBEND.EXE: {e}"))?;
    let font = HudFont::read(&exe).map_err(|e| e.to_string())?;
    let startup = open_pod("startup")?;
    let palette =
        act::Palette::parse(startup.read("art", "vga.act").map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;

    let rows: Vec<String> = if rest.is_empty() {
        [
            "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            "abcdefghijklmnopqrstuvwxyz",
            "0123456789 .,:;!?-+()[]/*",
            "Go Faster to Deploy Mine",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    } else {
        vec![rest.join(" ")]
    };

    let width = rows.iter().map(|r| font.width(r)).max().unwrap_or(1) + 8;
    let height = rows.len() * (LINE + 1) + 6;
    let mut pixels = vec![0u8; width * height];
    // The brightest entry of the front end's palette, so the specimen can be
    // read; the HUD itself draws in whatever the caller asks for.
    let ink = (0..=255u8)
        .max_by_key(|&i| palette.rgb(i).iter().map(|&c| c as u32).sum::<u32>())
        .unwrap_or(255);
    for (i, row) in rows.iter().enumerate() {
        font.draw(&mut pixels, width, height, 4, (3 + i * (LINE + 1)) as isize, row, ink);
    }
    // Four times up, so five-pixel letters can be looked at.
    let scale = 4;
    let (out_w, out_h) = (width * scale, height * scale);
    let mut rgb = Vec::with_capacity(out_w * out_h * 3);
    for y in 0..out_h {
        for x in 0..out_w {
            rgb.extend_from_slice(&palette.rgb(pixels[(y / scale) * width + x / scale]));
        }
    }
    std::fs::write(out, png::rgb(out_w, out_h, &rgb)).map_err(|e| e.to_string())?;
    let widths: Vec<usize> = (0x20..0x7f)
        .filter_map(|c| font.glyph(char::from(c as u8)))
        .map(|g| g.width)
        .collect();
    println!(
        "{} glyphs, 5 tall in a {LINE}-pixel line, widths {}..{} -> {}",
        widths.len(),
        widths.iter().min().unwrap_or(&0),
        widths.iter().max().unwrap_or(&0),
        out.display()
    );
    Ok(())
}

fn cmd_demo(n: &str, at: &str, out: &Path) -> Result<(), String> {
    let game = open_pod("game")?;
    let startup = open_pod("startup")?;
    let demo = hb_formats::demo::Demo::parse(
        startup.read("demo", &format!("demo{n}.dmo")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let stem = demo.level.trim_end_matches(".lvl");
    let level = hb_render::Level::load(&game, Some(&startup), stem)
        .map_err(|_| format!("demo {n} was recorded on {}, which did not ship", demo.level))?;
    let seconds: f32 = at.parse().map_err(|_| "seconds must be a number")?;
    let pose = demo.pose_at(seconds).ok_or("no pose at that time")?;

    let (w, h) = hb_render::Target::MODE_200;
    let mut target = hb_render::Target::new(w, h);
    target.clear(0);
    let mut camera = hb_render::Camera::looking_at(
        pose.x,
        pose.y,
        pose.z,
        hb_formats::Angle(pose.angles[2] as u16),
    );
    camera.pitch = hb_formats::Angle(pose.angles[0] as u16);
    let scene = level.scene();
    hb_render::draw_world(&mut target, &scene, &camera);
    if let Some(art) = startup
        .read("art", "ckpt200.raw")
        .ok()
        .and_then(|b| raw::Image::parse_guessed(b).ok().flatten())
    {
        target.overlay(&art);
    }
    std::fs::write(out, png::rgb(w, h, &target.to_rgb(&level.palette)))
        .map_err(|e| e.to_string())?;
    println!(
        "demo {n} at {seconds:.1}s of {:.1}: ({:.1}, {:.1}, {:.1}) heading {} pitch {} -> {}",
        demo.seconds(),
        pose.x as f32 / 65536.0,
        pose.y as f32 / 65536.0,
        pose.z as f32 / 65536.0,
        pose.angles[2],
        pose.angles[0],
        out.display()
    );
    Ok(())
}

fn cmd_check() -> Result<(), String> {
    let mut failures = 0usize;
    for spec in ["startup", "game"] {
        let pod = open_pod(spec)?;
        let (mut models, mut palettes, mut images, mut levels) = (0, 0, 0, 0);
        let (mut ramps, mut maps, mut blends) = (0, 0, 0);
        for e in pod.entries() {
            let data = pod.bytes(e);
            let outcome: Result<(), String> = match e.ext().as_str() {
                "bin" if e.dir() == "models" => {
                    models += 1;
                    if mrgl::Model::walks_exactly(data) {
                        Ok(())
                    } else {
                        Err("MRGL stream does not end at end of file".into())
                    }
                }
                "act" => {
                    palettes += 1;
                    act::Palette::parse_lenient(data).map(|_| ()).map_err(|e| e.to_string())
                }
                "raw" if e.dir() == "art" => {
                    images += 1;
                    raw::Image::parse_guessed(data).map(|_| ()).map_err(|e| e.to_string())
                }
                "lvl" => {
                    levels += 1;
                    lvl::Level::parse(data).map(|_| ()).map_err(|e| e.to_string())
                }
                "lte" | "fog" => {
                    ramps += 1;
                    colour::Ramp::parse(data).map(|_| ()).map_err(|e| e.to_string())
                }
                "map" => {
                    maps += 1;
                    colour::ColourMap::parse(data).map(|_| ()).map_err(|e| e.to_string())
                }
                "mix" if e.size > 0 => {
                    blends += 1;
                    colour::BlendTable::parse(data).map(|_| ()).map_err(|e| e.to_string())
                }
                _ => Ok(()),
            };
            if let Err(why) = outcome {
                println!("  {}: {why}", e.name);
                failures += 1;
            }
        }
        println!(
            "{spec}: {models} models, {palettes} palettes, {images} images, {levels} levels, \
             {ramps} ramps, {maps} colour maps, {blends} blend tables"
        );
    }
    if failures == 0 {
        println!("everything parses");
        Ok(())
    } else {
        Err(format!("{failures} entries failed to parse"))
    }
}
