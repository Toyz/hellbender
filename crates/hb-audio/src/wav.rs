//! The `.WAV` effects: RIFF, PCM, uncompressed.
//!
//! The shipped effects are 11,025 Hz, mono, 8-bit unsigned, which matches
//! `mixSpeed=11025` in `HELLBEND.INI`. The decoder accepts 8- or 16-bit and
//! any channel count because nothing stops a replacement file differing.

#[derive(Debug, Clone)]
pub struct Wav {
    pub rate: u32,
    pub channels: u16,
    /// Signed 16-bit, interleaved.
    pub samples: Vec<i16>,
}

#[derive(Debug)]
pub enum Error {
    NotRiff,
    NotWave,
    NoFormat,
    NoData,
    Unsupported(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotRiff => write!(f, "not a RIFF file"),
            Error::NotWave => write!(f, "not a WAVE file"),
            Error::NoFormat => write!(f, "no fmt chunk"),
            Error::NoData => write!(f, "no data chunk"),
            Error::Unsupported(what) => write!(f, "unsupported: {what}"),
        }
    }
}

impl std::error::Error for Error {}

fn u16_at(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

fn u32_at(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

impl Wav {
    pub fn parse(data: &[u8]) -> Result<Wav, Error> {
        if data.len() < 12 || &data[0..4] != b"RIFF" {
            return Err(Error::NotRiff);
        }
        if &data[8..12] != b"WAVE" {
            return Err(Error::NotWave);
        }
        let (mut format, mut body) = (None, None);
        let mut at = 12;
        while at + 8 <= data.len() {
            let id = &data[at..at + 4];
            let size = u32_at(data, at + 4) as usize;
            let from = at + 8;
            let to = (from + size).min(data.len());
            match id {
                b"fmt " => format = Some(&data[from..to]),
                b"data" => body = Some(&data[from..to]),
                _ => {}
            }
            // Chunks are word aligned.
            at = from + size + (size & 1);
        }
        let format = format.ok_or(Error::NoFormat)?;
        let body = body.ok_or(Error::NoData)?;
        if format.len() < 16 {
            return Err(Error::NoFormat);
        }
        if u16_at(format, 0) != 1 {
            return Err(Error::Unsupported("only PCM"));
        }
        let channels = u16_at(format, 2);
        let rate = u32_at(format, 4);
        let bits = u16_at(format, 14);
        let samples = match bits {
            // 8-bit PCM is unsigned with 128 as silence.
            8 => body.iter().map(|&b| ((b as i16) - 128) << 8).collect(),
            16 => body
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect(),
            _ => return Err(Error::Unsupported("only 8- or 16-bit")),
        };
        Ok(Wav { rate, channels, samples })
    }

    pub fn seconds(&self) -> f32 {
        if self.rate == 0 || self.channels == 0 {
            return 0.0;
        }
        self.samples.len() as f32 / self.channels as f32 / self.rate as f32
    }
}
