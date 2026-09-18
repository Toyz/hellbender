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
  hb terrain <name>                 load a level's thirteen terrain grids
  hb model <name.bin>               walk a model's MRGL nodes
  hb png <pod> <entry> <out.png>    a .RAW plus its palette, as a PNG
  hb view <name.bin> <out.png>      a model, flat shaded, as a PNG
  hb heightmap <level> <out.png>    a level's ground, lit, from above
  hb ground <level> <out.png>       a level's ground, textured, from above
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
        ["terrain", name] => cmd_terrain(name),
        ["model", name] => cmd_model(name),
        ["png", pod, entry, out] => cmd_png(pod, entry, Path::new(out)),
        ["view", name, out] => cmd_view(name, Path::new(out)),
        ["heightmap", name, out] => cmd_heightmap(name, Path::new(out)),
        ["ground", name, out] => cmd_ground(name, Path::new(out)),
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
    let boxes_a = t.boxes_a.top.values.iter().filter(|&&v| v != 0).count();
    let boxes_b = t.boxes_b.top.values.iter().filter(|&&v| v != 0).count();
    println!("  cells with a box A: {boxes_a}, box B: {boxes_b}");

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

fn cmd_view(name: &str, out: &Path) -> Result<(), String> {
    let startup = name.starts_with("startup:");
    let pod = open_pod(if startup { "startup" } else { "game" })?;
    let name = name.trim_start_matches("startup:");
    let data = pod.read("models", name).map_err(|e| e.to_string())?;
    let model = mrgl::Model::parse(data).map_err(|e| e.to_string())?;
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

fn cmd_ground(name: &str, out: &Path) -> Result<(), String> {
    let pod = open_pod("game")?;
    let startup = open_pod("startup")?;
    let data = pod
        .read("levels", &format!("{name}.lvl"))
        .map_err(|e| e.to_string())?;
    let level = lvl::Level::parse(data).map_err(|e| e.to_string())?;
    let stem = level.stem().to_string();
    let t = terrain::Terrain::load(|ext| {
        pod.read("data", &format!("{stem}.{ext}")).ok().map(<[u8]>::to_vec)
    })
    .map_err(|e| e.to_string())?;

    // The texture list, resolved to images. An entry may live in either
    // archive, and a level names textures the front end also uses.
    let names = text::name_list(
        pod.read("data", &format!("{stem}.tex")).map_err(|e| e.to_string())?,
        "TEX",
    )
    .map_err(|e| e.to_string())?;
    let textures: Vec<Option<raw::Image>> = names
        .iter()
        .map(|n| {
            let bytes = pod.read("art", n).or_else(|_| startup.read("art", n)).ok()?;
            raw::Image::parse_guessed(bytes).ok().flatten()
        })
        .collect();

    let (_, palette_name) = level.slot("ground_palette").ok_or("no ground palette")?;
    let palette_bytes = pod
        .read("art", palette_name)
        .or_else(|_| startup.read("art", palette_name))
        .map_err(|e| e.to_string())?;
    let palette = act::Palette::parse(palette_bytes).map_err(|e| e.to_string())?;
    let ramp = level
        .slot("light")
        .and_then(|(dir, file)| pod.read(dir, file).ok())
        .and_then(|b| colour::Ramp::parse(b).ok());

    let grid = hb_world::Grid::new(&t);
    let scale = 5;
    let (pixels, missing) = view::textured(&grid, &textures, &palette, ramp.as_ref(), scale);
    let side = terrain::SIDE * scale;
    std::fs::write(out, png::rgb(side, side, &pixels)).map_err(|e| e.to_string())?;
    let resolved = textures.iter().filter(|t| t.is_some()).count();
    println!(
        "{stem}: {resolved}/{} textures resolved, {missing} cells without one, \
         palette {palette_name}, ramp {} -> {}",
        names.len(),
        if ramp.is_some() { "yes" } else { "no" },
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
