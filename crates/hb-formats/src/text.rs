//! The level text formats: CRLF, positional, with punctuation sentinels.
//!
//! All of Hellbender's text data shares one design - a count on the first
//! line, fixed-length positional records after it, and blocks appended over
//! the project's life headed by a line starting with `;`, `!`, `@`, `=` or
//! `{`. The sentinels are a version history written into every file, and a
//! parser that checks them catches a misaligned read immediately.

use crate::{Error, Result};

/// Split a CRLF (or LF) text file into lines, dropping a trailing empty line.
pub fn lines(data: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(data);
    let mut out: Vec<String> =
        text.replace("\r\n", "\n").split('\n').map(|s| s.to_string()).collect();
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out
}

/// True if the line is one of the format's appended-block markers.
pub fn is_sentinel(line: &str) -> bool {
    matches!(line.chars().next(), Some(';' | '!' | '@' | '=' | '{' | '#' | '%' | ':'))
}

pub fn expect_sentinel(lines: &[String], at: usize, what: &'static str) -> Result<()> {
    match lines.get(at) {
        Some(line) if is_sentinel(line) => Ok(()),
        other => Err(Error::BadLine {
            what,
            line: at + 1,
            saw: other.cloned().unwrap_or_default(),
        }),
    }
}

pub fn int(lines: &[String], at: usize, what: &'static str) -> Result<i64> {
    lines
        .get(at)
        .and_then(|l| l.trim().parse::<i64>().ok())
        .ok_or_else(|| Error::BadLine {
            what,
            line: at + 1,
            saw: lines.get(at).cloned().unwrap_or_default(),
        })
}

/// A comma-separated list of integers, as every numeric line in these files is.
pub fn ints(lines: &[String], at: usize, what: &'static str) -> Result<Vec<i64>> {
    let line = lines.get(at).ok_or(Error::BadLine {
        what,
        line: at + 1,
        saw: String::new(),
    })?;
    line.split(',')
        .map(|p| p.trim().parse::<i64>())
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| Error::BadLine { what, line: at + 1, saw: line.clone() })
    }

/// `null` in a filename slot means the slot is unused.
pub fn optional_name(line: &str) -> Option<&str> {
    let line = line.trim();
    if line.is_empty() || line.eq_ignore_ascii_case("null") {
        None
    } else {
        Some(line)
    }
}

/// One `.ANI` entry: a texture that cycles through a list of frames.
#[derive(Debug, Clone)]
pub struct Animation {
    /// The texture this replaces, by name.
    pub base: String,
    /// 16.16 seconds a frame. 6,553 is 0.1 s and 16,384 is 0.25 s.
    pub delay: i32,
    /// The frames, starting with the base itself.
    pub frames: Vec<String>,
}

/// `.ANI`: a count, then per entry a base texture name, a `frames,delay` pair
/// and that many frame names.
pub fn animations(data: &[u8]) -> Result<Vec<Animation>> {
    let lines = lines(data);
    let count = int(&lines, 0, "ANI count")? as usize;
    let mut out = Vec::with_capacity(count);
    let mut at = 1;
    for _ in 0..count {
        let base = lines.get(at).cloned().ok_or(Error::BadLine {
            what: "ANI base texture",
            line: at + 1,
            saw: String::new(),
        })?;
        let head = ints(&lines, at + 1, "ANI frames,delay")?;
        if head.len() != 2 {
            return Err(Error::BadLine {
                what: "ANI frames,delay",
                line: at + 2,
                saw: lines[at + 1].clone(),
            });
        }
        let frames = head[0].max(0) as usize;
        let end = at + 2 + frames;
        if end > lines.len() {
            return Err(Error::BadLine {
                what: "ANI frame list",
                line: at + 3,
                saw: format!("{frames} frames but the file ends"),
            });
        }
        out.push(Animation {
            base,
            delay: head[1] as i32,
            frames: lines[at + 2..end].to_vec(),
        });
        at = end;
    }
    Ok(out)
}

/// `.TEX`: a count, then that many texture filenames.
pub fn name_list(data: &[u8], what: &'static str) -> Result<Vec<String>> {
    let lines = lines(data);
    let count = int(&lines, 0, what)? as usize;
    if lines.len() < count + 1 {
        return Err(Error::BadLine {
            what,
            line: lines.len(),
            saw: format!("declared {count}, have {}", lines.len().saturating_sub(1)),
        });
    }
    Ok(lines[1..=count].to_vec())
}

/// `.DEF`: a count, then that many 25-line records.
pub const DEF_RECORD_LINES: usize = 25;

/// One `.DEF` type record.
///
/// The loader at `HELLBEND.EXE:0x404f80` reads each record into a 664-byte
/// type struct in the array at `[0x500748]`. The struct offsets are given on
/// each field so a reading of the engine can be matched to the data.
#[derive(Debug, Clone, Default)]
pub struct EnemyDef {
    /// The six integers of the record's first line, into type offsets 0x0c,
    /// 0x1c, 0x08, 0x10, 0x14 and 0x18. Field 0 is the behaviour class (see
    /// [`EnemyDef::class`]) and field 2 the radius (see
    /// [`EnemyDef::radius`]). Fields 1, 3 and 5 are zero in every one of the
    /// 1,848 shipped records.
    pub fields: [i64; 6],
    /// Line 1 field 0, type offset 0x64: the rate the actor eases its position
    /// toward where it wants to be, 16.16 per second. `0x4068f0` multiplies it
    /// by the frame time. Also the default shot speed's half: with no
    /// `#New2ndweapon` line the loader sets the shot speed to twice this.
    pub move_rate: i32,
    /// Line 1 field 1, type offset 0x68: the rate the actor eases its angles
    /// toward the ones it wants, 16.16 per second. A turret swings at this.
    pub turn_rate: i32,
    /// Line 1 field 2, type offset 0x6c: seconds between shots, 16.16.
    pub fire_interval: i32,
    /// Line 1 field 3, type offset 0x70: what one shot takes off the player's
    /// health, whose full value is 1.0, in 16.16.
    pub shot_damage: i32,
    /// Line 1 field 4, type offset 0xdc: the weapon kind a shot carries. 19 is
    /// a guided missile; the rest fly straight.
    pub weapon: i32,
    /// Line 3: the model vertices shots leave from, count at type offset 0x190
    /// and up to eight indices from 0x194. A shot picks one at random.
    pub muzzles: Vec<u16>,
    /// Line 5 (`;NewHit`): hit spheres, count at 0x1b4, vertex indices from
    /// 0x1b8 and 16.16 radii from 0x1d8. Empty means the model's own bounding
    /// box is the target - `0x40ce70` takes one branch or the other.
    pub hit_spheres: Vec<(u16, i32)>,
    /// Line 7 (`!NewAtakRet`), type offsets 0x1f8, 0x1fc, 0x200 and one the
    /// loader reads and drops. The first two are the attack and retreat
    /// ranges, whole units, that the flyers' AI compares distances with
    /// (`0x496f68`, `0x496ce7`); the loader's defaults without the line are 32
    /// and 16.
    pub attack_retreat: [i32; 4],
    /// Line 10 (`#New2ndweapon`), type offsets 0x20c, 0x210, 0x214, 0x218.
    pub second_weapon: SecondWeapon,
    /// Line 12, type offset 0x21c: played at the muzzle on every shot. `null`
    /// in every shipped record.
    pub fire_sound: Option<String>,
    /// The intact model.
    pub model: String,
    /// The wrecked model. `wbunker.bin` pairs with `wbnkruin.bin`; everything
    /// else pairs with `cube.bin`.
    pub wreck: String,
    pub name: String,
    /// Course id from the level's `.CRS`, or `-1` for none.
    pub course: i64,
    /// Damage multipliers in 16.16: cannon, laser, missile.
    pub damage: [i64; 3],
    pub friendly: bool,
    /// The record verbatim, so nothing is lost while fields are still unnamed.
    pub raw: Vec<String>,
}

impl EnemyDef {
    /// Line 0 field 0, type offset 0x0c: which behaviour routine runs the
    /// actor. The switch at `0x40bb93` dispatches on it through a 65-entry
    /// table; class 10 is a turret (`0x408c30`) and class 47 a course follower
    /// (`0x421240`).
    pub fn class(&self) -> i64 {
        self.fields[0]
    }

    /// Line 0 field 2, type offset 0x08: the object's radius in 16.16 world
    /// units. Models are normalised to +/-1.0, so this is also the scale the
    /// model is drawn at - the actor draw at `0x40da00` passes it as the draw
    /// record's size - and the size the visibility test at `0x40c21f` weighs
    /// against distance.
    pub fn radius(&self) -> i32 {
        self.fields[2] as i32
    }

    /// How fast this type's shots fly, in 16.16 units per second.
    pub fn shot_speed(&self) -> i32 {
        self.second_weapon.shot_speed
    }
}

/// `.DEF` line 10, `#New2ndweapon`.
///
/// The loader peeks for the `#`; a record without the line gets
/// `shot_speed = 2 * move_rate` and zeros (`0x405338`). Every shipped record
/// has it.
#[derive(Debug, Clone, Copy, Default)]
pub struct SecondWeapon {
    /// Type offset 0x20c: 0 fires one barrel; 1 cycles three and 2 cycles five,
    /// each offset by 2,048 (11.25 degrees) in pitch or heading from the tables
    /// at `0x500760` and `0x500778`.
    pub barrels: i32,
    pub unknown_1: i32,
    pub unknown_2: i32,
    /// Type offset 0x218: shot speed, 16.16 units per second.
    pub shot_speed: i32,
}

/// One placed object: an index into the level's type table, its hit points,
/// a position and three angles.
///
/// The loader reads these with `fscanf(file, "%d,%d,%d,%d,%d,%d,%d,%d")` at
/// `HELLBEND.EXE:0x4059ab` straight into the actor struct - offsets 0x18,
/// 0x1c, 0x00, 0x04, 0x08, 0x0c, 0x10, 0x14 - and refuses more than 500 per
/// level.
#[derive(Debug, Clone, Copy)]
pub struct Placement {
    /// Index into the record list from [`enemy_defs`]. Always in range across
    /// all 7,606 shipped instances.
    pub kind: usize,
    /// Actor offset 0x1c, 16.16. Every hit subtracts the shot's damage times
    /// the type's multiplier (`0x40d3fc`) and the actor dies at zero; a shot
    /// only tests actors whose value is above zero (`0x40d772`). The commonest
    /// values are 4,096, 8,192 and 40,960 - one, two and ten hits of the
    /// player's 1/16 laser.
    pub hit_points: i32,
    /// 16.16 world position. The world is centred, so x and z run to +/-512.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Actor offsets 0x0c and 0x10, the engine's 16-bit circle. Zero in every
    /// shipped instance.
    pub pitch: i32,
    pub roll: i32,
    /// Actor offset 0x14, a heading in the engine's 16-bit circle.
    pub heading: u16,
}

/// The instance list, which lives in the same `.DEF` file after the type
/// records: a count, then that many eight-integer lines.
pub fn placements(data: &[u8]) -> Result<Vec<Placement>> {
    let lines = lines(data);
    let types = int(&lines, 0, "DEF count")? as usize;
    let at = 1 + types * DEF_RECORD_LINES;
    let count = int(&lines, at, "DEF placement count")? as usize;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let row = at + 1 + i;
        let v = ints(&lines, row, "DEF placement")?;
        if v.len() != 8 {
            return Err(Error::BadLine {
                what: "DEF placement",
                line: row + 1,
                saw: lines.get(row).cloned().unwrap_or_default(),
            });
        }
        out.push(Placement {
            kind: v[0].max(0) as usize,
            hit_points: v[1] as i32,
            x: v[2] as i32,
            y: v[3] as i32,
            z: v[4] as i32,
            pitch: v[5] as i32,
            roll: v[6] as i32,
            heading: v[7] as u16,
        });
    }
    Ok(out)
}

pub fn enemy_defs(data: &[u8]) -> Result<Vec<EnemyDef>> {
    let lines = lines(data);
    let count = int(&lines, 0, "DEF count")? as usize;
    let mut out = Vec::with_capacity(count);
    for r in 0..count {
        let base = 1 + r * DEF_RECORD_LINES;
        if base + DEF_RECORD_LINES > lines.len() {
            return Err(Error::BadLine {
                what: "DEF record",
                line: base + 1,
                saw: format!("declared {count} records, ran out at {r}"),
            });
        }
        let record = &lines[base..base + DEF_RECORD_LINES];
        expect_sentinel(record, 4, "DEF ;NewHit")?;
        expect_sentinel(record, 6, "DEF !NewAtakRet")?;
        expect_sentinel(record, 9, "DEF #New2ndweapon")?;
        expect_sentinel(record, 11, "DEF %SFX")?;
        expect_sentinel(record, 16, "DEF = damage")?;
        expect_sentinel(record, 20, "DEF @ friendly")?;
        expect_sentinel(record, 22, "DEF { sounds")?;

        let head: Vec<&str> = record[0].split(',').collect();
        if head.len() != 8 {
            return Err(Error::BadLine {
                what: "DEF first line",
                line: base + 1,
                saw: record[0].clone(),
            });
        }
        let mut fields = [0i64; 6];
        for (i, slot) in fields.iter_mut().enumerate() {
            *slot = head[i].trim().parse().unwrap_or(0);
        }
        let damage = [
            record[17].trim().parse().unwrap_or(0),
            record[18].trim().parse().unwrap_or(0),
            record[19].trim().parse().unwrap_or(0),
        ];
        let fixed = |at: usize, n: usize, what: &'static str| -> Result<Vec<i32>> {
            let v = ints(record, at, what)?;
            if v.len() < n {
                return Err(Error::BadLine { what, line: base + at + 1, saw: record[at].clone() });
            }
            Ok(v.into_iter().map(|x| x as i32).collect())
        };
        let conduct = fixed(1, 5, "DEF line 1")?;
        // Count then indices; the engine reads nine integers and uses as many
        // indices as the count says.
        let muzzle_line = fixed(3, 9, "DEF muzzles")?;
        let muzzles = muzzle_line[1..=muzzle_line[0].clamp(0, 8) as usize]
            .iter()
            .map(|&v| v as u16)
            .collect();
        let hit_line = fixed(5, 17, "DEF ;NewHit")?;
        let hit_spheres = (0..hit_line[0].clamp(0, 8) as usize)
            .map(|i| (hit_line[1 + i] as u16, hit_line[9 + i]))
            .collect();
        let second = fixed(10, 4, "DEF #New2ndweapon")?;
        let attack = fixed(7, 4, "DEF !NewAtakRet")?;
        let sound = |at: usize| {
            let name = record[at].trim();
            (!name.is_empty() && !name.eq_ignore_ascii_case("null")).then(|| name.to_string())
        };
        out.push(EnemyDef {
            move_rate: conduct[0],
            turn_rate: conduct[1],
            fire_interval: conduct[2],
            shot_damage: conduct[3],
            weapon: conduct[4],
            muzzles,
            hit_spheres,
            attack_retreat: [attack[0], attack[1], attack[2], attack[3]],
            second_weapon: SecondWeapon {
                barrels: second[0],
                unknown_1: second[1],
                unknown_2: second[2],
                shot_speed: second[3],
            },
            fire_sound: sound(12),
            fields,
            model: head[6].trim().to_string(),
            wreck: head[7].trim().to_string(),
            name: record[8].clone(),
            course: record[15].trim().parse().unwrap_or(-1),
            damage,
            friendly: record[21].trim() != "0",
            raw: record.to_vec(),
        })
    }
    Ok(out)
}
