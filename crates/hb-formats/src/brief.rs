//! The mission briefing: `DATA\<stem>.TXT`, named by line 0 of the
//! [`.LVL`](crate::lvl).
//!
//! Not a model and not a placement list - `MODELS\*.TXT` is an animated
//! model and shares only the extension. This is two file names and some
//! prose:
//!
//! ```text
//! Globe.Bin                     a model, turned on the briefing screen
//! Morbos00.Raw                  the texture it wears - the planet's surface
//! PLANET: Morbos                the briefing itself, as many lines as it
//! MISSION: Counterstrike        likes
//! ...
//! .                             a line holding one dot ends it
//! ```

use crate::{Error, Result};

/// The screen every briefing is drawn on, 320x200 in `ART\BRIEF.ACT`. The
/// engine opens it by name (`0x50c594`, whose failure is
/// `Unable to open brief.raw`); the `.TXT` never mentions it.
pub const BACKDROP: &str = "brief.raw";

/// The dark panel in the upper half of that screen, in its own 320x200:
/// x, y, width, height. Measured off the art - the frame is an oval and
/// this is the rectangle inside it that has nothing drawn on it. There is a
/// second, shorter panel below the two ornaments at y 115 to 140, which this
/// does not use.
pub const PANEL: [usize; 4] = [40, 26, 238, 86];

/// The engine types the briefing out a character at a time: `0x45a034` takes
/// one byte, draws it (`0x485da0`), measures it (`0x485a00`) and moves the
/// pen on, with the loop bounded by how many characters have been reached so
/// far. This is how many a second, which the port picks - the engine's rate
/// comes from a clock this reading did not follow.
pub const TYPED_A_SECOND: f32 = 40.0;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Brief {
    /// The model the briefing screen turns, `globe.bin` in every level that
    /// names one.
    pub model: String,
    /// The texture it wears: the planet, 64x64, in a palette of its own.
    /// The screen behind it is `ART\BRIEF.RAW`, which no briefing names
    /// because every one of them uses it ([`BACKDROP`]).
    pub texture: String,
    /// The prose, one entry a line, with the blank lines kept - they are the
    /// paragraph breaks.
    pub lines: Vec<String>,
}

impl Brief {
    pub fn parse(data: impl AsRef<[u8]>) -> Result<Brief> {
        let text = String::from_utf8_lossy(data.as_ref()).into_owned();
        let mut lines = text.lines().map(|l| l.trim_end_matches('\r'));
        let model = lines
            .next()
            .ok_or(Error::Truncated { what: "briefing model", at: 0, need: 1, have: 0 })?
            .trim()
            .to_string();
        let texture = lines
            .next()
            .ok_or(Error::Truncated { what: "briefing texture", at: 0, need: 1, have: 0 })?
            .trim()
            .to_string();
        let mut prose = Vec::new();
        for line in lines {
            if line.trim() == "." {
                break;
            }
            prose.push(line.trim_end().to_string());
        }
        // The prose often starts and ends with a blank line; the screen does
        // not need them.
        while prose.first().is_some_and(|l| l.is_empty()) {
            prose.remove(0);
        }
        while prose.last().is_some_and(|l| l.is_empty()) {
            prose.pop();
        }
        Ok(Brief { model, texture, lines: prose })
    }

    /// The planet and the mission, which are the first two lines of every
    /// shipped briefing, without their labels.
    pub fn headline(&self) -> (Option<&str>, Option<&str>) {
        let after = |prefix: &str| {
            self.lines
                .iter()
                .find(|l| l.to_ascii_uppercase().starts_with(prefix))
                .and_then(|l| l.split_once(':'))
                .map(|(_, rest)| rest.trim())
        };
        (after("PLANET"), after("MISSION"))
    }
}
