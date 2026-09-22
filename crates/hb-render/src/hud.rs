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

use crate::camera::Camera;
use crate::raster::Target;
use hb_formats::raw::Image;
use hb_formats::hud_font::{HudFont, LINE};
use hb_formats::mrgl::Model;

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

/// The radar, which sits over the dish in the cockpit art: a square at
/// 507, 6 in 640x480, 101 on a side (`0x474bb8` builds it out of the view's
/// size). With no cockpit and the view zoomed the top moves to the view's
/// own top, which is not here.
pub const RADAR: [isize; 4] = [507, 6, 101, 101];

/// How much of the world fits across the box. `0x474f92` shifts the offset
/// down 11, `0x474ffc` multiplies by the box's width and shifts down 11
/// again, so a world unit is `width / 64` pixels - 64 units across.
pub const RADAR_ACROSS: f32 = 64.0;

/// And the edge is round, not square: `0x474fe9` tests the squared offset
/// against 0xef420 before any of that, which is 30.9 world units.
pub const RADAR_REACH: f32 = 30.9;

/// What the objective arrow is drawn in: `0x475333` puts -101 in the shade
/// global for an objective ahead and `0x475327` -152 for one behind, and a
/// negative shade is a palette index outright.
pub const ARROW_AHEAD: u8 = 101;
pub const ARROW_BEHIND: u8 = 152;

/// What a blip is drawn in when its class is one the guns count as a target
/// (`0x40dc00` says so, `0x47519e` picks the colour), and when it is not.
pub const BLIP_TARGET: u8 = 0x3f;
pub const BLIP_OTHER: u8 = 0x94;

/// The classes `0x40dc00` answers yes for - its table at `0x40dc5c` maps a
/// class to a jump, and every entry but 0x0d returns 1. Class 63 and up is
/// no as well. The missile lock asks the same question (`0x47bce6`).
pub fn is_target(class: i64) -> bool {
    matches!(
        class,
        0 | 1
            | 3
            | 5
            | 6
            | 9..=15
            | 19..=24
            | 27
            | 30
            | 31
            | 33
            | 34
            | 36
            | 37
            | 41
            | 42
            | 45
            | 47
            | 61
            | 62
    )
}

/// One thing on the radar: where it is relative to the player in world
/// units, right and forward, already wrapped and turned by the heading; what
/// colour it takes; and whether it is below the player, which flattens it.
#[derive(Debug, Clone, Copy)]
pub struct Blip {
    pub right: f32,
    pub forward: f32,
    pub colour: u8,
    pub below: bool,
}

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
    /// What the radar has. Empty while the radar is destroyed
    /// (`0x512744`).
    pub blips: &'a [Blip],
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

/// The radar. `0x475100` walks the placements once a frame, drops the dead,
/// the dying and classes 18 and 33, turns each offset by the heading, and
/// hands it to `0x474f00`, which is what this is: a wrapped offset scaled
/// into the box, clipped to a circle, and a blip drawn at it.
pub fn radar(target: &mut Target, blips: &[Blip]) {
    let (w, h) = (target.width, target.height);
    let [rx, ry, rw, rh] = RADAR;
    let (x0, y0) = (rx * w as isize / 640, ry * h as isize / 480);
    let (bw, bh) = (rw * w as isize / 640, rh * h as isize / 480);
    for blip in blips {
        if blip.right.hypot(blip.forward) > RADAR_REACH {
            continue;
        }
        let x = x0 + bw / 2 + (blip.right * bw as f32 / RADAR_ACROSS) as isize;
        let y = y0 + bh / 2 - (blip.forward * bh as f32 / RADAR_ACROSS) as isize;
        mark(target, x, y, blip.colour, blip.below);
    }
}

/// One blip, the shape `0x4746a0` draws: a three-pixel bar with a pixel
/// above and below it, outlined in the paper colour - and for something
/// below the player, the bar alone.
pub fn mark(target: &mut Target, x: isize, y: isize, colour: u8, below: bool) {
    let (w, h) = (target.width, target.height);
    let mut dot = |dx: isize, dy: isize, index: u8| {
        fill(&mut target.colour, w, h, x + dx, y + dy, 1, 1, index);
    };
    for (dx, dy) in [(-1, -1), (1, -1), (-2, 0), (2, 0), (-1, 1), (1, 1)] {
        dot(dx, dy, 0);
    }
    for dx in -1..=1 {
        dot(dx, 0, colour);
    }
    if below {
        dot(0, -1, 0);
        dot(0, 1, 0);
    } else {
        dot(0, -2, 0);
        dot(0, 2, 0);
        dot(0, -1, colour);
        dot(0, 1, colour);
    }
}

/// Where the weapon's picture goes: 14, 3, 63 by 56 in the same 640x480
/// (`0x41f8fa` builds it out of the view's size). That is the left end of
/// the top-left panel, with the weapon and ammunition lines to its right -
/// and the message panel, 16, 3, 236 by 56, covers it, which is why those
/// lines step aside for a message too.
pub const WEAPON_ICON: [isize; 4] = [14, 3, 63, 56];

/// The twelve pictures (`0x501838`), each a 64x64 `.RAW` in the art.
pub const ICONS: [&str; 12] = [
    "valk", "d4s", "skl", "f6rfl1", "dom4s", "c4s", "vip4s", "cls4s", "m4s", "mg4s", "f6mine1",
    "f6super",
];

/// Which picture each weapon row uses (`0x501868`, read with the row as the
/// index). A row with none of its own gets the first, the Valkyrie's, and
/// `0x420e8e` treats anything outside 0..12 as an error.
pub const WEAPON_ICONS: [usize; 31] = [
    0, 2, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 6, 0, 0, 0, 0, 5, 7, 8, 9, 10, 0, 11,
];

/// The weapon's picture, scaled into its box. The engine gets there the long
/// way - it sets the viewport to the box, hangs the picture on a quad as a
/// texture and draws it (`0x420e70`) - which comes to the same thing.
pub fn icon(target: &mut Target, image: &Image) {
    let (w, h) = (target.width, target.height);
    let [ix, iy, iw, ih] = WEAPON_ICON;
    let (x0, y0) = (ix * w as isize / 640, iy * h as isize / 480);
    let (bw, bh) = (iw * w as isize / 640, ih * h as isize / 480);
    // Cleared first, one pixel in and two short, as `0x420ecb` does.
    fill(&mut target.colour, w, h, x0 + 1, y0 + 1, bw - 3, bh - 3, 0);
    let (sw, sh) = (image.shape.width, image.shape.height);
    if sw == 0 || sh == 0 || bw <= 0 || bh <= 0 {
        return;
    }
    for y in 0..bh {
        for x in 0..bw {
            let (px, py) = (x0 + x, y0 + y);
            if px < 0 || py < 0 || px >= w as isize || py >= h as isize {
                continue;
            }
            let sx = (x as usize * sw / bw as usize).min(sw - 1);
            let sy = (y as usize * sh / bh as usize).min(sh - 1);
            let index = image.pixels[sy * sw + sx];
            if index != 0 {
                target.colour[py as usize * w + px as usize] = index;
            }
        }
    }
}

/// The objective arrow, which the engine draws on the radar: `0x474bb8`
/// sets the viewport to the dish and `0x475290` draws `navtarg.bin` into it
/// turned by `0x8000` less the bearing, with the camera at the origin
/// looking straight down. That model is one quad - a tip, two shoulders and
/// a base - so the port turns those four points in two dimensions instead of
/// reproducing a viewport whose camera setup has not been read.
///
/// `bearing` is the engine's own `0x59d118`: the direction from the point to
/// the player, less the player's heading, in the 16-bit circle. Its colour
/// is the shade `0x475317` picks, which is one index for an objective
/// roughly behind you and another for anything else.
pub fn arrow(target: &mut Target, bearing: u16) {
    let behind = (0x7800..0x8800).contains(&bearing);
    let colour = if behind { ARROW_BEHIND } else { ARROW_AHEAD };
    let (w, h) = (target.width, target.height);
    let [rx, ry, rw, rh] = RADAR;
    let (x0, y0) = (rx * w as isize / 640, ry * h as isize / 480);
    let (bw, bh) = (rw * w as isize / 640, rh * h as isize / 480);
    let (cx, cy) = (x0 as f32 + bw as f32 / 2.0, y0 as f32 + bh as f32 / 2.0);
    // The model, in its own units: the tip is 0.4425 out and the shoulders
    // 0.061 either side of it.
    let shape = [(0.0f32, 0.4425f32), (0.061, 0.061), (0.0, 0.0), (-0.061, 0.061)];
    let scale = bh as f32 * 0.45 / 0.4425;
    let turn = (bearing as f32 / 65536.0) * std::f32::consts::TAU;
    let (sin, cos) = turn.sin_cos();
    let points: Vec<(f32, f32)> = shape
        .iter()
        .map(|&(x, y)| {
            let (x, y) = (x * scale, y * scale);
            (cx + x * cos + y * sin, cy + x * sin - y * cos)
        })
        .collect();
    // A quad drawn as two triangles of pixels, which at this size is a few
    // spans.
    for tri in [[0, 1, 2], [0, 2, 3]] {
        let pts: Vec<(f32, f32)> = tri.iter().map(|&i| points[i]).collect();
        let top = pts.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor() as isize;
        let bottom = pts.iter().map(|p| p.1).fold(f32::MIN, f32::max).ceil() as isize;
        let left = pts.iter().map(|p| p.0).fold(f32::MAX, f32::min).floor() as isize;
        let right = pts.iter().map(|p| p.0).fold(f32::MIN, f32::max).ceil() as isize;
        for py in top..=bottom {
            for px in left..=right {
                let p = (px as f32 + 0.5, py as f32 + 0.5);
                let side = |a: (f32, f32), b: (f32, f32)| {
                    (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
                };
                let (d0, d1, d2) =
                    (side(pts[0], pts[1]), side(pts[1], pts[2]), side(pts[2], pts[0]));
                let inside = (d0 >= 0.0 && d1 >= 0.0 && d2 >= 0.0)
                    || (d0 <= 0.0 && d1 <= 0.0 && d2 <= 0.0);
                if inside && px >= x0 && px < x0 + bw && py >= y0 && py < y0 + bh {
                    fill(&mut target.colour, w, h, px, py, 1, 1, colour);
                }
            }
        }
    }
}

/// The reticle, which is a model rather than anything drawn by hand:
/// `target.bin` in `STARTUP.POD`, nine vertices fifty units ahead and two
/// across, four triangles from the middle out to the up, left, down and
/// right of a small octagon. `0x4652bd` draws it with the camera at the
/// origin and no rotation, so it lands in the middle of the view.
///
/// Its colour walks between 32 and 63 and back, a step a frame
/// (`0x465223`), which is the green band of the palette - the shade global
/// takes it negated, and a negative shade is a palette index outright.
pub fn reticle(target: &mut Target, model: &Model, colour: u8) {
    let (w, h) = (target.width, target.height);
    let ([sx, sy], [cx, cy]) = Camera::screen(w, h);
    let point = |v: &hb_formats::mrgl::Vertex| {
        let z = v.z as f32 / 65536.0;
        if z <= 0.0 {
            return None;
        }
        Some((
            cx + (v.x as f32 / 65536.0) / z * sx,
            cy - (v.y as f32 / 65536.0) / z * sy,
        ))
    };
    for polygon in &model.polygons {
        let Some(first) = polygon.corners.first() else { continue };
        let Some(a) = model.vertices.get(first.vertex as usize).and_then(point) else { continue };
        for pair in polygon.corners[1..].windows(2) {
            let (Some(b), Some(c)) = (
                model.vertices.get(pair[0].vertex as usize).and_then(point),
                model.vertices.get(pair[1].vertex as usize).and_then(point),
            ) else {
                continue;
            };
            flat(target, [a, b, c], colour);
        }
    }
}

/// A triangle straight into the frame, with no depth test and no shading.
///
/// The reticle is fifty units ahead of an eye that is not the player's, so
/// tested against the world's depth it vanishes into the first hillside -
/// which is what it did. It is an overlay: it goes on top.
fn flat(target: &mut Target, tri: [(f32, f32); 3], colour: u8) {
    let (w, h) = (target.width, target.height);
    let top = tri.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor().max(0.0) as isize;
    let bottom = tri.iter().map(|p| p.1).fold(f32::MIN, f32::max).ceil().min(h as f32) as isize;
    let left = tri.iter().map(|p| p.0).fold(f32::MAX, f32::min).floor().max(0.0) as isize;
    let right = tri.iter().map(|p| p.0).fold(f32::MIN, f32::max).ceil().min(w as f32) as isize;
    let side = |a: (f32, f32), b: (f32, f32), p: (f32, f32)| {
        (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
    };
    for py in top..bottom {
        for px in left..right {
            let p = (px as f32 + 0.5, py as f32 + 0.5);
            let (d0, d1, d2) = (side(tri[0], tri[1], p), side(tri[1], tri[2], p), side(tri[2], tri[0], p));
            let inside =
                (d0 >= 0.0 && d1 >= 0.0 && d2 >= 0.0) || (d0 <= 0.0 && d1 <= 0.0 && d2 <= 0.0);
            if inside {
                target.colour[py as usize * w + px as usize] = colour;
            }
        }
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
    if !readout.blips.is_empty() {
        radar(target, readout.blips);
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

/// The top-left panel, which is the other place the game writes to you:
/// 16, 3, 236 by 56 in 640x480 (`0x420080` builds it out of the view's
/// size), cleared and written seven pixels a line - eight lines. While
/// something is in it, `0x674d68` is set and the weapon and ammunition lines
/// stand aside, because it is the same strip of screen and the weapon's
/// picture is under it too.
pub const PANEL: [isize; 4] = [16, 3, 236, 56];

/// Write into the panel, most recent line last. More than eight lines and
/// the oldest fall off the top.
pub fn panel(target: &mut Target, font: &HudFont, lines: &[String]) {
    let (w, h) = (target.width, target.height);
    let [x, y, pw, ph] = PANEL;
    let (x, y) = (x * w as isize / 640, y * h as isize / 480);
    let (pw, ph) = (pw * w as isize / 640, ph * h as isize / 480);
    fill(&mut target.colour, w, h, x, y, pw, ph, 0);
    let rows = (ph / LINE as isize).max(1) as usize;
    let wrapped = wrap(font, lines, (pw - 4).max(8) as usize);
    let from = wrapped.len().saturating_sub(rows);
    for (i, line) in wrapped[from..].iter().enumerate() {
        font.draw(&mut target.colour, w, h, x + 2, y + i as isize * LINE as isize, line, INK);
    }
}

/// Break lines to a pixel width, on spaces, the way the engine's own wrap does
/// (`0x484920`): a word that does not fit starts the next line.
pub fn wrap(font: &HudFont, lines: &[String], width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        let mut row = String::new();
        for word in line.split_whitespace() {
            let candidate =
                if row.is_empty() { word.to_string() } else { format!("{row} {word}") };
            if font.width(&candidate) <= width || row.is_empty() {
                row = candidate;
            } else {
                out.push(std::mem::take(&mut row));
                row = word.to_string();
            }
        }
        out.push(row);
    }
    out
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
