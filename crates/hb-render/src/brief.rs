//! The briefing screen: a level's `.TXT` typed into the panel of
//! `BRIEF.RAW`, in the HUD's font.
//!
//! Every briefing is drawn on the same backdrop; the name in the file is the
//! texture the globe wears. `FONT.BIN` is 23 pixels a line and a briefing runs
//! to seventeen lines, which does not fit a 200-line screen, so the text is in
//! the small font. Which font the engine uses here is not read.

use hb_formats::act::Palette;
use hb_formats::brief::PANEL;
use hb_formats::hud_font::{HudFont, LINE};

/// The ink: the brightest entry of the briefing's palette.
pub fn ink(palette: &Palette) -> u8 {
    (0..=255u8).max_by_key(|&i| palette.rgb(i).iter().map(|&c| c as u32).sum::<u32>()).unwrap_or(255)
}

/// How many rows of text the panel holds.
pub fn rows() -> usize {
    PANEL[3] / LINE
}

/// The rows showing once `revealed` characters have been typed: the text
/// wrapped to the panel, cut where the typing has got to - a line break costs
/// a character, so a blank line takes time too - and scrolled so the last
/// thing typed is in view.
pub fn typed(font: &HudFont, lines: &[String], revealed: usize) -> Vec<String> {
    let mut left = revealed;
    let mut shown: Vec<String> = Vec::new();
    for line in crate::hud::wrap(font, lines, PANEL[2]) {
        if left == 0 {
            break;
        }
        let take = left.min(line.chars().count());
        shown.push(line.chars().take(take).collect());
        left = (left - take).saturating_sub(1);
    }
    let from = shown.len().saturating_sub(rows());
    shown.split_off(from)
}

/// Draw rows into the panel of a backdrop `width` pixels across, as many as
/// fit from the first.
pub fn draw(pixels: &mut [u8], width: usize, height: usize, font: &HudFont, rows: &[String], ink: u8) {
    let [px, py, ..] = PANEL;
    for (i, line) in rows.iter().take(self::rows()).enumerate() {
        font.draw(pixels, width, height, px as isize, (py + i * LINE) as isize, line, ink);
    }
}
