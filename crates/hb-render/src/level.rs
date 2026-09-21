//! Everything a level needs to be drawn, pulled out of the archives.
//!
//! The engine loads these from the names on the level's [.LVL] manifest, plus
//! the thirteen terrain grids whose names it derives from the stem. This
//! assembles the same set.

use hb_formats::act::Palette;
use hb_formats::colour::{ColourMap, Ramp};
use hb_formats::lvl::Level as Manifest;
use hb_formats::raw::Image;
use hb_formats::{anim, mrgl};
use hb_formats::terrain::Terrain;
use hb_formats::text::{self, EnemyDef, Placement};
use hb_pod::Pod;

/// A sky texture's indices are 48 higher than the entries they name in the sky
/// palette.
///
/// A sky palette is not a palette: it is a gradient band of 15 to 32 entries
/// starting at 192, with everything else black. The sky textures index 240
/// upwards - `SKY.RAW` uses 240 to 254 and `NEWSKY.RAW` 244 to 251 - and
/// subtracting 48 lands every one of them inside its palette's band, in all 20
/// levels with a sky texture.
pub const SKY_BIAS: u8 = 48;

/// A mesh's flipbook material with its frames loaded.
#[derive(Debug, Clone)]
pub struct Flip {
    pub material: usize,
    pub frames: Vec<Option<Image>>,
    /// Seconds a frame.
    pub period: f32,
}

pub struct Level {
    pub manifest: Manifest,
    pub stem: String,
    pub terrain: Terrain,
    /// The level's `.TEX` list, resolved. An entry is `None` when the named
    /// file is not in either archive or is a zero-length placeholder.
    pub textures: Vec<Option<Image>>,
    /// Each texture at half and quarter size - 32 and 16 texels for the
    /// 64-texel terrain textures - for the engine's distance-picked
    /// resolution (see [`crate::scene`]). `None` where the texture is missing
    /// or will not halve.
    pub mips: Vec<[Option<Image>; 2]>,
    pub texture_names: Vec<String>,
    pub palette: Palette,
    pub light: Option<Ramp>,
    pub fog: Option<Ramp>,
    /// The `.DEF` type table: up to 100 kinds of object.
    pub kinds: Vec<EnemyDef>,
    /// The `.DEF` instance list: where those kinds actually stand.
    pub placements: Vec<Placement>,
    /// One mesh per kind, in the same order, loaded from `MODELS\`. `None`
    /// when the kind's model is a `.TXT` animated model, which is a different
    /// format and is not parsed yet.
    pub meshes: Vec<Option<mrgl::Model>>,
    /// The `.TXT` source of any mesh that has one, so the pose can be taken
    /// again at a new time.
    pub animated: Vec<Option<anim::Animated>>,
    /// Each mesh's materials resolved to images, in the order the mesh names
    /// them. A model's textures come from `ART\` by name, not from the level's
    /// `.TEX` list.
    pub mesh_textures: Vec<Vec<Option<Image>>>,
    /// The scale each mesh is drawn at: its type's radius from `.DEF` line 0
    /// field 2, which is what the engine's actor draw at `0x40da00` passes.
    /// A wreck is drawn at the radius of the type it replaces - the engine
    /// swaps the model on the same actor.
    pub mesh_radius: Vec<i32>,
    /// The sky texture, 64 x 64. `None` for the levels whose sky slot names a
    /// zero-length `.VOX`, which are the ones set in space.
    pub sky: Option<Image>,
    /// The star field the space levels draw instead of a sky plane, empty
    /// everywhere else. See [`stars`].
    pub stars: Vec<Star>,
    /// How many times the field is drawn: `space.vox` draws it twice, the
    /// second pass with the axes swapped, and `stars.vox` once (`0x4506e0`,
    /// `0x450500`).
    pub star_passes: u8,
    /// The sky texture's own indices remapped into the level's palette.
    ///
    /// A sky palette shares almost nothing with its level's - `FLOAT.ACT` and
    /// `FLOATSK.ACT` agree on 17 of 256 entries, 16 of them the reserved ones -
    /// so the sky cannot be blitted straight into the frame. The level's
    /// `.MAP` is the table that fixes it: it takes a 15-bit colour to the
    /// nearest index in the level's palette, which is exactly the question
    /// "what is this sky colour called here".
    pub sky_remap: Option<[u8; 256]>,
    /// `.ANI` cycles, resolved to indices into [`Level::textures`].
    pub animations: Vec<Cycle>,
    /// The level's `.CRS` courses, which a type's `.DEF` line 15 names.
    pub courses: Vec<hb_formats::course::Course>,
    /// For each type, the index in `meshes` of its wreck model, when it has a
    /// real one. Wrecks are appended after the types so a destroyed placement
    /// can point at one without the renderer knowing anything happened.
    pub wreck_mesh: Vec<Option<usize>>,
    /// For each type, the bytes of its destroy sound from `.DEF` line 24.
    pub destroy_sound: Vec<Option<Vec<u8>>>,
    /// For each mesh, its flipbook materials with their frames.
    pub mesh_flipbooks: Vec<Vec<Flip>>,
    /// The mission, as the `.NAV` file lists it.
    pub navs: Vec<hb_formats::nav::Nav>,
    /// For each of the 31 powerup kinds, the index in `meshes` of its model.
    pub powerup_mesh: Vec<Option<usize>>,
    /// For each kind, how close the player must come to pick it up, units.
    pub powerup_size: Vec<f32>,
    /// For each weapon kind, the index in `meshes` of the model its shots
    /// are drawn with.
    pub shot_mesh: Vec<Option<usize>>,
    /// The Valkyrie Cannon's three muzzle flashes.
    pub muzzle_mesh: [Option<usize>; 3],
    /// The explosion's frames, `blast1.raw` upward.
    pub blast: Vec<Option<Image>>,
    /// The powerups the level's `.PUP` lays out.
    pub powerups: Vec<text::PlacedPowerup>,
    /// The level's `.QKE`: the doors and the ground that moves.
    pub quake: hb_formats::quake::Quake,
}

impl Level {
    /// Pose every animated model at `seconds`. The engine keeps a clock per
    /// actor; this port runs them all off the level's, which is the same
    /// thing for the 18 models that have one animation each.
    pub fn animate(&mut self, seconds: f32) {
        for (i, source) in self.animated.iter().enumerate() {
            let (Some(source), Some(mesh)) = (source.as_ref(), self.meshes.get_mut(i)) else {
                continue;
            };
            let Some(mesh) = mesh.as_mut() else { continue };
            let (vertices, polygons) = source.pose(seconds);
            mesh.vertices = vertices;
            mesh.polygons = polygons;
        }
    }

    /// Where a type's animated model keeps one of its parts, as a model
    /// offset at the type's own radius. `None` for a type whose model is not
    /// animated, or which has no such part.
    pub fn part_origin(&self, kind: usize, part: usize, seconds: f32) -> Option<[i32; 3]> {
        let model = self.animated.get(kind)?.as_ref()?;
        let at = model.part_origin(part, seconds)?;
        let radius = self.kinds.get(kind)?.radius();
        Some(at.map(|c| ((c as i64 * radius as i64) / hb_formats::mrgl::MODEL_ONE as i64) as i32))
    }

    /// How fast the sky drifts, texture units a second in u and v: the
    /// `.LVL`'s line 41, which the parser reads to `0x6670d0` and the sky
    /// routine adds to its scroll every frame (`0x44fd98`). 10.0 in eleven
    /// levels, which is a tile every 12.8 seconds, and still in fifteen.
    pub fn sky_drift(&self) -> [f32; 2] {
        let [u, v] = self.manifest.weather_params[0];
        [u as f32 / 65536.0, v as f32 / 65536.0]
    }

    /// The altitude of the sky layer, in units: the `.LVL`'s
    /// [line 30](../../../docs/formats/lvl.md), which the sky setup keeps
    /// shifted up fifteen at `0x5055d4`. 127.5 in 24 levels, 95.0 in
    /// `KREASH` and 65.0 in `JURASIC`.
    pub fn sky_height(&self) -> f32 {
        self.manifest.sky_height as f32 * 32768.0 / 65536.0
    }

    /// The sky's scroll after `seconds`.
    pub fn sky_scroll(&self, seconds: f32) -> [f32; 2] {
        let [u, v] = self.sky_drift();
        [(u * seconds).rem_euclid(256.0), (v * seconds).rem_euclid(256.0)]
    }
}


/// One of the 2,000 points the space levels draw for a sky.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Star {
    /// Where it is, as a 16.16 offset from the eye - the engine zeroes the
    /// camera's position before transforming, so a star never moves.
    pub at: [i32; 3],
    /// Its palette index, 0 to 31, which is the grey ramp at the bottom of
    /// every level palette: a star's brightness.
    pub colour: u8,
}

/// The star field, as `0x44f6f0` generates it once at startup: x and z are
/// `(rand() - 0x4000) << 8`, so +-64.0; y is `rand() << 8`, 0 to 128.0,
/// negated for the second thousand so the field surrounds the eye; and the
/// colour is `rand() >> 10`. Nothing about the stars is in a file, which is
/// why a `.VOX` is zero bytes long - it only has to be named.
///
/// `rand` here is the C runtime's, which is what the game calls: the seed
/// times 214013 plus 2531011, taking bits 16 to 30.
pub fn stars() -> Vec<Star> {
    let mut seed: u32 = 1;
    let mut rand = || {
        seed = seed.wrapping_mul(214013).wrapping_add(2531011);
        ((seed >> 16) & 0x7fff) as i32
    };
    (0..2000)
        .map(|i| {
            let x = (rand() - 0x4000) << 8;
            let y = rand() << 8;
            let z = (rand() - 0x4000) << 8;
            let colour = (rand() >> 10) as u8;
            Star { at: [x, if i < 1000 { y } else { -y }, z], colour }
        })
        .collect()
}

/// An animated texture, with every name already resolved to a texture index.
#[derive(Debug, Clone)]
pub struct Cycle {
    /// The texture that is replaced.
    pub base: usize,
    /// Seconds per frame.
    pub delay: f32,
    pub frames: Vec<usize>,
}

impl Level {
    /// `game` holds the level; `startup` is consulted for art a level names but
    /// does not carry, which happens because the front end and the levels share
    /// some textures.
    pub fn load(game: &Pod, startup: Option<&Pod>, name: &str) -> Result<Level, String> {
        let read = |dir: &str, file: &str| -> Option<Vec<u8>> {
            game.read(dir, file)
                .ok()
                .or_else(|| startup.and_then(|s| s.read(dir, file).ok()))
                .map(<[u8]>::to_vec)
        };

        let bytes = read("levels", &format!("{name}.lvl"))
            .ok_or_else(|| format!("no levels\\{name}.lvl"))?;
        let manifest = Manifest::parse(&bytes).map_err(|e| e.to_string())?;
        let stem = manifest.stem().to_string();

        let terrain = Terrain::load(|ext| read("data", &format!("{stem}.{ext}")))
            .map_err(|e| e.to_string())?;

        let tex = read("data", &format!("{stem}.tex"))
            .ok_or_else(|| format!("no data\\{stem}.tex"))?;
        let texture_names = text::name_list(&tex, "TEX").map_err(|e| e.to_string())?;
        let (_, palette_name) =
            manifest.slot("ground_palette").ok_or("no ground palette slot")?;
        let palette_bytes = read("art", palette_name)
            .ok_or_else(|| format!("no art\\{palette_name}"))?;
        let palette = Palette::parse(&palette_bytes).map_err(|e| e.to_string())?;

        let ramp = |slot: &str| -> Option<Ramp> {
            let (dir, file) = manifest.slot(slot)?;
            Ramp::parse(&read(dir, file)?).ok()
        };

        let def = read("data", &format!("{stem}.def"))
            .ok_or_else(|| format!("no data\\{stem}.def"))?;
        let kinds = text::enemy_defs(&def).map_err(|e| e.to_string())?;
        let placements = text::placements(&def).map_err(|e| e.to_string())?;
        // An animated `.TXT` model is posed into the same mesh a static one
        // uses, so one draw path serves both; [`Level::animate`] re-poses it
        // as the clock runs.
        let animated: Vec<Option<anim::Animated>> = kinds
            .iter()
            .map(|k| {
                k.model
                    .to_ascii_lowercase()
                    .ends_with(".txt")
                    .then(|| read("models", &k.model).and_then(|b| anim::parse(&b).ok()))
                    .flatten()
            })
            .collect();
        let meshes: Vec<Option<mrgl::Model>> = kinds
            .iter()
            .map(|k| {
                let bytes = read("models", &k.model)?;
                if k.model.to_ascii_lowercase().ends_with(".txt") {
                    let animated = anim::parse(&bytes).ok()?;
                    let (vertices, polygons) = animated.rest_pose();
                    return Some(mrgl::Model {
                        vertices,
                        polygons,
                        materials: animated.materials.clone(),
                        ..mrgl::Model::default()
                    });
                }
                let model = mrgl::Model::parse(&bytes).ok()?;
                // A group model - only the SAM sites use one - has no geometry
                // of its own, only child names. The engine takes the first
                // child for the type's bounds (`0x473ee0` recurses into the
                // name at 0x18), and so does this. The children run
                // Sam01-05-02 like an animation, whose rate is not read.
                if model.polygons.is_empty() {
                    if let Some(first) = model.children.first() {
                        return mrgl::Model::parse(&read("models", first)?).ok();
                    }
                }
                Some(model)
            })
            .collect();

        let mesh_textures: Vec<Vec<Option<Image>>> = meshes
            .iter()
            .map(|m| match m {
                Some(model) => model
                    .materials
                    .iter()
                    .map(|name| {
                        read("art", name).and_then(|b| Image::parse_guessed(&b).ok().flatten())
                    })
                    .collect(),
                None => Vec::new(),
            })
            .collect();

        // The sky, and the table that brings it into the level's palette.
        let sky = manifest
            .slot("sky")
            .and_then(|(dir, file)| read(dir, file))
            .and_then(|b| Image::parse_guessed(&b).ok().flatten());
        let vox = manifest
            .slot("sky")
            .map(|(_, file)| file.to_ascii_lowercase())
            .filter(|file| file.ends_with(".vox"));
        let star_passes = match vox.as_deref() {
            Some("space.vox") => 2,
            Some(_) => 1,
            None => 0,
        };
        let stars = if star_passes > 0 { stars() } else { Vec::new() };
        let sky_remap = (|| {
            let (dir, name) = manifest.slot("sky_palette")?;
            let sky_palette = Palette::parse(&read(dir, name)?).ok()?;
            // The .MAP belongs to the level's palette, not to the level: it
            // answers "the nearest index in this palette", so it is named
            // after the palette. All 26 levels resolve that way - IOWAH,
            // IOWAH2 and IOWAH3 use IOWA.ACT and so IOWA.MAP - where naming it
            // after the level finds 11 and after the level family 21.
            let (_, ground_palette) = manifest.slot("ground_palette")?;
            let palette_stem = ground_palette.split('.').next()?;
            let map = read("fog", &format!("{palette_stem}.map"))?;
            let map = ColourMap::parse(&map).ok()?;
            let mut table = [0u8; 256];
            for (i, slot) in table.iter_mut().enumerate() {
                let entry = (i as u8).wrapping_sub(crate::level::SKY_BIAS);
                let [r, g, b] = sky_palette.rgb(entry);
                *slot = map.lookup(r, g, b);
            }
            Some(table)
        })();

        // The animated textures.
        //
        // An animation's frames are mostly **not** in the level's `.TEX` list:
        // 140 of the 146 across the 11 levels with an `.ANI` name at least one
        // texture that is not. The engine keeps a single 1,024-entry texture
        // table that both the terrain list and the animation frames register
        // into - `"Too many flippin textures 1"` is its overflow check - so a
        // frame is just another texture loaded by name from `ART\`.
        //
        // The port mirrors that by appending the frames to the texture list,
        // so a slot index past the `.TEX` names is an animation frame.
        let mut textures: Vec<Option<Image>> = texture_names
            .iter()
            .map(|n| read("art", n).and_then(|b| Image::parse_guessed(&b).ok().flatten()))
            .collect();
        let mut extra: Vec<String> = Vec::new();
        let index_of = |name: &str,
                            textures: &mut Vec<Option<Image>>,
                            extra: &mut Vec<String>|
         -> Option<usize> {
            if let Some(i) = texture_names.iter().position(|n| n.eq_ignore_ascii_case(name)) {
                return Some(i);
            }
            if let Some(i) = extra.iter().position(|n| n.eq_ignore_ascii_case(name)) {
                return Some(texture_names.len() + i);
            }
            let image = read("art", name).and_then(|b| Image::parse_guessed(&b).ok().flatten())?;
            extra.push(name.to_string());
            textures.push(Some(image));
            Some(textures.len() - 1)
        };
        let animations: Vec<Cycle> = manifest
            .slot("animations")
            .and_then(|(dir, file)| read(dir, file))
            .and_then(|b| text::animations(&b).ok())
            .map(|list| {
                list.into_iter()
                    .filter_map(|a| {
                        // The base has to be a terrain texture; a frame need
                        // only exist in the archive.
                        let base = texture_names
                            .iter()
                            .position(|n| n.eq_ignore_ascii_case(&a.base))?;
                        let frames: Vec<usize> = a
                            .frames
                            .iter()
                            .filter_map(|f| index_of(f, &mut textures, &mut extra))
                            .collect();
                        Some(Cycle { base, delay: a.delay as f32 / 65536.0, frames })
                    })
                    .filter(|c| c.frames.len() > 1)
                    .collect()
            })
            .unwrap_or_default();

        // Wrecks go after the types in the same mesh list. `cube.bin` means no
        // wreck at all and is not loaded as one.
        let mut meshes = meshes;
        let mut mesh_textures = mesh_textures;
        let mut wreck_mesh = Vec::with_capacity(kinds.len());
        let mut mesh_radius: Vec<i32> = kinds.iter().map(EnemyDef::radius).collect();
        for kind in &kinds {
            if kind.wreck.eq_ignore_ascii_case("cube.bin") {
                wreck_mesh.push(None);
                continue;
            }
            let model = read("models", &kind.wreck).and_then(|b| mrgl::Model::parse(&b).ok());
            match model {
                Some(model) => {
                    let textures = model
                        .materials
                        .iter()
                        .map(|n| read("art", n).and_then(|b| Image::parse_guessed(&b).ok().flatten()))
                        .collect();
                    meshes.push(Some(model));
                    mesh_textures.push(textures);
                    mesh_radius.push(kind.radius());
                    wreck_mesh.push(Some(meshes.len() - 1));
                }
                None => wreck_mesh.push(None),
            }
        }
        // The powerups' models follow, from `STARTUP.POD`. They have no type
        // to take a radius from, so they are drawn at their own scale: a
        // model unit is `1 / unit` of a world unit, which in the renderer's
        // terms (vertex times scale over 2^14) is a scale of `2^30 / unit`.
        let mut powerup_mesh = Vec::with_capacity(hb_sim::powerup::KINDS.len());
        let mut powerup_size = Vec::with_capacity(hb_sim::powerup::KINDS.len());
        for (_, name) in hb_sim::powerup::KINDS {
            let model = read("models", name).and_then(|b| mrgl::Model::parse(&b).ok());
            match model.filter(|m| m.unit.is_some_and(|u| u > 0)) {
                Some(model) => {
                    let textures = model
                        .materials
                        .iter()
                        .map(|n| read("art", n).and_then(|b| Image::parse_guessed(&b).ok().flatten()))
                        .collect();
                    powerup_size.push(hb_sim::powerup::size_of(&model));
                    mesh_radius.push(((1i64 << 30) / model.unit.unwrap_or(1) as i64) as i32);
                    meshes.push(Some(model));
                    mesh_textures.push(textures);
                    powerup_mesh.push(Some(meshes.len() - 1));
                }
                None => {
                    powerup_mesh.push(None);
                    powerup_size.push(0.0);
                }
            }
        }
        let mesh_flipbooks = meshes
            .iter()
            .map(|m| match m {
                Some(model) => model
                    .flipbooks
                    .iter()
                    .map(|book| Flip {
                        material: book.material,
                        frames: book
                            .frames
                            .iter()
                            .map(|n| read("art", n).and_then(|b| Image::parse_guessed(&b).ok().flatten()))
                            .collect(),
                        period: book.period as f32 / 65536.0,
                    })
                    .collect(),
                None => Vec::new(),
            })
            .collect();
        // The shots' models, by weapon kind, and the Valkyrie's three
        // muzzle flashes after them. The engine draws a shot at size 1.0
        // (`0x4769e9`), which is this renderer's scale of 1.0 in 16.16.
        let mut shot_mesh = Vec::with_capacity(hb_sim::weapons::ROWS.len());
        let load_shot = |name: &str,
                             meshes: &mut Vec<Option<mrgl::Model>>,
                             mesh_textures: &mut Vec<Vec<Option<Image>>>,
                             mesh_radius: &mut Vec<i32>|
         -> Option<usize> {
            let model = read("models", name).and_then(|b| mrgl::Model::parse(&b).ok())?;
            let textures = model
                .materials
                .iter()
                .map(|n| read("art", n).and_then(|b| Image::parse_guessed(&b).ok().flatten()))
                .collect();
            meshes.push(Some(model));
            mesh_textures.push(textures);
            mesh_radius.push(1 << 16);
            Some(meshes.len() - 1)
        };
        for row in hb_sim::weapons::ROWS {
            let slot = (row.draw <= 1 && !row.model.is_empty())
                .then(|| load_shot(row.model, &mut meshes, &mut mesh_textures, &mut mesh_radius))
                .flatten();
            shot_mesh.push(slot);
        }
        let muzzle_mesh = hb_sim::weapons::MUZZLE
            .map(|name| load_shot(name, &mut meshes, &mut mesh_textures, &mut mesh_radius));

        // The explosion's sixteen frames, `blast1.raw` to `blast16.raw`.
        let blast: Vec<Option<Image>> = (1..=hb_sim::explosion::FRAMES)
            .map(|n| read("art", &format!("blast{n}.raw")).and_then(|b| Image::parse_guessed(&b).ok().flatten()))
            .collect();

        let quake = manifest
            .slot("quake")
            .and_then(|(dir, file)| read(dir, file))
            .map(|b| hb_formats::quake::parse(&b).map_err(|e| e.to_string()))
            .transpose()?
            .unwrap_or_default();

        let powerups = manifest
            .slot("powerups")
            .and_then(|(dir, file)| read(dir, file))
            .map(|b| text::powerups(&b).map_err(|e| e.to_string()))
            .transpose()?
            .unwrap_or_default();

        // `.DEF` line 24 is the destroy sound. One type names `CRY-DES.DEF`
        // where it means the `.WAV`; that one resolves to nothing.
        let destroy_sound = kinds
            .iter()
            .map(|k| {
                let name = k.raw.get(24)?.trim();
                if name.eq_ignore_ascii_case("null") || name.is_empty() {
                    return None;
                }
                read("sound", name)
            })
            .collect();

        let navs = match manifest.slot("navigation").and_then(|(dir, file)| read(dir, file)) {
            Some(bytes) => hb_formats::nav::navs(&bytes).map_err(|e| e.to_string())?,
            None => Vec::new(),
        };

        let courses = read("data", &manifest.courses)
            .and_then(|b| hb_formats::course::parse(&b).ok())
            .unwrap_or_default();

        // Smaller copies of every texture, for the resolution the engine picks
        // by distance. How the engine made its smaller copies is not read;
        // these average each 2x2 in colour and look the result up in the
        // level's `.MAP`, the table that answers "nearest index in this
        // palette" and the one the engine itself uses for that question.
        let colour_map = (|| {
            let (_, ground_palette) = manifest.slot("ground_palette")?;
            let stem = ground_palette.split('.').next()?;
            ColourMap::parse(&read("fog", &format!("{stem}.map"))?).ok()
        })();
        let mips = textures
            .iter()
            .map(|t| match t {
                Some(image) => {
                    let half = halve(image, &palette, colour_map.as_ref());
                    let quarter = half.as_ref().and_then(|h| halve(h, &palette, colour_map.as_ref()));
                    [half, quarter]
                }
                None => [None, None],
            })
            .collect();

        Ok(Level {
            blast,
            shot_mesh,
            muzzle_mesh,
            mesh_flipbooks,
            navs,
            powerup_mesh,
            powerup_size,
            powerups,
            quake,
            animated,
            mips,
            wreck_mesh,
            destroy_sound,
            courses,
            animations,
            sky,
            sky_remap,
            stars,
            star_passes,
            light: ramp("light"),
            fog: ramp("fog"),
            kinds,
            placements,
            meshes,
            mesh_textures,
            mesh_radius,
            manifest,
            stem,
            terrain,
            textures,
            texture_names,
            palette,
        })
    }

    pub fn resolved_textures(&self) -> usize {
        self.textures.iter().filter(|t| t.is_some()).count()
    }

    pub fn scene(&self) -> crate::scene::Scene<'_> {
        crate::scene::Scene {
            grid: hb_world::Grid::new(&self.terrain),
            textures: &self.textures,
            mips: &self.mips,
            palette: &self.palette,
            light: self.light.as_ref(),
            fog: self.fog.as_ref(),
            frames: None,
            sky: self.sky.as_ref(),
            sky_remap: self.sky_remap.as_ref(),
            placements: &self.placements,
            meshes: &self.meshes,
            mesh_textures: &self.mesh_textures,
            mesh_radius: &self.mesh_radius,
            mesh_flipbooks: &self.mesh_flipbooks,
            seconds: 0.0,
            sun: {
                let v = self.manifest.light.map(|c| c as f32 / 65536.0);
                let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if length > 0.0 { v.map(|c| c / length) } else { [0.0, -1.0, 0.0] }
            },
            sun_ambient: (self.manifest.ambient as f32 / 65536.0).clamp(0.0, 1.0),
            ambient: (self.manifest.ambient >> 8).clamp(0, 255) as u8,
            sky_scroll: [0.0, 0.0],
            sky_height: self.sky_height(),
            stars: &self.stars,
            star_passes: self.star_passes,
        }
    }

    /// Which texture index each slot resolves to at a given time, so the
    /// renderer can stay ignorant of animation.
    ///
    /// The identity, except where an `.ANI` cycle replaces its base texture
    /// with one of its frames.
    pub fn texture_frames(&self, seconds: f32) -> Vec<u16> {
        let mut out: Vec<u16> = (0..self.textures.len() as u16).collect();
        for cycle in &self.animations {
            if cycle.delay <= 0.0 || cycle.base >= out.len() {
                continue;
            }
            let step = (seconds / cycle.delay) as usize % cycle.frames.len();
            out[cycle.base] = cycle.frames[step] as u16;
        }
        out
    }

    /// How many placed objects have a mesh that could be loaded.
    pub fn drawable_placements(&self) -> usize {
        self.placements
            .iter()
            .filter(|p| self.meshes.get(p.kind).is_some_and(Option::is_some))
            .count()
    }
}

/// An image at half size: each 2x2 block averaged in colour and turned back
/// into an index through the palette's `.MAP`, or by nearest colour without
/// one.
fn halve(image: &Image, palette: &Palette, map: Option<&ColourMap>) -> Option<Image> {
    let (w, h) = (image.shape.width, image.shape.height);
    if w < 2 || h < 2 || w % 2 != 0 || h % 2 != 0 {
        return None;
    }
    let (hw, hh) = (w / 2, h / 2);
    let mut pixels = Vec::with_capacity(hw * hh);
    for y in 0..hh {
        for x in 0..hw {
            let mut sum = [0u32; 3];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let [r, g, b] = palette.rgb(image.pixels[(2 * y + dy) * w + 2 * x + dx]);
                sum[0] += r as u32;
                sum[1] += g as u32;
                sum[2] += b as u32;
            }
            let [r, g, b] = sum.map(|c| (c / 4) as u8);
            let index = match map {
                Some(map) => map.lookup(r, g, b),
                None => (0..240u16)
                    .min_by_key(|&i| {
                        let [pr, pg, pb] = palette.rgb(i as u8);
                        let d = [pr as i32 - r as i32, pg as i32 - g as i32, pb as i32 - b as i32];
                        d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
                    })
                    .unwrap_or(0) as u8,
            };
            pixels.push(index);
        }
    }
    Some(Image { shape: hb_formats::raw::Shape::new(hw, hh), pixels })
}
