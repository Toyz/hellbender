//! `STARTUP\FONT.BIN` and `FONT.NDX`: the front end's typeface.
//!
//! 95 glyphs, one per printable ASCII character from space to `~`, each a
//! bitmap of its own width and a common height of 23. The `.NDX` is the width
//! list as text; the `.BIN` is the pixels, glyph after glyph, row major.
//!
//! That the height is 23 is arithmetic rather than a guess: the widths sum to
//! 1,137 and the bitmap is 26,151 bytes, which is 1,137 x 23 exactly.

use crate::text::lines;
use crate::{Error, Result};

pub const FIRST: u8 = b' ';
pub const GLYPHS: usize = 95;
pub const HEIGHT: usize = 23;

#[derive(Debug, Clone)]
pub struct Font {
    pub widths: Vec<usize>,
    /// Where each glyph starts in `pixels`.
    offsets: Vec<usize>,
    pub pixels: Vec<u8>,
}

impl Font {
    /// `index` is the `.NDX` text, `bitmap` the `.BIN` bytes.
    pub fn parse(index: &[u8], bitmap: &[u8]) -> Result<Font> {
        let widths: Vec<usize> = lines(index)
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().parse::<usize>().unwrap_or(0))
            .collect();
        if widths.len() != GLYPHS {
            return Err(Error::WrongSize {
                what: "font index",
                want: format!("{GLYPHS} widths"),
                have: widths.len(),
            });
        }
        let want: usize = widths.iter().sum::<usize>() * HEIGHT;
        if bitmap.len() != want {
            return Err(Error::WrongSize {
                what: "font bitmap",
                want: format!("{want} bytes"),
                have: bitmap.len(),
            });
        }
        let mut offsets = Vec::with_capacity(GLYPHS + 1);
        let mut at = 0;
        for &w in &widths {
            offsets.push(at);
            at += w * HEIGHT;
        }
        offsets.push(at);
        Ok(Font { widths, offsets, pixels: bitmap.to_vec() })
    }

    fn slot(&self, ch: char) -> Option<usize> {
        let byte = ch as u32;
        if !(FIRST as u32..FIRST as u32 + GLYPHS as u32).contains(&byte) {
            return None;
        }
        Some((byte - FIRST as u32) as usize)
    }

    pub fn width_of(&self, ch: char) -> usize {
        self.slot(ch).map(|i| self.widths[i]).unwrap_or(0)
    }

    pub fn measure(&self, text: &str) -> usize {
        text.chars().map(|c| self.width_of(c)).sum()
    }

    /// The glyph's pixels, row major, `width_of(ch)` wide and [`HEIGHT`] tall.
    pub fn glyph(&self, ch: char) -> Option<(&[u8], usize)> {
        let i = self.slot(ch)?;
        Some((&self.pixels[self.offsets[i]..self.offsets[i + 1]], self.widths[i]))
    }

    /// Draw into an 8-bit buffer, treating index 0 as transparent.
    ///
    /// `recolour` replaces every non-zero pixel, for drawing the same face in
    /// a colour the frame's palette has.
    pub fn draw(
        &self,
        target: &mut [u8],
        stride: usize,
        height: usize,
        x: isize,
        y: isize,
        text: &str,
        recolour: Option<u8>,
    ) -> usize {
        let mut pen = x;
        for ch in text.chars() {
            let Some((glyph, w)) = self.glyph(ch) else { continue };
            for gy in 0..HEIGHT {
                let ty = y + gy as isize;
                if ty < 0 || ty as usize >= height {
                    continue;
                }
                for gx in 0..w {
                    let tx = pen + gx as isize;
                    if tx < 0 || tx as usize >= stride {
                        continue;
                    }
                    let value = glyph[gy * w + gx];
                    if value != 0 {
                        target[ty as usize * stride + tx as usize] =
                            recolour.unwrap_or(value);
                    }
                }
            }
            pen += w as isize;
        }
        (pen - x).max(0) as usize
    }
}
