//! Hellbender's data formats, decoded.
//!
//! Each module corresponds to a page under `docs/formats/`. Where a format is
//! only partly understood the struct carries the raw field with a numbered
//! name rather than pretending to know what it is.

pub mod act;
pub mod anim;
pub mod colour;
pub mod course;
pub mod lvl;
pub mod mrgl;
pub mod png;
pub mod raw;
pub mod terrain;
pub mod text;

use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// A fixed-size format was not the size it must be.
    WrongSize { what: &'static str, want: String, have: usize },
    /// A record ran past the end of the buffer.
    Truncated { what: &'static str, at: usize, need: usize, have: usize },
    /// A tagged record carried a tag the format does not define.
    BadTag { what: &'static str, tag: u32, at: usize },
    /// A text format's line was missing, or was not what the position requires.
    BadLine { what: &'static str, line: usize, saw: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::WrongSize { what, want, have } => {
                write!(f, "{what}: expected {want}, got {have} bytes")
            }
            Error::Truncated { what, at, need, have } => {
                write!(f, "{what}: record at {at:#x} needs {need} bytes, {have} left")
            }
            Error::BadTag { what, tag, at } => {
                write!(f, "{what}: undefined tag {tag:#x} at {at:#x}")
            }
            Error::BadLine { what, line, saw } => {
                write!(f, "{what}: line {line} is {saw:?}")
            }
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// 16.16 fixed point, the game's arithmetic throughout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Fixed(pub i32);

impl Fixed {
    pub const ONE: Fixed = Fixed(65536);

    pub fn to_f32(self) -> f32 {
        self.0 as f32 / 65536.0
    }

    pub fn from_f32(v: f32) -> Fixed {
        Fixed((v * 65536.0) as i32)
    }
}

impl fmt::Display for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_f32())
    }
}

/// A 16-bit circle: 0 to 65,535 is one full turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Angle(pub u16);

impl Angle {
    pub fn to_radians(self) -> f32 {
        self.0 as f32 * std::f32::consts::TAU / 65536.0
    }

    pub fn to_degrees(self) -> f32 {
        self.0 as f32 * 360.0 / 65536.0
    }
}

pub(crate) fn u16_at(data: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([data[at], data[at + 1]])
}

pub(crate) fn i32_at(data: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

pub(crate) fn u32_at(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

pub(crate) fn cstr(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}
