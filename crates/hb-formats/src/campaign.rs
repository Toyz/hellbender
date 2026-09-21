//! The campaign: which level follows which.
//!
//! `GAME.POD` holds the levels in alphabetical order and nothing in a `.LVL`
//! says what comes next, so the order is the engine's. `0x482720` builds it
//! when a new game starts: twenty three entries of forty bytes at `0x6106c0`,
//! each a level's file name copied in, with two parallel arrays of numbers -
//! the chapter at `0x60ff60` and the mission within it at `0x60fc30`.
//!
//! The first entry is `morbos.lvl`. The three `NETLVL` levels are not in it:
//! they are for network play and `netlvl.ini` lists them separately.

/// One entry: the level's stem, which chapter it belongs to and which
/// mission of that chapter it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mission {
    pub stem: &'static str,
    pub chapter: u8,
    pub mission: u8,
}

const fn m(stem: &'static str, chapter: u8, mission: u8) -> Mission {
    Mission { stem, chapter, mission }
}

/// The campaign in order (`0x482720`). The chapter and mission numbers are
/// the two arrays the same routine fills - 1, 1; 1, 2; 1, 3; 2, 1; 2, 2 for
/// the first five, which is the grouping the names already suggest.
pub const CAMPAIGN: [Mission; 23] = [
    m("morbos", 1, 1),
    m("morbos2", 1, 2),
    m("morbos3", 1, 3),
    m("float", 2, 1),
    m("float2", 2, 2),
    m("iowah", 3, 1),
    m("iowah2", 3, 2),
    m("iowah3", 3, 3),
    m("kreash", 4, 1),
    m("kreash2", 4, 2),
    m("kreash3", 4, 3),
    m("jurasic", 5, 1),
    m("jurasic2", 5, 2),
    m("jurasic3", 5, 3),
    m("roid", 6, 1),
    m("roid2", 6, 2),
    m("roid3", 6, 3),
    m("roid4", 6, 4),
    m("hoth", 7, 1),
    m("hoth2", 7, 2),
    m("hoth3", 7, 3),
    m("ship", 8, 1),
    m("ship2", 8, 2),
];

/// The network levels, which are not part of the campaign (`0x504da4`).
pub const NETWORK: [&str; 3] = ["netlvl1", "netlvl2", "netlvl3"];

/// Where the campaign starts.
pub const FIRST: &str = CAMPAIGN[0].stem;

/// Where a level sits in the campaign, if it is in it at all.
pub fn index_of(stem: &str) -> Option<usize> {
    CAMPAIGN.iter().position(|m| m.stem.eq_ignore_ascii_case(stem))
}

/// What follows it, wrapping round at the end.
pub fn after(stem: &str) -> &'static str {
    match index_of(stem) {
        Some(at) => CAMPAIGN[(at + 1) % CAMPAIGN.len()].stem,
        None => FIRST,
    }
}
