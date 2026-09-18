//! `.CRS`: the paths enemies follow, named by line 15 of a `.DEF` record.
//!
//! Two variants share one container. Both start with a course count and
//! separate courses with a line of asterisks; the line after the separator
//! says which variant this course is.

use crate::text::lines;
use crate::{Error, Result};

/// A world position in 16.16 fixed point. See [`crate::terrain`] for the units:
/// the world is 1024 units square, centred on the origin, so x and z run from
/// about -512 to +512.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone)]
pub enum Course {
    /// `numPoints,groundCourse,periodic` then that many points. 86 of the 90
    /// shipped courses.
    Points {
        /// Non-zero when the course is meant to be followed along the ground.
        ground: i32,
        /// Non-zero when the course loops back to its first point.
        periodic: i32,
        points: Vec<Point>,
    },
    /// `[Course N] ID,numSegs,direction` then that many `Course type` /
    /// `Course start` / `Course end` blocks. Four of the 90, all in `KREASH`
    /// and `KREASH3`.
    Segments {
        id: i32,
        direction: i32,
        segments: Vec<Segment>,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct Segment {
    pub kind: i32,
    pub start: Point,
    pub end: Point,
}

impl Course {
    /// Every point the course visits, whichever variant it is.
    pub fn points(&self) -> Vec<Point> {
        match self {
            Course::Points { points, .. } => points.clone(),
            Course::Segments { segments, .. } => {
                let mut out = Vec::with_capacity(segments.len() + 1);
                for (i, seg) in segments.iter().enumerate() {
                    if i == 0 {
                        out.push(seg.start);
                    }
                    out.push(seg.end);
                }
                out
            }
        }
    }
}

fn triple(line: Option<&String>, at: usize) -> Result<Point> {
    let parts: Vec<i32> = line
        .map(|l| l.split(',').filter_map(|p| p.trim().parse().ok()).collect())
        .unwrap_or_default();
    if parts.len() != 3 {
        return Err(Error::BadLine {
            what: "CRS point",
            line: at + 1,
            saw: line.cloned().unwrap_or_default(),
        });
    }
    Ok(Point { x: parts[0], y: parts[1], z: parts[2] })
}

fn ints(line: Option<&String>, want: usize, at: usize) -> Result<Vec<i32>> {
    let parts: Vec<i32> = line
        .map(|l| l.split(',').filter_map(|p| p.trim().parse().ok()).collect())
        .unwrap_or_default();
    if parts.len() != want {
        return Err(Error::BadLine {
            what: "CRS header",
            line: at + 1,
            saw: line.cloned().unwrap_or_default(),
        });
    }
    Ok(parts)
}

pub fn parse(data: &[u8]) -> Result<Vec<Course>> {
    let l = lines(data);
    if l.is_empty() {
        return Ok(Vec::new());
    }
    let count: usize = l[0].trim().parse().map_err(|_| Error::BadLine {
        what: "CRS count",
        line: 1,
        saw: l[0].clone(),
    })?;
    let mut out = Vec::with_capacity(count);
    let mut i = 1;
    for _ in 0..count {
        if !l.get(i).is_some_and(|s| s.starts_with('*')) {
            return Err(Error::BadLine {
                what: "CRS separator",
                line: i + 1,
                saw: l.get(i).cloned().unwrap_or_default(),
            });
        }
        let head = l.get(i + 1).cloned().unwrap_or_default();
        if head.starts_with("numPoints") {
            let hdr = ints(l.get(i + 2), 3, i + 2)?;
            let n = hdr[0].max(0) as usize;
            let mut points = Vec::with_capacity(n);
            for k in 0..n {
                points.push(triple(l.get(i + 3 + k), i + 3 + k)?);
            }
            out.push(Course::Points { ground: hdr[1], periodic: hdr[2], points });
            i += 3 + n;
        } else if head.starts_with("[Course") {
            let hdr = ints(l.get(i + 2), 3, i + 2)?;
            let n = hdr[1].max(0) as usize;
            i += 3;
            let mut segments = Vec::with_capacity(n);
            for _ in 0..n {
                // Each segment is its own separator, a type, a start and an end,
                // with a label line before each of the last two.
                let kind = ints(l.get(i + 2), 1, i + 2)?[0];
                segments.push(Segment {
                    kind,
                    start: triple(l.get(i + 4), i + 4)?,
                    end: triple(l.get(i + 6), i + 6)?,
                });
                i += 7;
            }
            out.push(Course::Segments { id: hdr[0], direction: hdr[2], segments });
        } else {
            return Err(Error::BadLine { what: "CRS course header", line: i + 2, saw: head });
        }
    }
    Ok(out)
}
