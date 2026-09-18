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

pub struct Level {
    pub manifest: Manifest,
    pub stem: String,
    pub terrain: Terrain,
    /// The level's `.TEX` list, resolved. An entry is `None` when the named
    /// file is not in either archive or is a zero-length placeholder.
    pub textures: Vec<Option<Image>>,
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
    /// Each mesh's materials resolved to images, in the order the mesh names
    /// them. A model's textures come from `ART\` by name, not from the level's
    /// `.TEX` list.
    pub mesh_textures: Vec<Vec<Option<Image>>>,
    /// The sky texture, 64 x 64. `None` for the levels whose sky slot names a
    /// zero-length `.VOX`, which are the ones set in space.
    pub sky: Option<Image>,
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
        // An animated `.TXT` model is flattened to its first frame so that it
        // can be drawn by the same path as a static one. Playing the animation
        // needs the rotation order, which is not established.
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
                        materials: animated.materials,
                        ..mrgl::Model::default()
                    });
                }
                mrgl::Model::parse(&bytes).ok()
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
        let mut index_of = |name: &str,
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

        Ok(Level {
            animations,
            sky,
            sky_remap,
            light: ramp("light"),
            fog: ramp("fog"),
            kinds,
            placements,
            meshes,
            mesh_textures,
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
            palette: &self.palette,
            light: self.light.as_ref(),
            fog: self.fog.as_ref(),
            frames: None,
            sky: self.sky.as_ref(),
            sky_remap: self.sky_remap.as_ref(),
            placements: &self.placements,
            meshes: &self.meshes,
            mesh_textures: &self.mesh_textures,
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
