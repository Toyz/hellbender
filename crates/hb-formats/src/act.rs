//! The `.ACT` palette: 768 bytes, 256 RGB triples, full 0-255 range.

use crate::{Error, Result};

pub const ENTRIES: usize = 256;
pub const BYTES: usize = ENTRIES * 3;

#[derive(Debug, Clone)]
pub struct Palette {
    pub colours: [[u8; 3]; ENTRIES],
}

impl Default for Palette {
    fn default() -> Self {
        Palette { colours: [[0; 3]; ENTRIES] }
    }
}

impl Palette {
    pub fn parse(data: &[u8]) -> Result<Palette> {
        if data.len() != BYTES {
            return Err(Error::WrongSize {
                what: "ACT palette",
                want: format!("{BYTES} bytes"),
                have: data.len(),
            });
        }
        let mut colours = [[0u8; 3]; ENTRIES];
        for (i, slot) in colours.iter_mut().enumerate() {
            slot.copy_from_slice(&data[i * 3..i * 3 + 3]);
        }
        Ok(Palette { colours })
    }

    /// A zero-length `.ACT` is a legal placeholder; two ship in the archives.
    pub fn parse_lenient(data: &[u8]) -> Result<Option<Palette>> {
        if data.is_empty() {
            Ok(None)
        } else {
            Palette::parse(data).map(Some)
        }
    }

    pub fn rgb(&self, index: u8) -> [u8; 3] {
        self.colours[index as usize]
    }

    /// The index nearest a colour, as the engine measures it (`0x485080`):
    /// `29|dr| + 58|dg| + 15|db|`, the first of equals.
    pub fn nearest(&self, [r, g, b]: [u8; 3]) -> u8 {
        let d = |a: u8, b: u8| (a as i64 - b as i64).abs();
        (0..=255u8)
            .min_by_key(|&i| {
                let [pr, pg, pb] = self.rgb(i);
                29 * d(r, pr) + 58 * d(g, pg) + 15 * d(b, pb)
            })
            .unwrap_or(0)
    }

    /// The index among `range` nearest a colour by straight-line distance -
    /// the port's own measure, for colours it picks itself.
    pub fn nearest_in(&self, [r, g, b]: [u8; 3], range: std::ops::RangeInclusive<u8>) -> u8 {
        let d = |a: u8, b: u8| (a as i32 - b as i32).pow(2);
        let start = *range.start();
        range
            .min_by_key(|&i| {
                let [pr, pg, pb] = self.rgb(i);
                d(r, pr) + d(g, pg) + d(b, pb)
            })
            .unwrap_or(start)
    }
}
