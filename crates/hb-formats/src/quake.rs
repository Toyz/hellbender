//! `.QKE`: the parts of the world that move.
//!
//! "Quake" is the engine's own word - the diagnostics are
//! `processBoxQuake: no match for watchBox found` - and the file holds two
//! lists: ground quakes, which move a rectangle of the ground's heightfield,
//! and box quakes, which move one cell's box. A box quake with an up sound and
//! a down sound is a door or a lift.
//!
//! The reader is `0x40f970` and the writer `0x40f260`; between them they give
//! the record in full. A ground entry is 144 bytes at `0x75b0e0` and a box
//! entry 184 at `0x763d90`, and the file writes each field in the order the
//! struct holds it.

use crate::fixed::to_units;
use crate::text::{int, ints, lines, optional_name};
use crate::{Error, Result};

/// One entry of either list: the numbers, then five sound slots.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entry {
    /// First line. 0 for an entry that does nothing - `HOTH` keeps fifteen
    /// ground entries and only some are live.
    pub kind: i64,
    /// Two heights in the terrain's own units (a signed byte height shifted
    /// up eight, as the chambers are): where the piece moves between.
    pub heights: [i64; 2],
    /// A ground quake's rectangle of cells - two corners and a mode - or a
    /// box quake's single cell and its box set.
    pub where_: Vec<i64>,
    /// What this entry watches, and how long it waits once triggered. See
    /// [`Entry::watches`] for the first three and [`Entry::delay`] for the
    /// fourth.
    pub watch: [i64; 4],
    /// Which altitude moves, then the seconds out, the pause at the far end,
    /// the seconds back and the pause at rest. The two durations are 16.16
    /// seconds: the engine divides the distance left by them, so a piece
    /// takes as long to move whatever its height (`0x410e40`).
    pub motion: [i64; 5],
    /// Four for a ground quake, five for a box quake. The loader turns the
    /// first into a mode byte, the next three into single bits, and the
    /// fifth into a four-bit field that says what the entry watches
    /// (`0x410034`).
    pub flags: Vec<i64>,
    /// Up to five sounds. A door names two, going up and going down.
    pub sounds: Vec<Option<String>>,
    /// The number after `!--Additional quake info--`: the entry's id, which
    /// is what a watching entry names. See [`Entry::watches`].
    pub extra: i64,
    /// Box entries only: the number and two names after
    /// `@--Box quake switch info--`.
    pub switch: Option<(i64, Vec<Option<String>>)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Quake {
    pub ground: Vec<Entry>,
    pub boxes: Vec<Entry>,
}

struct Reader<'a> {
    lines: &'a [String],
    at: usize,
}

impl Reader<'_> {
    fn banner(&mut self) {
        self.at += 1;
    }

    fn int(&mut self, what: &'static str) -> Result<i64> {
        let v = int(self.lines, self.at, what)?;
        self.at += 1;
        Ok(v)
    }

    fn ints(&mut self, n: usize, what: &'static str) -> Result<Vec<i64>> {
        let v = ints(self.lines, self.at, what)?;
        if v.len() != n {
            return Err(Error::BadLine {
                what,
                line: self.at + 1,
                saw: self.lines[self.at].clone(),
            });
        }
        self.at += 1;
        Ok(v)
    }

    fn name(&mut self) -> Option<String> {
        let line = self.lines.get(self.at).map(String::as_str).unwrap_or("");
        self.at += 1;
        optional_name(line).map(str::to_string)
    }

    /// True when the line at the cursor starts with `mark`.
    fn at_mark(&self, mark: char) -> bool {
        self.lines.get(self.at).is_some_and(|l| l.starts_with(mark))
    }
}

fn entry(r: &mut Reader, box_quake: bool) -> Result<Entry> {
    r.banner();
    let kind = r.int("QKE kind")?;
    let heights = r.ints(2, "QKE heights")?;
    let where_ = r.ints(if box_quake { 3 } else { 5 }, "QKE cells")?;
    let watch = r.ints(4, "QKE watch")?;
    let motion = r.ints(5, "QKE motion")?;
    let flags = r.ints(if box_quake { 5 } else { 4 }, "QKE flags")?;
    let sounds = (0..5).map(|_| r.name()).collect();
    // `!--Additional quake info--`, then one number.
    r.banner();
    let extra = r.int("QKE extra")?;
    let switch = if box_quake && r.at_mark('@') {
        r.banner();
        let n = r.int("QKE switch")?;
        Some((n, (0..2).map(|_| r.name()).collect()))
    } else {
        None
    };
    Ok(Entry {
        kind,
        heights: [heights[0], heights[1]],
        where_,
        watch: [watch[0], watch[1], watch[2], watch[3]],
        motion: [motion[0], motion[1], motion[2], motion[3], motion[4]],
        flags,
        sounds,
        extra,
        switch,
    })
}

/// A count, that many ground entries, a count, that many box entries.
pub fn parse(data: &[u8]) -> Result<Quake> {
    let lines = lines(data);
    let mut r = Reader { lines: &lines, at: 0 };
    r.banner();
    let ground_count = r.int("QKE ground count")?.max(0) as usize;
    let ground = (0..ground_count).map(|_| entry(&mut r, false)).collect::<Result<Vec<_>>>()?;
    r.banner();
    let box_count = r.int("QKE box count")?.max(0) as usize;
    let boxes = (0..box_count).map(|_| entry(&mut r, true)).collect::<Result<Vec<_>>>()?;
    Ok(Quake { ground, boxes })
}

/// What a box quake waits for before it moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watches {
    /// The entry whose [`Entry::extra`] is this id.
    Link(i64),
    /// Whatever moves this cell - row, column, and which box set.
    Cell { row: i64, column: i64, set: i64 },
}

impl Entry {
    /// Whether the entry does anything.
    pub fn live(&self) -> bool {
        self.kind != 0
    }

    /// A box quake whose switch block says 1 is a switch: it looks for the
    /// entry watching it and throws that one (`0x410d00`).
    pub fn is_switch(&self) -> bool {
        matches!(self.switch, Some((1, _)))
    }

    /// What this entry is waiting for, from the flags line's fifth number:
    /// 3 watches a cell, 4 watches an id (`0x410d33`, `0x410d66`).
    pub fn watches(&self) -> Option<Watches> {
        match self.flags.get(4)? {
            3 => Some(Watches::Cell {
                row: self.watch[0],
                column: self.watch[1],
                set: self.watch[2],
            }),
            4 if self.watch[0] >= 0 => Some(Watches::Link(self.watch[0])),
            _ => None,
        }
    }

    /// The same for a ground entry, whose flags line has four numbers
    /// rather than five, so the watch kind is the last of them
    /// (`0x410dcd`).
    pub fn watches_ground(&self) -> Option<Watches> {
        match self.flags.get(3)? {
            3 if self.where_.len() >= 4 => Some(Watches::Cell {
                row: self.watch[0],
                column: self.watch[1],
                set: self.watch[2],
            }),
            4 if self.watch[0] >= 0 => Some(Watches::Link(self.watch[0])),
            _ => None,
        }
    }

    /// Seconds it waits after being thrown, before it starts to move
    /// (`0x411337`). 0 in every shipped entry.
    pub fn delay(&self) -> f32 {
        to_units(self.watch[3] as i32)
    }

    /// Seconds the move out and the move back take, and the pause at each
    /// end: (out, hold, back, rest).
    pub fn timing(&self) -> [f32; 4] {
        let s = |v: i64| to_units(v as i32);
        [s(self.motion[1]), s(self.motion[2]), s(self.motion[3]), s(self.motion[4])]
    }

    /// The two heights as world units: the terrain stores a height as a
    /// value shifted up eight, so this is the same scale the chambers use.
    pub fn span(&self) -> [f32; 2] {
        self.heights.map(|h| to_units((h << 8) as i32))
    }

    /// The sounds it names, in order, without the empty slots.
    pub fn named_sounds(&self) -> Vec<&str> {
        self.sounds.iter().flatten().map(String::as_str).collect()
    }
}
