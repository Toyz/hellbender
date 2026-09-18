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
}
