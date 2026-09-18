//! The `.MOD` music: 6-channel ProTracker.
//!
//! All 15 modules in the archives carry the `6CHN` tag at offset 1080 and the
//! standard 31-sample ProTracker layout. The composer left the sample path in
//! the title field - `(C) Terminal Reality\samples\kik...`.

/// Where the four-byte channel tag lives.
pub const TAG_AT: usize = 1080;
/// The header is always this long, whatever the channel count.
pub const HEADER: usize = 1084;
pub const ROWS: usize = 64;

#[derive(Debug, Clone, Default)]
pub struct Sample {
    pub name: String,
    /// In samples, not words.
    pub length: usize,
    /// -8 to 7, as a signed nibble.
    pub finetune: i8,
    /// 0 to 64.
    pub volume: u8,
    pub repeat_from: usize,
    pub repeat_len: usize,
    /// Signed 8-bit PCM.
    pub data: Vec<i8>,
}

impl Sample {
    pub fn loops(&self) -> bool {
        self.repeat_len > 2
    }
}

/// One cell of a pattern: what a channel does on a row.
#[derive(Debug, Clone, Copy, Default)]
pub struct Note {
    /// Amiga period, or 0 for no new note.
    pub period: u16,
    /// 1-based, or 0 for "keep the current one".
    pub sample: u8,
    pub effect: u8,
    pub argument: u8,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub title: String,
    pub channels: usize,
    pub samples: Vec<Sample>,
    /// Pattern index per position.
    pub order: Vec<u8>,
    pub restart: u8,
    /// `patterns[p][row][channel]`.
    pub patterns: Vec<Vec<Vec<Note>>>,
}

#[derive(Debug)]
pub enum Error {
    TooShort,
    UnknownTag([u8; 4]),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::TooShort => write!(f, "shorter than a ProTracker header"),
            Error::UnknownTag(tag) => {
                write!(f, "unknown channel tag {:?}", String::from_utf8_lossy(tag))
            }
        }
    }
}

impl std::error::Error for Error {}

fn text(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).trim_end().to_string()
}

/// Channels for a tag. `M.K.` and `M!K!` are the four-channel originals;
/// `nCHN` and `nnCH` carry the count in the tag.
pub fn channels_for(tag: &[u8; 4]) -> Option<usize> {
    match tag {
        b"M.K." | b"M!K!" | b"FLT4" => Some(4),
        [n, b'C', b'H', b'N'] if n.is_ascii_digit() => Some((n - b'0') as usize),
        [a, b, b'C', b'H'] if a.is_ascii_digit() && b.is_ascii_digit() => {
            Some(((a - b'0') * 10 + (b - b'0')) as usize)
        }
        _ => None,
    }
}

impl Module {
    pub fn parse(data: &[u8]) -> Result<Module, Error> {
        if data.len() < HEADER {
            return Err(Error::TooShort);
        }
        let mut tag = [0u8; 4];
        tag.copy_from_slice(&data[TAG_AT..TAG_AT + 4]);
        let channels = channels_for(&tag).ok_or(Error::UnknownTag(tag))?;

        let mut samples = Vec::with_capacity(31);
        for i in 0..31 {
            let at = 20 + i * 30;
            let word = |o: usize| u16::from_be_bytes([data[at + o], data[at + o + 1]]) as usize;
            let finetune = (data[at + 24] & 0x0f) as i8;
            samples.push(Sample {
                name: text(&data[at..at + 22]),
                length: word(22) * 2,
                // The nibble is signed: 8 to 15 mean -8 to -1.
                finetune: if finetune > 7 { finetune - 16 } else { finetune },
                volume: data[at + 25].min(64),
                repeat_from: word(26) * 2,
                repeat_len: word(28) * 2,
                data: Vec::new(),
            });
        }

        let length = data[950] as usize;
        let restart = data[951];
        let order: Vec<u8> = data[952..1080].to_vec();
        let count = order[..length.min(128)].iter().copied().max().unwrap_or(0) as usize + 1;

        let mut at = HEADER;
        let mut patterns = Vec::with_capacity(count);
        for _ in 0..count {
            let mut rows = Vec::with_capacity(ROWS);
            for _ in 0..ROWS {
                let mut row = Vec::with_capacity(channels);
                for _ in 0..channels {
                    if at + 4 > data.len() {
                        row.push(Note::default());
                        continue;
                    }
                    let b = &data[at..at + 4];
                    at += 4;
                    row.push(Note {
                        period: (((b[0] & 0x0f) as u16) << 8) | b[1] as u16,
                        sample: (b[0] & 0xf0) | (b[2] >> 4),
                        effect: b[2] & 0x0f,
                        argument: b[3],
                    });
                }
                rows.push(row);
            }
            patterns.push(rows);
        }

        for sample in &mut samples {
            let end = (at + sample.length).min(data.len());
            sample.data = data[at.min(data.len())..end].iter().map(|&b| b as i8).collect();
            at += sample.length;
            // A repeat past the end is a broken module, not a reason to fail.
            if sample.repeat_from + sample.repeat_len > sample.data.len() {
                sample.repeat_len = 0;
            }
        }

        Ok(Module {
            title: text(&data[0..20]),
            channels,
            samples,
            order: order[..length.min(128)].to_vec(),
            restart,
            patterns,
        })
    }

    pub fn used_samples(&self) -> usize {
        self.samples.iter().filter(|s| s.length > 2).count()
    }
}
