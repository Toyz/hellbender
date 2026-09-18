//! The colour tables: `.MAP`, `.LTE`, `.FOG`, `.MIX`.
//!
//! The engine renders in 8-bit indexed colour, so shading, fogging and
//! blending are all table lookups. Every table maps index to index and is
//! authored rather than computed - a port that wants the original's exact
//! output has to use them.

use crate::{Error, Result};

/// `.MAP`: 15-bit colour to palette index, `(r5 << 10) | (g5 << 5) | b5`.
#[derive(Debug, Clone)]
pub struct ColourMap(pub Vec<u8>);

impl ColourMap {
    pub const ENTRIES: usize = 32_768;

    pub fn parse(data: &[u8]) -> Result<ColourMap> {
        if data.len() != Self::ENTRIES {
            return Err(Error::WrongSize {
                what: "MAP colour table",
                want: format!("{} bytes", Self::ENTRIES),
                have: data.len(),
            });
        }
        Ok(ColourMap(data.to_vec()))
    }

    pub fn lookup(&self, r: u8, g: u8, b: u8) -> u8 {
        let i = ((r as usize >> 3) << 10) | ((g as usize >> 3) << 5) | (b as usize >> 3);
        self.0[i]
    }
}

/// `.LTE` and `.FOG`: 16 rows of 256, `table[shade][index] -> index`.
///
/// Row 0 is the identity in both. At the far end they differ, and so does what
/// they do with the reserved indices: `.LTE` row 15 sends 0..[`SHADED`] to
/// index 0, black, and passes the reserved indices through untouched, while
/// `.FOG` row 15 sends every index without exception to 255, the fog colour.
#[derive(Debug, Clone)]
pub struct Ramp {
    pub levels: usize,
    pub table: Vec<u8>,
}

/// The palette's shadeable range. Indices 240 and above are reserved: a
/// `.LTE` leaves them alone at every level and a `.MIX` refuses to blend them.
/// They are the colours the HUD and the front end need to stay exact.
pub const SHADED: usize = 240;

impl Ramp {
    pub const LEVELS: usize = 16;
    pub const BYTES: usize = Self::LEVELS * 256;

    pub fn parse(data: &[u8]) -> Result<Ramp> {
        if data.is_empty() || data.len() % 256 != 0 {
            return Err(Error::WrongSize {
                what: "LTE/FOG ramp",
                want: "a multiple of 256".into(),
                have: data.len(),
            });
        }
        Ok(Ramp { levels: data.len() / 256, table: data.to_vec() })
    }

    pub fn shade(&self, level: usize, index: u8) -> u8 {
        self.table[level * 256 + index as usize]
    }
}

/// `.MIX`: 256 x 256, `mix[a][b] -> index`. Symmetric. Row 0 is the identity
/// over the shadeable range and sends the reserved indices to 0, so blending
/// with index 0 is the no-op the renderer wants for the colours it may touch.
#[derive(Debug, Clone)]
pub struct BlendTable(pub Vec<u8>);

impl BlendTable {
    pub const BYTES: usize = 256 * 256;

    pub fn parse(data: &[u8]) -> Result<BlendTable> {
        if data.len() != Self::BYTES {
            return Err(Error::WrongSize {
                what: "MIX blend table",
                want: format!("{} bytes", Self::BYTES),
                have: data.len(),
            });
        }
        Ok(BlendTable(data.to_vec()))
    }

    pub fn blend(&self, a: u8, b: u8) -> u8 {
        self.0[a as usize * 256 + b as usize]
    }

    pub fn is_symmetric(&self) -> bool {
        (0..256).all(|a| (0..256).all(|b| self.0[a * 256 + b] == self.0[b * 256 + a]))
    }
}
