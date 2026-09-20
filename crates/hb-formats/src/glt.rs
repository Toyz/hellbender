//! `.GLT`: which textures mean a light.
//!
//! The extended ground light table - the loader's own error is
//! `Bad ext ground light`. A record names the texture that marks a lit
//! window, the one that replaces it when the light is off, and the one for a
//! broken light, and carries eight numbers whose meaning is not settled. The
//! reader is `0x48c4c0` and the writer `0x48c6c0`.
//!
//! See `docs/formats/scenery.md`.

use crate::text::{int, ints, lines};
use crate::{Error, Result};

/// One light: three textures and the numbers that go with them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Light {
    /// The texture that says a face is this light, lit.
    pub on: String,
    /// What it wears switched off.
    pub off: String,
    /// And shot out.
    pub broken: String,
    /// The eight numbers, in the order the record holds them: the five of the
    /// fourth line then the three of the fifth. The first is 16.16 and reads
    /// as a size in units; the rest are unidentified.
    pub numbers: [i64; 8],
}

/// The defaults the loader writes when the fourth line carries two numbers
/// instead of five (`0x48c630`), in the same order as [`Light::numbers`].
pub const SHORT_FORM: [i64; 8] = [0, 0, 0, 6, 1, 0, 1, 0];

/// A count, then that many five-line records.
pub fn parse(data: &[u8]) -> Result<Vec<Light>> {
    let lines = lines(data);
    let count = int(&lines, 0, "GLT count")?.max(0) as usize;
    let mut at = 1;
    let mut out = Vec::with_capacity(count);
    let name = |at: usize| -> Result<String> {
        lines.get(at).map(|l| l.trim().to_string()).ok_or(Error::BadLine {
            what: "GLT texture",
            line: at + 1,
            saw: String::new(),
        })
    };
    for _ in 0..count {
        let (on, off, broken) = (name(at)?, name(at + 1)?, name(at + 2)?);
        let first = ints(&lines, at + 3, "GLT numbers")?;
        at += 4;
        let mut numbers = SHORT_FORM;
        match first.len() {
            // The old shape: two numbers and no fifth line.
            2 => {
                numbers[0] = first[0];
                numbers[1] = first[1];
            }
            5 => {
                numbers[..5].copy_from_slice(&first);
                let second = ints(&lines, at, "GLT numbers")?;
                if second.len() != 3 {
                    return Err(Error::BadLine {
                        what: "GLT numbers",
                        line: at + 1,
                        saw: lines[at].clone(),
                    });
                }
                numbers[5..].copy_from_slice(&second);
                at += 1;
            }
            _ => {
                return Err(Error::BadLine {
                    what: "GLT numbers",
                    line: at,
                    saw: lines[at - 1].clone(),
                })
            }
        }
        out.push(Light { on, off, broken, numbers });
    }
    Ok(out)
}

impl Light {
    /// The first number as world units. It is 2, 4, 6 or 10 in every shipped
    /// record.
    pub fn size(&self) -> f32 {
        self.numbers[0] as f32 / 65536.0
    }

    /// The three textures, in the order on, off, broken.
    pub fn textures(&self) -> [&str; 3] {
        [&self.on, &self.off, &self.broken]
    }
}
