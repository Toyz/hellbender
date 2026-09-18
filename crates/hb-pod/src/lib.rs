//! The `.POD` archive: Terminal Reality's POD1 container.
//!
//! Layout is in `docs/formats/pod.md`. Nothing is compressed, so an archive is
//! opened by mapping the whole file and handing out subslices of it.

use std::fmt;
use std::path::{Path, PathBuf};

const DIR_OFFSET: usize = 0x54;
const ENTRY_SIZE: usize = 40;
const NAME_LEN: usize = 32;
const COMMENT_LEN: usize = 80;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    /// The file is shorter than the header, or than its own directory claims.
    Truncated { need: usize, have: usize },
    /// An entry's body runs past the end of the file.
    EntryOutOfBounds { name: String, offset: u32, size: u32 },
    NotFound(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Truncated { need, have } => {
                write!(f, "truncated archive: need {need} bytes, have {have}")
            }
            Error::EntryOutOfBounds { name, offset, size } => {
                write!(f, "entry {name} at {offset}+{size} runs past end of file")
            }
            Error::NotFound(name) => write!(f, "no entry named {name}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// One directory entry.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The path as stored, with backslashes: `ART\AFTR0000.RAW`.
    pub name: String,
    /// The second string in the 32-byte name field. For `.RAW` entries this is
    /// the palette the texture was authored against; otherwise it is empty.
    /// The engine never reads it.
    pub palette: String,
    pub size: u32,
    pub offset: u32,
}

impl Entry {
    /// The path with forward slashes, for use on a host filesystem.
    pub fn path(&self) -> String {
        self.name.replace('\\', "/")
    }

    /// The directory component, lower-cased: `art`, `data`, `models`.
    pub fn dir(&self) -> String {
        match self.name.split_once('\\') {
            Some((dir, _)) => dir.to_ascii_lowercase(),
            None => String::new(),
        }
    }

    /// The basename, lower-cased.
    pub fn file_name(&self) -> String {
        match self.name.rsplit_once('\\') {
            Some((_, name)) => name.to_ascii_lowercase(),
            None => self.name.to_ascii_lowercase(),
        }
    }

    /// The extension without the dot, lower-cased. Empty if there is none.
    pub fn ext(&self) -> String {
        match self.name.rsplit_once('.') {
            Some((_, ext)) => ext.to_ascii_lowercase(),
            None => String::new(),
        }
    }
}

pub struct Pod {
    path: PathBuf,
    data: Vec<u8>,
    comment: String,
    entries: Vec<Entry>,
}

impl Pod {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();
        let data = std::fs::read(&path)?;
        Self::parse(path, data)
    }

    pub fn parse(path: PathBuf, data: Vec<u8>) -> Result<Self, Error> {
        if data.len() < DIR_OFFSET {
            return Err(Error::Truncated { need: DIR_OFFSET, have: data.len() });
        }
        let count = u32_at(&data, 0) as usize;
        let dir_end = DIR_OFFSET + count * ENTRY_SIZE;
        if data.len() < dir_end {
            return Err(Error::Truncated { need: dir_end, have: data.len() });
        }
        let comment = cstr(&data[4..4 + COMMENT_LEN]);

        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let base = DIR_OFFSET + i * ENTRY_SIZE;
            let raw = &data[base..base + NAME_LEN];
            // The 32 bytes hold the path, a NUL, and then - for art entries -
            // a second NUL-terminated string naming a palette.
            let name = cstr(raw);
            let palette = raw
                .split(|&b| b == 0)
                .skip(1)
                .find(|part| !part.is_empty())
                .map(|part| cp437(part))
                .unwrap_or_default();
            let size = u32_at(&data, base + 32);
            let offset = u32_at(&data, base + 36);
            let end = offset as usize + size as usize;
            if end > data.len() {
                return Err(Error::EntryOutOfBounds { name, offset, size });
            }
            entries.push(Entry { name, palette, size, offset });
        }
        Ok(Pod { path, data, comment, entries })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The build label the packer wrote, such as `Startup Hellbender 1.0`.
    pub fn comment(&self) -> &str {
        &self.comment
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn bytes(&self, entry: &Entry) -> &[u8] {
        let start = entry.offset as usize;
        &self.data[start..start + entry.size as usize]
    }

    /// Look an entry up the way the engine does: by directory and name, case
    /// insensitively. `find("data", "float.ra0")`.
    pub fn find(&self, dir: &str, name: &str) -> Option<&Entry> {
        let dir = dir.to_ascii_lowercase();
        let name = name.to_ascii_lowercase();
        self.entries
            .iter()
            .find(|e| e.dir() == dir && e.file_name() == name)
    }

    pub fn read(&self, dir: &str, name: &str) -> Result<&[u8], Error> {
        let entry = self
            .find(dir, name)
            .ok_or_else(|| Error::NotFound(format!("{dir}\\{name}")))?;
        Ok(self.bytes(entry))
    }

    /// Every byte after the directory is covered by exactly one entry, with no
    /// gaps and no padding. True of both shipped archives; a useful check that
    /// a reader has the layout right.
    pub fn is_gapless(&self) -> bool {
        let dir_end = DIR_OFFSET + self.entries.len() * ENTRY_SIZE;
        let lowest = self.entries.iter().map(|e| e.offset as usize).min();
        let total: usize = self.entries.iter().map(|e| e.size as usize).sum();
        lowest == Some(dir_end) && dir_end + total == self.data.len()
    }
}

fn u32_at(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

fn cstr(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    cp437(&raw[..end])
}

/// Names are ASCII in practice; decode as cp437 so a stray high byte cannot
/// fail the whole archive.
fn cp437(raw: &[u8]) -> String {
    raw.iter().map(|&b| CP437[b as usize]).collect()
}

const CP437: [char; 256] = {
    let mut table = ['\u{0}'; 256];
    let mut i = 0;
    while i < 128 {
        table[i] = i as u8 as char;
        i += 1;
    }
    // The high half is only reachable through corrupt data; map it to the
    // Latin-1 code points so the string stays printable and round-trips.
    let mut i = 128;
    while i < 256 {
        table[i] = i as u8 as char;
        i += 1;
    }
    table
};
