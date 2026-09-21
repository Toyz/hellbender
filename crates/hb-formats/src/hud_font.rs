//! The small font the HUD is written in, which lives in the executable.
//!
//! Not the [`crate::font`] `FONT.BIN` - that one is 23 pixels tall and belongs
//! to the front end. Every line the game draws over the cockpit is in this
//! one: five pixels tall in a six-row cell, with a seven-pixel line height.
//!
//! It is a table at virtual address `0x50f530`, 48 bytes a character for
//! characters 0x20 to 0xff. The first byte is the glyph's width; the rest is
//! one byte a pixel, `width` across and six down, 1 where the pixel is set.
//! The width routine (`0x485a00`) indexes it as `char * 48` and adds one to
//! each width for the gap between letters, which is how the engine measures a
//! string before it centres it.
//!
//! Reading it means reading `HELLBEND.EXE`, which is on the disc beside the
//! archives, so it is input like any other file. Nothing of it is copied into
//! this repository.

use crate::{Error, Result};

/// Where the table is, as a virtual address in the retail 1.00 image.
pub const TABLE: u32 = 0x50f530;

/// Bytes a character.
pub const STRIDE: usize = 48;

/// Rows in a glyph's cell. A descender uses the last of them.
pub const ROWS: usize = 6;

/// What one line of text takes, from `0x4810a2`: `lines * 7`.
pub const LINE: usize = 7;

/// The first character in the table; below it the engine draws nothing.
pub const FIRST: usize = 0x20;

#[derive(Debug, Clone, Default)]
pub struct Glyph {
    pub width: usize,
    /// `width * ROWS` bytes, a row at a time, 1 where the pixel is set.
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct HudFont {
    /// Indexed by character code; entries below [`FIRST`] are empty.
    pub glyphs: Vec<Glyph>,
}

impl HudFont {
    /// Read the table out of a PE image.
    pub fn read(exe: &[u8]) -> Result<HudFont> {
        let at = offset_of(exe, TABLE)?;
        let mut glyphs = vec![Glyph::default(); 256];
        for (code, glyph) in glyphs.iter_mut().enumerate().skip(FIRST) {
            let start = at + code * STRIDE;
            let record = exe.get(start..start + STRIDE).ok_or(Error::Truncated {
                what: "HUD font glyph",
                at: start,
                need: STRIDE,
                have: exe.len(),
            })?;
            let width = record[0] as usize;
            if width == 0 || width * ROWS > STRIDE - 1 {
                continue;
            }
            glyph.width = width;
            glyph.pixels = record[1..1 + width * ROWS].to_vec();
        }
        Ok(HudFont { glyphs })
    }

    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        let code = c as usize;
        self.glyphs.get(code).filter(|g| g.width > 0)
    }

    /// How wide a string is, the engine's way: each glyph's width plus one
    /// (`0x485a31`).
    pub fn width(&self, text: &str) -> usize {
        text.chars().filter_map(|c| self.glyph(c)).map(|g| g.width + 1).sum()
    }

    /// Draw a line into an 8-bit frame. `colour` is the palette index every
    /// set pixel takes.
    pub fn draw(
        &self,
        frame: &mut [u8],
        width: usize,
        height: usize,
        x: isize,
        y: isize,
        text: &str,
        colour: u8,
    ) {
        let mut pen = x;
        for c in text.chars() {
            let Some(glyph) = self.glyph(c) else { continue };
            for row in 0..ROWS {
                let py = y + row as isize;
                if py < 0 || py >= height as isize {
                    continue;
                }
                for column in 0..glyph.width {
                    if glyph.pixels[row * glyph.width + column] == 0 {
                        continue;
                    }
                    let px = pen + column as isize;
                    if px < 0 || px >= width as isize {
                        continue;
                    }
                    frame[py as usize * width + px as usize] = colour;
                }
            }
            pen += glyph.width as isize + 1;
        }
    }
}

/// Where a virtual address lands in the file, through the PE section table.
fn offset_of(exe: &[u8], va: u32) -> Result<usize> {
    let bad = |what: &'static str| Error::BadTag { what, tag: va, at: 0 };
    if exe.get(..2) != Some(b"MZ") {
        return Err(bad("not a PE image"));
    }
    let pe = u32_at(exe, 0x3c)? as usize;
    if exe.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Err(bad("no PE header"));
    }
    let sections = u16_at(exe, pe + 6)? as usize;
    let optional = u16_at(exe, pe + 20)? as usize;
    let base = u32_at(exe, pe + 24 + 28)?;
    let table = pe + 24 + optional;
    for i in 0..sections {
        let entry = table + i * 40;
        let start = u32_at(exe, entry + 12)? + base;
        let size = u32_at(exe, entry + 16)?;
        let raw = u32_at(exe, entry + 20)? as usize;
        if va >= start && va < start + size {
            return Ok(raw + (va - start) as usize);
        }
    }
    Err(bad("address is in no section"))
}

fn u16_at(data: &[u8], at: usize) -> Result<u16> {
    let bytes = data.get(at..at + 2).ok_or(Error::Truncated {
        what: "PE header",
        at,
        need: 2,
        have: data.len(),
    })?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn u32_at(data: &[u8], at: usize) -> Result<u32> {
    let bytes = data.get(at..at + 4).ok_or(Error::Truncated {
        what: "PE header",
        at,
        need: 4,
        have: data.len(),
    })?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}
