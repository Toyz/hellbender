//! The targeting marks (`0x47c4b0`, `0x47c910`, `0x47c820`).

use hb_render::hud::{self, mark_half, target_box, target_diamond, LOCK_SIZE, OBJECTIVE_SIZE};
use hb_render::Target;

fn at(t: &Target, x: isize, y: isize) -> u8 {
    t.colour[y as usize * t.width + x as usize]
}

fn blank(w: usize, h: usize) -> Target {
    let mut t = Target::new(w, h);
    t.clear(0xff);
    t
}

#[test]
fn a_mark_is_half_its_size_at_320_by_200_and_all_of_it_at_640_by_480() {
    assert_eq!(mark_half(LOCK_SIZE, 320, 200), (8, 8));
    assert_eq!(mark_half(LOCK_SIZE, 320, 400), (8, 17));
    assert_eq!(mark_half(LOCK_SIZE, 640, 480), (17, 17));
    assert_eq!(mark_half(OBJECTIVE_SIZE, 320, 200), (10, 10));
}

#[test]
fn the_box_is_black_the_colour_and_black_from_the_outside_in() {
    let mut t = blank(320, 200);
    target_box(&mut t, 100, 100, LOCK_SIZE, hud::LOCK_COLOUR, None);
    // Along the top edge's middle, from the outside in.
    assert_eq!(at(&t, 100, 91), 0xff);
    assert_eq!(at(&t, 100, 92), 0);
    assert_eq!(at(&t, 100, 93), hud::LOCK_COLOUR);
    assert_eq!(at(&t, 100, 94), 0);
    assert_eq!(at(&t, 100, 95), 0xff, "hollow");
    // And the corners are square.
    assert_eq!(at(&t, 92, 92), 0);
    assert_eq!(at(&t, 108, 108), 0);
}

#[test]
fn the_diamond_has_its_points_above_below_and_beside() {
    let mut t = blank(320, 200);
    target_diamond(&mut t, 100, 100, LOCK_SIZE, hud::LOCK_COLOUR, None);
    assert_eq!(at(&t, 100, 92), 0, "top point");
    assert_eq!(at(&t, 108, 100), 0, "right point");
    assert_eq!(at(&t, 100, 93), hud::LOCK_COLOUR);
    assert_eq!(at(&t, 92, 92), 0xff, "no corner");
}

#[test]
fn the_health_bar_shows_what_is_left_in_three_colours() {
    let bar = |now: f32| {
        let mut t = blank(320, 200);
        target_box(&mut t, 100, 100, OBJECTIVE_SIZE, hud::OBJECTIVE_COLOUR, Some((now, 4.0)));
        // Under the box: from x - 10, two below its bottom edge at y + 10.
        let row: Vec<u8> = (90..=110).map(|x| at(&t, x, 112)).collect();
        row
    };
    let full = bar(4.0);
    assert!(full.iter().all(|&c| c == hud::ESCORT_COLOUR), "{full:?}");
    let half = bar(2.0);
    assert_eq!(half[0], hud::ESCORT_COLOUR);
    assert_eq!(half[15], 0, "the rest is black");
    assert_eq!(bar(1.5)[0], 0x8f);
    assert_eq!(bar(0.5)[0], 0x97);
}
