//! `.NAV`: a level's mission, as the list of navigation points the player is
//! sent to in turn. See `docs/formats/level-text.md`.
//!
//! The reader is `0x470bd0`. It is `fscanf` driven, and a block that starts
//! with `!`, `@` or `;` is optional: the loader peeks at the first character
//! of the next line and fills in a default when the block is absent. The
//! layout of a record in memory (256 bytes at `0x625140`) is given against
//! each field.

use crate::text::{int, ints, lines, optional_name};
use crate::{Error, Result};

/// What a navigation point asks of the player. The number is the record's
/// first line, `+0xc` in memory; the names are the HUD's own where it has
/// one (`0x472879`), and the text files' otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// 0: destroy every placement in the list. "Destroy Target".
    Destroy,
    /// 1: fly underground within 40 units of it. "Enter Tunnel".
    EnterTunnel,
    /// 2: fly within 40 units of it. "Fly to Checkpoint".
    Checkpoint,
    /// 3: fly into it to leave the level. "Fly to Jump Zone".
    JumpZone,
    /// 4: fly above ground within 40 units of it. "Exit Tunnel".
    ExitTunnel,
    /// 5: a boss, with its own music and, optionally, placements that keep it
    /// whole while they stand.
    Guardian,
    /// 6: where the player starts, and facing which way.
    Start,
    /// 7: reached once every required point before it is done.
    Sync,
    /// 8: drop a rescue beacon within 8 units of it.
    DropBeacon,
    /// 9: the end of the list. The loader adds one when a file has none.
    End,
    /// 10: fly within 30 units of it to be moved somewhere else.
    Warp,
    /// 11: a beacon the player dropped. Never in a file; the engine appends
    /// them while playing.
    Beacon,
    /// 12: see a placement to where it is going. "Escort Ship".
    Escort,
    /// 13: collect a pod. "Pick up Message Pod".
    MessagePod,
    /// 14: destroy one placement. "Destroy Target".
    Kill,
}

impl Kind {
    pub const ALL: [Kind; 15] = [
        Kind::Destroy,
        Kind::EnterTunnel,
        Kind::Checkpoint,
        Kind::JumpZone,
        Kind::ExitTunnel,
        Kind::Guardian,
        Kind::Start,
        Kind::Sync,
        Kind::DropBeacon,
        Kind::End,
        Kind::Warp,
        Kind::Beacon,
        Kind::Escort,
        Kind::MessagePod,
        Kind::Kill,
    ];

    /// The number in the file. The loader stops with "Bad nav type!" on
    /// anything above 14; 15 exists only in network play and is never read.
    pub fn from_code(code: i64) -> Option<Kind> {
        usize::try_from(code).ok().and_then(|i| Kind::ALL.get(i).copied())
    }

    pub fn code(self) -> i64 {
        Kind::ALL.iter().position(|&k| k == self).unwrap_or(0) as i64
    }

    /// The three letters the HUD's objective box shows for this kind - the
    /// jump table at `0x44ead0` picks one of twelve strings at `0x505478`,
    /// and four of the sixteen entries fall through, which leaves whatever
    /// the box already said. 15, which only network play uses, is `PLY`.
    pub fn abbreviation(self) -> Option<&'static str> {
        Some(match self {
            Kind::Destroy | Kind::Kill => "TGT",
            Kind::EnterTunnel => "TUN",
            Kind::Checkpoint => "CHK",
            Kind::JumpZone => "JMP",
            Kind::ExitTunnel => "EXT",
            Kind::Guardian => "GRD",
            Kind::Start => "STR",
            Kind::DropBeacon => "RES",
            Kind::Beacon => "BCN",
            Kind::Escort => "EST",
            Kind::MessagePod => "MSG",
            Kind::Sync | Kind::End | Kind::Warp => return None,
        })
    }
}

/// What follows the display text, which depends on the kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Data {
    None,
    /// [`Kind::Destroy`]: a count, then that many placement indices, one per
    /// line (`+0xbc`, `+0xc0`...).
    Targets(Vec<usize>),
    /// [`Kind::Guardian`]: the boss's placement (`+0xbc`), the music module
    /// that plays while it is the current point (`+0xc0`), and, after a line
    /// the loader skips (`;place4` in every file) and a `!NewH` header, the
    /// placements that shield it (`+0xd0`, `+0xd4`...).
    Guardian { actor: usize, music: String, shields: Vec<usize> },
    /// [`Kind::Start`]: pitch, roll and heading, in the 16-bit circle.
    Start { angles: [i32; 3] },
    /// [`Kind::Warp`]: where to (`+0xbc`), and a name the engine reads and
    /// does not use.
    Warp { to: [i32; 3], name: String },
    /// [`Kind::Escort`] and [`Kind::Kill`]: a placement index.
    Actor(usize),
    /// [`Kind::MessagePod`]: an index into the level's pods.
    Pod(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nav {
    pub kind: Kind,
    /// 16.16 world coordinates (`+0x0`). Kinds that follow a placement
    /// ignore it; the escort and pod records hold nonsense here.
    pub position: [i32; 3],
    /// The first number of the `!priority,time` block (`+0x60`). 0 is a
    /// required point and 1 an optional one; without the block it is 1.
    pub priority: i32,
    /// The second, a time limit in 16.16 seconds (`+0x64`), 0 for none.
    pub time: i32,
    /// Played when the point is done (`+0x6c`). The line after it is the
    /// "completion text", which the loader reads and throws away.
    pub completion_sound: Option<String>,
    /// Played once, the first time the player comes within 80 units (`+0xa8`).
    pub proximity_sound: Option<String>,
    /// The line shown for the objective (`+0x10`), at most 78 characters.
    pub text: String,
    pub data: Data,
}

impl Nav {
    pub fn required(&self) -> bool {
        self.priority == 0
    }
}

/// The count on the first line, then that many records, each ending with a
/// line of dashes.
pub fn navs(data: &[u8]) -> Result<Vec<Nav>> {
    let lines = lines(data);
    let count = int(&lines, 0, "NAV count")?;
    let mut at = 1;
    let mut out = Vec::with_capacity(count.max(0) as usize);
    let starts = |at: usize, c: char| lines.get(at).is_some_and(|l| l.starts_with(c));
    // `%s` reads up to the first blank.
    let token = |at: usize| -> Result<String> {
        lines
            .get(at)
            .and_then(|l| l.split_whitespace().next())
            .map(str::to_string)
            .ok_or_else(|| Error::BadLine {
                what: "NAV name",
                line: at + 1,
                saw: lines.get(at).cloned().unwrap_or_default(),
            })
    };
    let index = |at: usize, what| -> Result<usize> {
        let v = int(&lines, at, what)?;
        usize::try_from(v).map_err(|_| Error::BadLine {
            what,
            line: at + 1,
            saw: lines[at].clone(),
        })
    };
    let triple = |at: usize, what| -> Result<[i32; 3]> {
        match ints(&lines, at, what)?.as_slice() {
            &[x, y, z] => Ok([x as i32, y as i32, z as i32]),
            _ => Err(Error::BadLine { what, line: at + 1, saw: lines[at].clone() }),
        }
    };
    for _ in 0..count {
        let code = int(&lines, at, "NAV type")?;
        let kind = Kind::from_code(code).ok_or_else(|| Error::BadLine {
            what: "NAV type",
            line: at + 1,
            saw: lines[at].clone(),
        })?;
        let position = triple(at + 1, "NAV position")?;
        at += 2;

        let (mut priority, mut time) = (1, 0);
        if starts(at, '!') {
            match ints(&lines, at + 1, "NAV priority,time")?.as_slice() {
                &[p, t] => (priority, time) = (p as i32, t as i32),
                _ => {
                    return Err(Error::BadLine {
                        what: "NAV priority,time",
                        line: at + 2,
                        saw: lines[at + 1].clone(),
                    })
                }
            }
            at += 2;
        }
        let mut completion_sound = None;
        if starts(at, '@') {
            completion_sound = optional_name(&token(at + 1)?).map(str::to_string);
            at += 3;
        }
        let mut proximity_sound = None;
        if starts(at, ';') {
            proximity_sound = optional_name(&token(at + 1)?).map(str::to_string);
            at += 2;
        }

        // `fgets`, `strncpy` of 79, then the last character dropped: the
        // newline, or the 79th character of a longer line.
        let line = lines.get(at).cloned().unwrap_or_default();
        let text = if line.len() >= 79 {
            line.chars().take(78).collect()
        } else {
            line
        };
        at += 1;

        let data = match kind {
            Kind::Destroy => {
                let n = index(at, "NAV target count")?;
                let targets = (0..n)
                    .map(|i| index(at + 1 + i, "NAV target"))
                    .collect::<Result<Vec<_>>>()?;
                at += 1 + n;
                Data::Targets(targets)
            }
            Kind::Guardian => {
                let actor = index(at, "NAV guardian")?;
                let music = token(at + 1)?;
                at += 3;
                let mut shields = Vec::new();
                if starts(at, '!') {
                    let n = index(at + 1, "NAV shield count")?;
                    shields = (0..n)
                        .map(|i| index(at + 2 + i, "NAV shield"))
                        .collect::<Result<Vec<_>>>()?;
                    at += 2 + n;
                }
                Data::Guardian { actor, music, shields }
            }
            Kind::Start => {
                let angles = triple(at, "NAV start angles")?;
                at += 1;
                Data::Start { angles }
            }
            Kind::Warp => {
                let to = triple(at, "NAV warp")?;
                let name = token(at + 1)?;
                at += 2;
                Data::Warp { to, name }
            }
            Kind::Escort | Kind::Kill => {
                let actor = index(at, "NAV actor")?;
                at += 1;
                Data::Actor(actor)
            }
            Kind::MessagePod => {
                let pod = index(at, "NAV pod")?;
                at += 1;
                Data::Pod(pod)
            }
            _ => Data::None,
        };
        if starts(at, '-') {
            at += 1;
        }
        out.push(Nav {
            kind,
            position,
            priority,
            time,
            completion_sound,
            proximity_sound,
            text,
            data,
        });
    }
    Ok(out)
}
