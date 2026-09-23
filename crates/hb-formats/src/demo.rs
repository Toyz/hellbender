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
    /// Pitch, roll and heading in the engine's 16-bit circle, and whether the
    /// fire button is held (0 or 1, `0x44d3f4`).
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

/// Where a playback has got to: its clock (`0x505130`, 16.16 seconds from
/// the level's start, `0x44cec0`) and the records whose keys are still to be
/// pressed (`0x505138`, `0x50513c`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Player {
    pub clock: i32,
    from: usize,
    to: usize,
}

impl Player {
    /// The frame time added after each frame is played (`0x44d48c`).
    pub fn advance(&mut self, dt: f32) {
        self.clock += from_units(dt);
    }
}

/// What a frame of playback does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub pose: Pose,
    pub fire: bool,
    /// Scan codes pressed this frame.
    pub keys: Vec<i32>,
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

    /// The record the engine plays toward at `time` (16.16 seconds): the
    /// first pose after it (`0x44d1a7`), or `None` once the recording has
    /// run out ("Demo Play Done").
    fn next(&self, time: i32) -> Option<usize> {
        self.records.iter().position(|r| matches!(r, Record::Pose(p) if p.time > time))
    }

    /// The pose at `time` (16.16 seconds) between the recorded ones either
    /// side, as `0x44d180` makes it: each value the previous pose's plus the
    /// difference times the time since over the time between, in integers
    /// (`imul`, `idiv`); the angles the short way round (`shl 16; sar 16`)
    /// and back into the circle. Before the first pose the previous one is
    /// the first record at time zero. The fourth value is the next pose's.
    fn between(&self, time: i32, next: usize) -> Pose {
        let Record::Pose(b) = self.records[next] else { unreachable!() };
        let a = self.records[..next]
            .iter()
            .rev()
            .find_map(|r| match r {
                Record::Pose(p) => Some(*p),
                Record::Key(_) => None,
            })
            .or_else(|| match self.records[0] {
                Record::Pose(p) => Some(Pose { time: 0, ..p }),
                Record::Key(_) => None,
            })
            .unwrap_or(Pose { time: 0, x: 0, y: 0, z: 0, angles: [0; 4] });
        let (span, since) = ((b.time - a.time) as i64, (time - a.time) as i64);
        let step = |from: i32, d: i64| from + (d * since / span.max(1)) as i32;
        let turn = |from: i32, to: i32| step(from, ((to - from) as i16) as i64) & 0xffff;
        Pose {
            time,
            x: step(a.x, (b.x - a.x) as i64),
            y: step(a.y, (b.y - a.y) as i64),
            z: step(a.z, (b.z - a.z) as i64),
            angles: [turn(a.angles[0], b.angles[0]), turn(a.angles[1], b.angles[1]), turn(a.angles[2], b.angles[2]), b.angles[3]],
        }
    }

    /// The pose at a given time in seconds, as the engine plays it back.
    pub fn pose_at(&self, seconds: f32) -> Option<Pose> {
        let time = from_units(seconds);
        Some(self.between(time, self.next(time)?))
    }

    /// One frame of `0x44d180`: the pose at the player's clock, whether the
    /// fire button is held - the next pose's fourth value, which the engine
    /// writes into the key array at `fireKey`'s code (`0x44d400`) - and the
    /// keys pressed this frame. `None` once the recording has run out.
    ///
    /// The key records lag a pose: when the pose being played toward moves
    /// on, the engine presses the key records between the two before it
    /// (`0x44d410`, `0x505138` and `0x50513c`).
    pub fn play(&self, player: &mut Player) -> Option<Frame> {
        let next = self.next(player.clock)?;
        let pose = self.between(player.clock, next);
        let mut keys = Vec::new();
        if player.to != next {
            if player.to > player.from {
                keys.extend(self.records[player.from..player.to].iter().filter_map(|r| match r {
                    Record::Key(k) => Some(k.scan_code),
                    Record::Pose(_) => None,
                }));
            }
            player.from = player.to;
            player.to = next;
        }
        Some(Frame { fire: pose.angles[3] != 0, pose, keys })
    }

    pub fn seconds(&self) -> f32 {
        to_units(self.poses().map(|p| p.time).max().unwrap_or(0))
    }
}
