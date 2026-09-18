//! Everything a level needs to be drawn, pulled out of the archives.
//!
//! The engine loads these from the names on the level's [.LVL] manifest, plus
//! the thirteen terrain grids whose names it derives from the stem. This
//! assembles the same set.

use hb_formats::act::Palette;
use hb_formats::colour::Ramp;
use hb_formats::lvl::Level as Manifest;
use hb_formats::raw::Image;
use hb_formats::terrain::Terrain;
use hb_formats::text;
use hb_pod::Pod;

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
        let textures = texture_names
            .iter()
            .map(|n| read("art", n).and_then(|b| Image::parse_guessed(&b).ok().flatten()))
            .collect();

        let (_, palette_name) =
            manifest.slot("ground_palette").ok_or("no ground palette slot")?;
        let palette_bytes = read("art", palette_name)
            .ok_or_else(|| format!("no art\\{palette_name}"))?;
        let palette = Palette::parse(&palette_bytes).map_err(|e| e.to_string())?;

        let ramp = |slot: &str| -> Option<Ramp> {
            let (dir, file) = manifest.slot(slot)?;
            Ramp::parse(&read(dir, file)?).ok()
        };

        Ok(Level {
            light: ramp("light"),
            fog: ramp("fog"),
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
        }
    }
}
