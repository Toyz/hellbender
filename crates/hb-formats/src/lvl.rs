//! The `.LVL` manifest: 43 lines of CRLF text naming everything a level is
//! made of. Positional, not keyed - see `docs/formats/lvl.md`.

use crate::text::{expect_sentinel, int, ints, lines, optional_name};
use crate::{Error, Result};

pub const LINES: usize = 43;
pub const VERSION: i64 = 4;

/// Which POD directory each filename slot resolves against. The engine
/// addresses entries as (directory, name), so the directory is fixed per slot
/// and never appears in the file.
pub const DIRS: [(&str, &str); 16] = [
    ("briefing", "data"),
    ("ground_altitude", "data"),
    ("ground_colour", "data"),
    ("ground_palette", "art"),
    ("textures", "data"),
    ("quake", "data"),
    ("powerups", "data"),
    ("animations", "data"),
    ("tdf", "data"),
    ("sky", "art"),
    ("sky_palette", "art"),
    ("enemies", "data"),
    ("navigation", "data"),
    ("music", "music"),
    ("fog", "fog"),
    ("light", "fog"),
];

#[derive(Debug, Clone, Default)]
pub struct Level {
    pub version: i64,
    /// Lines 2-17, in order, matching [`DIRS`].
    pub files: Vec<String>,
    /// Line 18: the light direction, a 16.16 unit vector - `-46333,-46333,0`,
    /// (-0.707, -0.707, 0), in every level. The parser at `0x44bb88` reads it
    /// into the level struct at `+0x284`, which is `0x666f34`, and the shading
    /// computation at `0x41c5d0` lights the ground and box set A with it.
    pub light: [i64; 3],
    /// Line 19: the ambient intensity for the same, 16.16 (`0x666f40`). It is
    /// each level's floor in the ground shading - 16384 in `HOTH` whose
    /// shading bottoms out at 64, 40960 in `FLOAT` at 160 - and a ground
    /// vertex whose shade word has bit 8 set takes it in place of its own
    /// shade (`0x414e10`).
    pub ambient: i64,
    /// Lines 20 and 21: the direction and ambient the same computation lights
    /// the chambers with (`0x666f44`, `0x666f50`, read at `0x41d161` beside the
    /// chamber array). `HOTH2` and `HOTH3` give no direction.
    pub chamber_light: [i64; 3],
    pub chamber_ambient: i64,
    /// Line 22, read into `0x666f54`.
    pub unknown_22: i64,
    /// Lines 24-28, `None` where the file says `null`.
    pub story_movies: [Option<String>; 5],
    pub unknown_30: i64,
    pub courses: String,
    pub glt: String,
    pub unknown_33: i64,
    /// Line 35. CD audio track, 0 for none.
    pub redbook_track: i64,
    pub briefing_movie: Option<String>,
    pub death_movie: Option<String>,
    /// Line 40. 0 none, 1 snow, 2 rain, 4 lightning; 6 is rain and lightning.
    pub weather: i64,
    /// Lines 41 and 42, 16.16 fixed point. Line 42 is 15.0, 30.0 in all 26.
    pub weather_params: [[i64; 2]; 2],
}

impl Level {
    pub fn parse(data: &[u8]) -> Result<Level> {
        let l = lines(data);
        if l.len() < LINES - 1 {
            return Err(Error::BadLine {
                what: "LVL",
                line: l.len(),
                saw: format!("expected {} lines, got {}", LINES - 1, l.len()),
            });
        }
        let version = int(&l, 0, "LVL version")?;
        // The five sentinels are the format's own version markers. Checking
        // them is the cheapest way to catch a file that is not this format.
        expect_sentinel(&l, 22, "LVL ;New story stuff")?;
        expect_sentinel(&l, 28, "LVL !New ground additions")?;
        expect_sentinel(&l, 33, "LVL @Redbook Audio Track")?;
        expect_sentinel(&l, 35, "LVL =New Cinematic Info")?;
        expect_sentinel(&l, 38, "LVL { Weather")?;

        let triple = |at: usize, what: &'static str| -> Result<[i64; 3]> {
            let v = ints(&l, at, what)?;
            if v.len() != 3 {
                return Err(Error::BadLine { what, line: at + 1, saw: l[at].clone() });
            }
            Ok([v[0], v[1], v[2]])
        };
        let pair = |at: usize, what: &'static str| -> Result<[i64; 2]> {
            let v = ints(&l, at, what)?;
            if v.len() != 2 {
                return Err(Error::BadLine { what, line: at + 1, saw: l[at].clone() });
            }
            Ok([v[0], v[1]])
        };
        let opt = |at: usize| l.get(at).and_then(|s| optional_name(s)).map(str::to_string);

        Ok(Level {
            version,
            files: l[1..17].to_vec(),
            light: triple(17, "LVL light")?,
            ambient: int(&l, 18, "LVL ambient")?,
            chamber_light: triple(19, "LVL chamber light")?,
            chamber_ambient: int(&l, 20, "LVL chamber ambient")?,
            unknown_22: int(&l, 21, "LVL line 22")?,
            story_movies: [opt(23), opt(24), opt(25), opt(26), opt(27)],
            unknown_30: int(&l, 29, "LVL line 30")?,
            courses: l[30].clone(),
            glt: l[31].clone(),
            unknown_33: int(&l, 32, "LVL line 33")?,
            redbook_track: int(&l, 34, "LVL line 35")?,
            briefing_movie: opt(36),
            death_movie: opt(37),
            weather: int(&l, 39, "LVL weather")?,
            weather_params: [pair(40, "LVL line 41")?, pair(41, "LVL line 42")?],
        })
    }

    /// The filename in a named slot, with the POD directory it lives in.
    pub fn slot(&self, name: &str) -> Option<(&str, &str)> {
        let i = DIRS.iter().position(|(slot, _)| *slot == name)?;
        Some((DIRS[i].1, self.files.get(i)?.as_str()))
    }

    /// The level stem, taken from the ground altitude filename, which is what
    /// the terrain loader builds its thirteen names from.
    pub fn stem(&self) -> &str {
        self.files
            .get(1)
            .and_then(|f| f.split('.').next())
            .unwrap_or("")
    }

    pub fn has_snow(&self) -> bool {
        self.weather & 1 != 0
    }

    pub fn has_rain(&self) -> bool {
        self.weather & 2 != 0
    }

    pub fn has_lightning(&self) -> bool {
        self.weather & 4 != 0
    }
}
