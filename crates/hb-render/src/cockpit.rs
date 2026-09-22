//! The hand on the stick, and the rest of the cockpit art.
//!
//! `0x41f900` builds the cockpit for the mode the game is in: it scales every
//! position out of a 640x480 layout, loads `ckpt<h>.raw` and the 27 hands, and
//! computes where the hand goes. `0x420607` draws one of them each frame, and
//! `0x41fde0` chooses which.

/// The three sets of nine hands, in the order their names are listed at
/// `0x5018f4`. Which set is used is the trigger's doing (`0x41fde0`): the
/// weapon key held picks [`WEAPON`], the fire key held picks [`FIRE`], and
/// neither picks [`REST`].
pub const SETS: [&str; 3] = ["hn", "hp", "ht"];
/// The fire key's set - `hn`.
pub const FIRE: usize = 0;
/// Neither key's - `hp`.
pub const REST: usize = 1;
/// The weapon key's - `ht`.
pub const WEAPON: usize = 2;

/// The nine positions within a set: back, middle and forward crossed with
/// left, middle and right. The chooser's index is `4 + column + 3 * row`, so
/// this is in that order and the middle is 4.
pub const CELLS: [&str; 9] =
    ["bl", "bm", "br", "ml", "mm", "mr", "fl", "fm", "fr"];

/// How far a control has to be pushed before the hand moves: a quarter.
///
/// `0x41fde0` compares each axis with `0x40` where a full push is `0x100` -
/// the engine hands it the control input divided by its own `0x2492` scale,
/// which is [`hb_sim::flight`]'s `SEVENTH`.
pub const CORNER: f32 = 0.25;

/// Which of [`CELLS`] a stick at `(x, y)`, each -1 to 1, is in. `x` is the
/// turn, positive right; `y` is the pitch, positive forward - which is the
/// nose going down, as `hb-sim`'s flight model has it.
pub fn cell(x: f32, y: f32) -> usize {
    let step = |v: f32| {
        if v < -CORNER {
            0
        } else if v > CORNER {
            2
        } else {
            1
        }
    };
    step(y) * 3 + step(x)
}

/// The file a set and a cell name in `ART`, for one of the three modes.
pub fn hand(set: usize, cell: usize, mode: u32) -> String {
    format!("{}{}{}.raw", SETS[set], CELLS[cell], mode)
}

/// Where the hand is drawn, and how big it is, at this frame size.
///
/// The engine scales a 640x480 layout: the hand is 280 by 110 there, its left
/// edge at 180, and its bottom on the bottom of the frame (`0x41f956` on, and
/// the draw at `0x420607`). At 320x200 that is the 140 by 46 the art is.
pub fn hand_box(width: usize, height: usize) -> [usize; 4] {
    let w = 280 * width / 640;
    let h = 110 * height / 480;
    [180 * width / 640, height.saturating_sub(h), w, h]
}

/// The knob is loaded beside the hands (`knob<h>.raw`, 12 by 17 of the same
/// layout) and its box is computed at `0x41fb46`, but nothing in the image
/// reads either again - see `docs/engine/cockpit.md`.
pub fn knob_size(width: usize, height: usize) -> [usize; 2] {
    [12 * width / 640, 17 * height / 480]
}
