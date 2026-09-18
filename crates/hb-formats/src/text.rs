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

#[derive(Debug, Clone)]
pub struct EnemyDef {
    /// The six integers of the record's first line. Fields 1, 3 and 5 are zero
    /// in every one of the 1,848 shipped records; field 0 is a class of some
    /// kind and field 2 is large and varied.
    pub fields: [i64; 6],
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
        out.push(EnemyDef {
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
