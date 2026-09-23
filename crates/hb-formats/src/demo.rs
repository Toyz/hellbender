//! `DEMO\*.DMO`: a recorded flight, for the attract mode.
//!
//! CRLF text. A record count, the level, then that many tagged records. A
//! record's first line is `tag,time`; tag 0 is a pose and is followed by a
//! position line and an angle line, tag 1 is a key press and is followed by
//! one line holding the key's scan code.
//!
//! All three shipped demos parse consuming every line, with exactly as many
//! records as their first line declares and the time never decreasing.

use crate::fixed::{from_units, to_units};
use crate::text::lines;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pose {
    /// 16.16 seconds since the demo started. `DEMO1.DMO` runs 50.8 of them,
    /// which is a plausible length for an attract loop.
    pub time: i32,
    /// 16.16 world position, in the world's signed centred coordinates.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Three angles in the engine's 16-bit circle, and a fourth value that is
    /// only ever 0 or 1.
    pub angles: [i32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPress {
    pub time: i32,
    /// A PC scan code. 57 is the space bar, which `HELLBEND.INI` binds as
    /// `fireKey`; 2 is the `1` key, `keyDispersionCannon`.
    pub scan_code: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Record {
    Pose(Pose),
    Key(KeyPress),
}

#[derive(Debug, Clone)]
pub struct Demo {
    /// The `.LVL` it was recorded on. Two of the three shipped demos name a
    /// level that is not in GAME.POD.
    pub level: String,
    pub records: Vec<Record>,
}

impl Demo {
    pub fn parse(data: &[u8]) -> Result<Demo> {
        let l = lines(data);
        let count: usize = l
            .first()
            .and_then(|s| s.trim().parse().ok())
            .ok_or(Error::BadLine { what: "DMO count", line: 1, saw: l.first().cloned().unwrap_or_default() })?;
        let level = l.get(1).cloned().unwrap_or_default();
        let ints = |at: usize, want: usize| -> Result<Vec<i32>> {
            let line = l.get(at).ok_or(Error::BadLine {
                what: "DMO record",
                line: at + 1,
                saw: String::new(),
            })?;
            let v: Vec<i32> = line
                .split(',')
                .map(|p| p.trim().parse::<i32>())
                .collect::<std::result::Result<_, _>>()
                .map_err(|_| Error::BadLine { what: "DMO record", line: at + 1, saw: line.clone() })?;
            if v.len() != want {
                return Err(Error::BadLine { what: "DMO record", line: at + 1, saw: line.clone() });
            }
            Ok(v)
        };

        let mut records = Vec::with_capacity(count);
        let mut at = 2;
        for _ in 0..count {
            let head = ints(at, 2)?;
            match head[0] {
                0 => {
                    let p = ints(at + 1, 3)?;
                    let a = ints(at + 2, 4)?;
                    records.push(Record::Pose(Pose {
                        time: head[1],
                        x: p[0],
                        y: p[1],
                        z: p[2],
                        angles: [a[0], a[1], a[2], a[3]],
                    }));
                    at += 3;
                }
                1 => {
                    let k = ints(at + 1, 1)?;
                    records.push(Record::Key(KeyPress { time: head[1], scan_code: k[0] }));
                    at += 2;
                }
                other => {
                    return Err(Error::BadTag { what: "DMO record", tag: other as u32, at });
                }
            }
        }
        Ok(Demo { level, records })
    }

    pub fn poses(&self) -> impl Iterator<Item = &Pose> {
        self.records.iter().filter_map(|r| match r {
            Record::Pose(p) => Some(p),
            Record::Key(_) => None,
        })
    }

    pub fn keys(&self) -> impl Iterator<Item = &KeyPress> {
        self.records.iter().filter_map(|r| match r {
            Record::Key(k) => Some(k),
            Record::Pose(_) => None,
        })
    }

    /// The pose at a given time, interpolated linearly in position between the
    /// two recorded poses either side of it.
    pub fn pose_at(&self, seconds: f32) -> Option<Pose> {
        let time = from_units(seconds);
        let poses: Vec<&Pose> = self.poses().collect();
        let after = poses.iter().position(|p| p.time >= time)?;
        if after == 0 {
            return Some(*poses[0]);
        }
        let (a, b) = (poses[after - 1], poses[after]);
        let span = (b.time - a.time).max(1) as f32;
        let t = (time - a.time) as f32 / span;
        let lerp = |p: i32, q: i32| p + ((q - p) as f32 * t) as i32;
        Some(Pose {
            time,
            x: lerp(a.x, b.x),
            y: lerp(a.y, b.y),
            z: lerp(a.z, b.z),
            angles: a.angles,
        })
    }

    pub fn seconds(&self) -> f32 {
        to_units(self.poses().map(|p| p.time).max().unwrap_or(0))
    }
}
