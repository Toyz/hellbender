//! The `.RAW` image: 8-bit indexed pixels, no header at all.
//!
//! Dimensions are not in the file. They come from the slot the image is loaded
//! into, and the filename says which slot. `Shape::guess` covers every size
//! that appears in the shipped archives.

use crate::act::Palette;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    pub width: usize,
    pub height: usize,
}

impl Shape {
    pub const fn new(width: usize, height: usize) -> Shape {
        Shape { width, height }
    }

    /// The dimensions for a byte count, for every size present in GAME.POD and
    /// STARTUP.POD. Sizes that more than one shape could explain are not
    /// guessed; ask for the shape explicitly instead.
    pub fn guess(len: usize) -> Option<Shape> {
        Some(match len {
            4_096 => Shape::new(64, 64),        // texture
            16_384 => Shape::new(128, 128),     // terrain grid
            64_000 => Shape::new(320, 200),     // full screen, mode 200
            128_000 => Shape::new(320, 400),    // full screen, mode 400
            307_200 => Shape::new(640, 480),    // full screen, mode 480
            6_440 => Shape::new(140, 46),       // cockpit hand, mode 200
            12_880 => Shape::new(140, 92),      // cockpit hand, mode 400
            30_800 => Shape::new(280, 110),     // cockpit hand, mode 480
            42 => Shape::new(6, 7),             // throttle knob, mode 200
            84 => Shape::new(6, 14),            // throttle knob, mode 400
            204 => Shape::new(12, 17),          // throttle knob, mode 480
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Image {
    pub shape: Shape,
    pub pixels: Vec<u8>,
}

impl Image {
    pub fn parse(data: &[u8], shape: Shape) -> Result<Image> {
        let want = shape.width * shape.height;
        if data.len() != want {
            return Err(Error::WrongSize {
                what: "RAW image",
                want: format!("{want} bytes"),
                have: data.len(),
            });
        }
        Ok(Image { shape, pixels: data.to_vec() })
    }

    /// Parse using `Shape::guess`. A zero-length entry - there are 21 - is not
    /// an error and yields `None`.
    pub fn parse_guessed(data: &[u8]) -> Result<Option<Image>> {
        if data.is_empty() {
            return Ok(None);
        }
        let shape = Shape::guess(data.len()).ok_or(Error::WrongSize {
            what: "RAW image",
            want: "one of the known image sizes".into(),
            have: data.len(),
        })?;
        Image::parse(data, shape).map(Some)
    }

    pub fn to_rgb(&self, palette: &Palette) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 3);
        for &index in &self.pixels {
            out.extend_from_slice(&palette.rgb(index));
        }
        out
    }
}
