//! The readouts, gauges and labels the engine paints over the cockpit.
//!
//! The layout is a table of twelve boxes at `0x5052d0`, four values each -
//! x, y, width, height - in the 640x480 the HUD is designed for. Every use
//! scales them to the view it is drawing into (`0x44e673` multiplies by the
//! view's size and divides by 640 or 480), so the same numbers serve all
//! three of the game's screen sizes. Two of the twelve, 3 and 5, are not
//! referenced anywhere in the image.
//!
//! Three routines share them. `0x44e5e0` draws the objective code, the
//! distance to it and the six gauges, then the countdown if one is running.
//! `0x44eb0f` draws the weapon and its ammunition, and skips both while a
//! message is up in the top-left panel. `0x44f000` is the cockpit labels,
//! which name every element on the screen and are toggled by the key bound
//! to `keyCockpitLabel`.

use crate::raster::Target;
use hb_formats::hud_font::{HudFont, LINE};

/// The twelve boxes, in 640x480. 3 and 5 are dead.
pub const BOXES: [[isize; 4]; 12] = [
    [312, 3, 80, 17],
    [140, 36, 110, 17],
    [312, 24, 80, 17],
    [514, 96, 88, 17],
    [140, 13, 110, 17],
    [310, 7, 70, 17],
    [50, 373, 14, 50],
    [21, 373, 14, 50],
    [79, 373, 14, 50],
    [547, 373, 14, 50],
    [576, 373, 14, 50],
    [605, 373, 14, 50],
];

pub const OBJECTIVE: usize = 0;
pub const AMMO: usize = 1;
pub const DISTANCE: usize = 2;
pub const WEAPON: usize = 4;

/// What the objective line says, by the mission point's kind (`0x505478`).
/// The jump table at `0x44ead0` sends anything past the twelfth nowhere, so
/// the line keeps whatever it last said.
pub const OBJECTIVES: [&str; 12] = [
    "TGT", "TUN", "CHK", "JMP", "EXT", "GRD", "STR", "EST", "MSG", "RES", "PLY", "BCN",
];

/// The gauges, in the order the routine draws them: which box, where its
/// palette ramp starts, and how many bands it has (`0x44e7d3` on). The names
/// are the game's own, from the cockpit labels.
pub const GAUGES: [(usize, u8, isize); 6] =
    [(6, 0x61, 14), (7, 0x81, 14), (8, 0x21, 30), (9, 0x91, 10), (10, 0xa1, 14), (11, 0xb1, 14)];

/// What each gauge is called, in [`GAUGES`] order (`0x505504` on).
pub const GAUGE_NAMES: [&str; 6] = [
    "Speed",
    "Hull Integrity",
    "Turbo Fuel",
    "Weapon Energy",
    "Ship Energy",
    "Shield Energy",
];

/// What the readouts are written in (`0x44e6f4`).
pub const INK: u8 = 0x2f;

/// What a gauge's empty part is filled with (`0x44e59c`).
pub const EMPTY: u8 = 1;

/// What a label's border is drawn in (`0x44ee8a`).
pub const BORDER: u8 = 0x97;

/// One cockpit label: where it sits in 640x480 (`0x5053a0`), what it says
/// (`0x505504` on), whether the callout is the box's right edge rather than
/// its left, and which HUD box the leader line runs down to.
#[derive(Debug, Clone, Copy)]
pub struct Label {
    pub at: (isize, isize),
    pub text: &'static str,
    pub right: bool,
    pub leader: Option<usize>,
}

/// The labels, in the order `0x44f000` draws them. The three on the right of
/// the screen hang their boxes to the left of the callout, which is what
/// `0x44eec0` is for; the rest run right from it, which is `0x44ee10`.
pub const LABELS: [Label; 11] = [
    Label { at: (23, 250), text: "Hull Integrity", right: false, leader: Some(7) },
    Label { at: (51, 290), text: "Speed", right: false, leader: Some(6) },
    Label { at: (81, 330), text: "Turbo Fuel", right: false, leader: Some(8) },
    Label { at: (562, 330), text: "Weapon Energy", right: true, leader: Some(9) },
    Label { at: (590, 290), text: "Ship Energy", right: true, leader: Some(10) },
    Label { at: (621, 250), text: "Shield Energy", right: true, leader: Some(11) },
    Label { at: (380, 135), text: "Objective Direction", right: false, leader: None },
    Label { at: (200, 77), text: "Objective Abbreviation", right: false, leader: None },
    Label { at: (200, 77), text: "Distance to Objective", right: false, leader: None },
    Label { at: (20, 85), text: "Current Weapon", right: false, leader: None },
    Label { at: (20, 85), text: "Shots left", right: false, leader: None },
];

/// One box, scaled into a view of this size.
pub fn box_of(slot: usize, width: usize, height: usize) -> (isize, isize, isize, isize) {
    let [x, y, w, h] = BOXES[slot];
    (
        x * width as isize / 640,
        y * height as isize / 480,
        w * width as isize / 640,
        h * height as isize / 480,
    )
}

/// What the HUD is told each frame. The six gauges are [`GAUGE_NAMES`], each
/// 0.0 to 1.0.
#[derive(Debug, Clone, Default)]
pub struct Readout<'a> {
    pub weapon: &'a str,
    /// `None` for a weapon with no stock, which the engine writes as `inf`.
    pub ammo: Option<i64>,
    pub objective: Option<&'a str>,
    pub distance: Option<i32>,
    pub gauges: [f32; 6],
    /// The countdown at `0x512620`, in whole seconds. Drawn bottom right
    /// while it runs, and nowhere when it is not.
    pub countdown: Option<i32>,
    /// Whether the cockpit labels are up (`keyCockpitLabel`).
    pub labels: bool,
}

pub fn fill(
    frame: &mut [u8],
    width: usize,
    height: usize,
    x: isize,
    y: isize,
    w: isize,
    h: isize,
    colour: u8,
) {
    for py in y.max(0)..(y + h).min(height as isize) {
        for px in x.max(0)..(x + w).min(width as isize) {
            frame[py as usize * width + px as usize] = colour;
        }
    }
}

/// A filled box with a line around it, which is `0x44e2a0`: the rectangle in
/// `inside`, then four lines a pixel outside it in `border`.
pub fn frame(
    buffer: &mut [u8],
    width: usize,
    height: usize,
    (x, y, w, h): (isize, isize, isize, isize),
    inside: u8,
    border: u8,
) {
    fill(buffer, width, height, x, y, w, h, inside);
    fill(buffer, width, height, x - 1, y - 1, w + 2, 1, border);
    fill(buffer, width, height, x - 1, y + h, w + 2, 1, border);
    fill(buffer, width, height, x - 1, y - 1, 1, h + 2, border);
    fill(buffer, width, height, x + w, y - 1, 1, h + 2, border);
}

/// A gauge: the part above the level in [`EMPTY`], the rest in `steps` bands
/// of the ramp that starts at `ramp`, bottom first.
pub fn gauge(
    frame: &mut [u8],
    width: usize,
    height: usize,
    (x, y, w, h): (isize, isize, isize, isize),
    ramp: u8,
    steps: isize,
    value: f32,
) {
    let filled = (h as f32 * value.clamp(0.0, 1.0)) as isize;
    fill(frame, width, height, x, y, w, h - filled, EMPTY);
    let level = y + h - filled;
    for i in 0..steps.max(1) {
        let top = (y + h - (i + 1) * h / steps).max(level);
        let bottom = (y + h - i * h / steps).max(level);
        if bottom > top {
            fill(frame, width, height, x, top, w, bottom - top, ramp.wrapping_add(i as u8));
        }
    }
}

/// One line, centred in its box on a cleared background.
pub fn line(target: &mut Target, font: &HudFont, slot: usize, text: &str) {
    let (w, h) = (target.width, target.height);
    let (bx, by, bw, bh) = box_of(slot, w, h);
    fill(&mut target.colour, w, h, bx, by, bw, bh, 0);
    let x = bx + (bw - font.width(text) as isize) / 2;
    let y = by + (bh - LINE as isize) / 2;
    font.draw(&mut target.colour, w, h, x, y, text, INK);
}

/// One cockpit label, the way `0x44ee10` draws one: a box as wide as the
/// text and four pixels more, framed, with the text two pixels in. When the
/// callout is a right edge the box hangs to the left of it instead
/// (`0x44eec0`). A gauge's label first gets a line down to it.
pub fn label(target: &mut Target, font: &HudFont, label: &Label) {
    let (w, h) = (target.width, target.height);
    let (vw, vh) = (w as isize, h as isize);
    let (cx, cy) = (label.at.0 * vw / 640, label.at.1 * vh / 480);
    if let Some(slot) = label.leader {
        let (bx, by, _, _) = box_of(slot, w, h);
        let x = bx + 6 * vw / 640;
        fill(&mut target.colour, w, h, x, cy, 1, by - 1 - cy, BORDER);
    }
    let width = font.width(label.text) as isize + 4;
    let height = 17 * vh / 480;
    let x = if label.right { cx - width } else { cx };
    let y = cy + 1;
    frame(&mut target.colour, w, h, (x, y, width, height), 0, BORDER);
    let text_y = y + (height - LINE as isize) / 2 + 1;
    font.draw(&mut target.colour, w, h, x + 2, text_y, label.text, INK);
}

/// Every cockpit label at once, which is what `keyCockpitLabel` turns on.
pub fn legend(target: &mut Target, font: &HudFont) {
    for entry in LABELS.iter() {
        label(target, font, entry);
    }
}

/// The whole readout.
pub fn draw(target: &mut Target, font: &HudFont, readout: &Readout) {
    line(target, font, WEAPON, &format!("Weapon: {}", readout.weapon));
    match readout.ammo {
        Some(n) => line(target, font, AMMO, &format!("Ammo: {n}")),
        None => line(target, font, AMMO, "Ammo: inf"),
    }
    if let Some(objective) = readout.objective {
        line(target, font, OBJECTIVE, &format!("Obj: {objective}"));
    }
    if let Some(distance) = readout.distance {
        line(target, font, DISTANCE, &format!("Dist:{distance:4}"));
    }
    let (w, h) = (target.width, target.height);
    for (i, &(slot, ramp, steps)) in GAUGES.iter().enumerate() {
        gauge(&mut target.colour, w, h, box_of(slot, w, h), ramp, steps, readout.gauges[i]);
    }
    if let Some(seconds) = readout.countdown.filter(|&s| s != 0) {
        countdown(target, font, seconds);
    }
    if readout.labels {
        legend(target, font);
    }
}

/// The countdown, where `0x44e9b8` puts it: the seconds as a bare number in
/// the bottom right corner, on a framed box.
pub fn countdown(target: &mut Target, font: &HudFont, seconds: i32) {
    let (w, h) = (target.width, target.height);
    let text = format!("{seconds}");
    let width = font.width(&text) as isize;
    let x = w as isize - width - 2;
    let y = h as isize - 9;
    frame(&mut target.colour, w, h, (x, y, width, LINE as isize), 8, 0x10);
    font.draw(&mut target.colour, w, h, x, y, &text, INK);
}

/// A message, where `0x481030` puts one: word-wrapped to 240 pixels, centred
/// in the view, three eighths of the way down, on a filled box that runs
/// three pixels left and two above the text to one past its right and bottom.
pub fn message(target: &mut Target, font: &HudFont, text: &str) {
    let (w, h) = (target.width, target.height);
    let width = font.width(text).min(240) as isize;
    let x = (w as isize - width) / 2;
    let y = (h * 3 / 8) as isize;
    fill(&mut target.colour, w, h, x - 3, y - 2, width + 5, LINE as isize + 3, 0);
    font.draw(&mut target.colour, w, h, x, y, text, INK);
}
