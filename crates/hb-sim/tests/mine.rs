//! The floating mine: laid behind a moving ship, armed and set off by that
//! same ship and nothing else.

use hb_sim::mine::{Blast, Field, Mine, BLAST, SPEED, SLOTS, TRIGGER};

fn field() -> Field {
    Field::new()
}

#[test]
fn it_is_refused_below_six_units_a_second() {
    let mut mines = field();
    let ahead = [0.0, 0.0, 1.0];
    assert!(!Field::fast_enough(SPEED - 0.1));
    assert_eq!(mines.lay([0.0; 3], ahead, SPEED - 0.1, 1.0), None);
    assert_eq!(mines.live().count(), 0);

    assert!(Field::fast_enough(SPEED));
    assert_eq!(mines.lay([0.0; 3], ahead, SPEED, 1.0), Some(0));
    assert_eq!(mines.live().count(), 1);
}

#[test]
fn it_lands_a_unit_behind_the_ship() {
    let mut mines = field();
    // Flying along +z, so the mine goes down at z = -1.
    mines.lay([10.0, 20.0, 30.0], [0.0, 0.0, 1.0], 16.0, 1.0);
    let mine = *mines.live().next().unwrap();
    assert_eq!(mine.at, [10.0, 20.0, 29.0]);
    assert!(!mine.armed);
}

#[test]
fn it_holds_a_hundred_and_no_more() {
    let mut mines = field();
    for i in 0..SLOTS {
        assert_eq!(mines.lay([0.0, i as f32 * 100.0, 0.0], [0.0; 3], 16.0, 1.0), Some(i));
    }
    assert_eq!(mines.lay([0.0; 3], [0.0; 3], 16.0, 1.0), None);
    assert_eq!(mines.live().count(), SLOTS);
}

#[test]
fn it_arms_and_goes_off_on_the_ship_that_laid_it() {
    let mut mines = field();
    // Laid from z = 1 flying along +z, so it lands at the origin.
    mines.lay([0.0, 0.0, 1.0], [0.0, 0.0, 1.0], 16.0, 0.25);
    // Far away: nothing happens, and it does not turn.
    let far = [200.0, 0.0, 200.0];
    for _ in 0..30 {
        assert!(mines.step(1.0 / 30.0, far).is_empty());
    }
    let mine: Mine = *mines.live().next().unwrap();
    assert!(!mine.armed && mine.angles == [0.0; 2], "{mine:?}");

    // Inside the trigger radius: it arms and goes off in the same frame,
    // because the engine's two tests run one after the other.
    let blasts = mines.step(1.0 / 30.0, [0.0, 0.0, TRIGGER - 1.0]);
    assert_eq!(blasts, vec![Blast { at: [0.0; 3], damage: 0.25 }]);
    assert_eq!(mines.live().count(), 0, "the slot is freed");
}

#[test]
fn the_blast_reaches_twice_as_far_as_the_trigger() {
    // Which is why the ship that sets one off is always inside it.
    assert_eq!(BLAST, TRIGGER * 2.0);
}

#[test]
fn an_armed_mine_turns_while_it_waits() {
    let mut mines = field();
    mines.lay([0.0; 3], [0.0; 3], 16.0, 1.0);
    // Arm it by coming close, then step it with the ship gone: it stays
    // armed and turns.
    mines.slots[0].as_mut().unwrap().armed = true;
    let far = [200.0, 0.0, 200.0];
    for _ in 0..30 {
        assert!(mines.step(1.0 / 30.0, far).is_empty());
    }
    let mine = *mines.live().next().unwrap();
    // A quarter of a circle a second on one axis, an eighth on the other.
    assert!((mine.angles[0] - 16384.0).abs() < 600.0, "{:?}", mine.angles);
    assert!((mine.angles[1] - 8192.0).abs() < 300.0, "{:?}", mine.angles);
}
