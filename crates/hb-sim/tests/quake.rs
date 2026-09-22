//! The door cycle: shot open, up, hold, down, rest.

use hb_formats::quake::Entry;
use hb_sim::quake::{Layer, Quakes, State, Trigger};

/// A box entry as the file gives one: cell (column 65, row 48) of set A,
/// rising to 7040 from a resting bottom of 128, one second each way with a
/// four second hold, shot open.
fn door() -> Entry {
    Entry {
        kind: 1,
        heights: [7040, 128],
        where_: vec![48, 65, 1],
        watch: [-1, 0, 0, 0],
        motion: [0, 65536, 262144, 65536, 0],
        flags: vec![0, 1, 0, 1, 1],
        sounds: vec![Some("1-0UDOOR.WAV".into()), Some("1-0DDOOR.WAV".into()), None, None, None],
        extra: 7,
        switch: Some((0, vec![None, None])),
    }
}

/// The box as the terrain has it: 640 thick, resting with its bottom at 128.
fn resting() -> Quakes {
    quakes(vec![door()], Vec::new())
}

fn quakes(boxes: Vec<Entry>, ground: Vec<Entry>) -> Quakes {
    Quakes::new(&hb_formats::quake::Quake { ground, boxes }, |_, _| (128, 768))
}

#[test]
fn a_shot_inside_the_box_opens_it_and_one_outside_does_not() {
    let mut doors = resting();
    assert_eq!(doors.doors[0].trigger, Trigger::Shot);
    // The wrong cell.
    assert_eq!(doors.shot((64, 48), 400.0), 0);
    // Above the box.
    assert_eq!(doors.shot((65, 48), 900.0), 0);
    // Inside it.
    assert_eq!(doors.shot((65, 48), 400.0), 1);
    assert_eq!(doors.doors[0].state, State::About);
    // And not twice.
    assert_eq!(doors.shot((65, 48), 400.0), 0);
}

#[test]
fn it_rises_for_a_second_holds_for_four_and_comes_back() {
    let mut doors = resting();
    doors.shot((65, 48), 400.0);
    let thickness = doors.doors[0].top - doors.doors[0].bottom;

    // The first frame starts it moving and plays the sound going up.
    doors.step(1.0 / 30.0);
    assert_eq!(doors.doors[0].state, State::Out);
    assert_eq!(doors.sounds.len(), 1);
    assert_eq!(doors.sounds[0].name, "1-0UDOOR.WAV");

    // A second of frames puts the top at the target.
    let mut moved = 0;
    for _ in 0..30 {
        moved += doors.step(1.0 / 30.0).len();
    }
    assert!(moved > 25, "it moved on {moved} frames");
    let door = &doors.doors[0];
    assert!((door.top - door.target).abs() < 1.0, "top {} target {}", door.top, door.target);
    assert!((door.top - door.bottom - thickness).abs() < 0.01, "it kept its thickness");
    assert_eq!(door.state, State::Hold);

    // It holds for four seconds without moving.
    let mut held = 0;
    while doors.doors[0].state == State::Hold {
        assert!(doors.step(1.0 / 30.0).is_empty(), "it does not move while it holds");
        held += 1;
    }
    assert!((115..=121).contains(&held), "{held} frames of holding, not four seconds");
    assert_eq!(doors.doors[0].state, State::Back);
    assert_eq!(doors.sounds[1].name, "1-0DDOOR.WAV");

    // And a second back down to where it started.
    for _ in 0..31 {
        doors.step(1.0 / 30.0);
    }
    let door = &doors.doors[0];
    assert_eq!(door.state, State::Rest);
    assert!((door.bottom - door.rest).abs() < 0.01, "bottom {} rest {}", door.bottom, door.rest);
    assert!((door.top - door.bottom - thickness).abs() < 0.01);
}

/// A switch lights while the door it points at is away from rest, and goes
/// dark when it settles (`0x412a50`).
#[test]
fn a_switch_wears_its_lit_texture_while_its_door_is_open() {
    let mut switch = door();
    switch.switch = Some((1, vec![Some("SWON.RAW".into()), Some("SWOFF.RAW".into())]));
    switch.extra = 7;
    let mut opened = door();
    opened.flags = vec![0, 1, 0, 1, 4];
    opened.watch = [7, 0, 0, 0];
    opened.where_ = vec![48, 66, 1];
    opened.switch = Some((0, vec![None, None]));

    let mut quakes = quakes(vec![switch, opened], Vec::new());
    assert!(quakes.doors[0].switch.is_some());
    assert!(!quakes.doors[0].lit);

    quakes.shot((65, 48), 400.0);
    // The switch itself starts, the other follows, and the switch lights.
    for _ in 0..4 {
        quakes.step(1.0 / 30.0);
    }
    assert!(quakes.doors[0].lit, "the switch did not light");
    let lit = quakes.swaps.first().expect("no swap").clone();
    assert_eq!(lit.cell, (65, 48));
    assert_eq!(lit.texture.as_deref(), Some("SWON.RAW"));

    // It goes dark once the door has been through its whole cycle.
    quakes.swaps.clear();
    for _ in 0..(12 * 30) {
        quakes.step(1.0 / 30.0);
        if !quakes.doors[0].lit {
            break;
        }
    }
    assert!(!quakes.doors[0].lit, "the switch stayed lit");
    assert_eq!(quakes.swaps.last().unwrap().texture.as_deref(), Some("SWOFF.RAW"));
}

#[test]
fn a_door_that_watches_an_id_follows_the_one_that_carries_it() {
    let mut first = door();
    first.extra = 7;
    let mut second = door();
    // Watching by id: the flags line's fifth number is 4 and the watch
    // line's first is the id it follows.
    second.flags = vec![0, 1, 0, 1, 4];
    second.watch = [7, 0, 0, 0];
    second.where_ = vec![48, 66, 1];
    second.extra = 0;

    let mut doors = quakes(vec![first, second], Vec::new());
    assert!(matches!(doors.doors[1].trigger, Trigger::Watching(_)));
    doors.shot((65, 48), 400.0);
    // Starting one starts whatever watches it, there and then: a switch does
    // not travel, so waiting for it to move would never wake anything.
    assert_ne!(doors.doors[1].state, State::Rest, "the watcher should be going");
    doors.step(1.0 / 30.0);
    doors.step(1.0 / 30.0);
    // Both are moving now, and the second cell moves too.
    let moved = doors.step(1.0 / 30.0);
    assert!(moved.iter().any(|m| m.cell == (65, 48)));
    for _ in 0..40 {
        doors.step(1.0 / 30.0);
    }
    assert!(doors.doors[1].top > 700.0, "the second door followed");
}

#[test]
fn an_entry_with_two_zero_heights_never_goes_anywhere() {
    let mut still = door();
    still.heights = [0, 0];
    let mut doors = quakes(vec![still], Vec::new());
    doors.shot((65, 48), 400.0);
    for _ in 0..30 {
        doors.step(1.0 / 30.0);
    }
    let door = &doors.doors[0];
    assert_eq!(door.bottom, 128.0);
    assert_eq!(door.top, 768.0);
}

/// A ground entry as the file gives one: the single cell (60, 40) of the
/// chamber ceiling, rising from -7040 to -3200 over two seconds, holding
/// none, and starting itself - the shape 320 of the shipped entries have.
fn patch() -> Entry {
    Entry {
        kind: 1,
        heights: [-3200, -7040],
        where_: vec![60, 40, 60, 40, 3],
        watch: [0, 0, 0, 0],
        motion: [0, 131072, 0, 131072, 0],
        flags: vec![1, 1, 1, 0],
        sounds: vec![None; 5],
        extra: 0,
        switch: None,
    }
}

#[test]
fn a_patch_that_starts_itself_runs_up_and_down_for_ever() {
    let mut quakes = Quakes::new(
        &hb_formats::quake::Quake { ground: vec![patch()], boxes: Vec::new() },
        |_, _| (-7040, -7040),
    );
    assert_eq!(quakes.patches.len(), 1);
    assert_eq!(quakes.patches[0].layer, Layer::ChamberCeiling);
    assert_eq!(quakes.patches[0].cells().count(), 1);
    assert_eq!(quakes.patches[0].trigger, Trigger::Always);

    // Nothing has to start it: two seconds of frames take it to the top.
    let mut highest = f32::MIN;
    for _ in 0..(2 * 30 + 2) {
        for m in quakes.step(1.0 / 30.0) {
            assert_eq!(m.cell, (60, 40));
            assert_eq!(m.bottom, m.top, "a ground cell has one height");
            highest = highest.max(m.top as f32);
        }
    }
    assert!((highest - -3200.0).abs() < 200.0, "it reached {highest}");
    // It holds for nothing at all, so it is at the top or already coming
    // back down.
    assert!(matches!(quakes.patches[0].state, State::Hold | State::Back));

    // And two more bring it back down and start it over.
    let mut lowest = f32::MAX;
    for _ in 0..(2 * 30 + 2) {
        for m in quakes.step(1.0 / 30.0) {
            lowest = lowest.min(m.top as f32);
        }
    }
    assert!((lowest - -7040.0).abs() < 200.0, "it came back to {lowest}");
    // It may be resting for this one frame, but it starts itself again the
    // next, and comes back up.
    quakes.step(1.0 / 30.0);
    assert_ne!(quakes.patches[0].state, State::Rest, "it never stays settled");
    let mut again = f32::MIN;
    for _ in 0..(2 * 30 + 2) {
        for m in quakes.step(1.0 / 30.0) {
            again = again.max(m.top as f32);
        }
    }
    assert!((again - -3200.0).abs() < 200.0, "the second time up reached {again}");
}

#[test]
fn a_patch_covers_every_cell_of_its_rectangle_and_wraps() {
    let mut wide = patch();
    // Two by three, starting three cells short of the world's edge.
    wide.where_ = vec![126, 40, 127, 42, 1];
    let quakes = Quakes::new(
        &hb_formats::quake::Quake { ground: vec![wide], boxes: Vec::new() },
        |_, _| (0, 0),
    );
    let cells: Vec<_> = quakes.patches[0].cells().collect();
    assert_eq!(cells.len(), 6);
    assert_eq!(cells[0], (126, 40));
    assert_eq!(cells[5], (127, 42));
    assert!(cells.iter().all(|c| (0..128).contains(&c.0) && (0..128).contains(&c.1)));

    // And one that runs over the edge wraps rather than leaving the grid.
    let mut over = patch();
    over.where_ = vec![127, 40, 1, 40, 1];
    let quakes = Quakes::new(
        &hb_formats::quake::Quake { ground: vec![over], boxes: Vec::new() },
        |_, _| (0, 0),
    );
    assert_eq!(quakes.patches[0].cells().collect::<Vec<_>>(), vec![(127, 40), (0, 40), (1, 40)]);
}

/// Against the game: every live box entry names a cell that has a box, and
/// that box is parked at one end of the entry's two heights - its bottom at
/// the second, so it opens upward, or its top already at the first, so it
/// starts open and the first move arrives at once. That is what says the two
/// heights are the top's target and the bottom's, which is what the travel
/// is computed from.
#[test]
fn the_shipped_doors_rest_where_their_second_height_says() {
    let Some(pod) = hb_pod::game_pod("GAME.POD") else { return };
    let (mut entries, mut resting, mut boxed, mut shot_open) = (0, 0, 0, 0);
    for stem in ["float", "hoth", "iowah", "jurasic", "kreash", "morbos", "roid", "ship"] {
        let terrain = hb_formats::terrain::Terrain::load(|ext| {
            pod.read("data", &format!("{stem}.{ext}")).ok().map(<[u8]>::to_vec)
        })
        .unwrap();
        let Ok(bytes) = pod.read("data", &format!("{stem}.qke")) else { continue };
        let quake = hb_formats::quake::parse(bytes).unwrap();
        let doors = Quakes::new(&quake, |layer, (x, z)| match layer {
            Layer::BoxA => (terrain.boxes_a.bottom.at(x, z), terrain.boxes_a.top.at(x, z)),
            Layer::BoxB => (terrain.boxes_b.bottom.at(x, z), terrain.boxes_b.top.at(x, z)),
            Layer::Ground => (terrain.ground.at(x, z), terrain.ground.at(x, z)),
            Layer::ChamberFloor => {
                (terrain.chambers.floor.at(x, z), terrain.chambers.floor.at(x, z))
            }
            Layer::ChamberCeiling => {
                (terrain.chambers.ceiling.at(x, z), terrain.chambers.ceiling.at(x, z))
            }
        });
        for door in &doors.doors {
            // An entry with two zero heights has nowhere to go, and 264 of
            // the shipped ones are like that.
            if door.target == 0.0 && door.rest == 0.0 {
                continue;
            }
            entries += 1;
            shot_open += usize::from(door.trigger == Trigger::Shot);
            // A cell with a box has some thickness to it.
            if door.top > door.bottom {
                boxed += 1;
            }
            // Parked at one end or the other.
            if (door.bottom - door.rest).abs() < 1.0 || (door.top - door.target).abs() < 1.0 {
                resting += 1;
            }
        }
    }
    assert!(entries > 150, "{entries} live entries that go somewhere");
    assert!(shot_open > 50, "{shot_open} of them are shot open");
    assert!(boxed * 10 > entries * 9, "only {boxed} of {entries} name a cell with a box");
    assert!(
        resting * 10 > entries * 9,
        "only {resting} of {entries} are parked at one of their two heights"
    );
}
