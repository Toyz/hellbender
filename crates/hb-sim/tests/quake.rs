//! The door cycle: shot open, up, hold, down, rest.

use hb_formats::quake::Entry;
use hb_sim::quake::{Doors, State, Trigger};

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
fn resting() -> Doors {
    Doors::new(&[door()], |_, _| (128, 768))
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

    let mut doors = Doors::new(&[first, second], |_, _| (128, 768));
    assert!(matches!(doors.doors[1].trigger, Trigger::Watching(_)));
    doors.shot((65, 48), 400.0);
    doors.step(1.0 / 30.0);
    // The first frame the shot one actually moves, the other one starts.
    doors.step(1.0 / 30.0);
    assert_eq!(doors.doors[1].state, State::About);
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
    let mut doors = Doors::new(&[still], |_, _| (128, 768));
    doors.shot((65, 48), 400.0);
    for _ in 0..30 {
        doors.step(1.0 / 30.0);
    }
    let door = &doors.doors[0];
    assert_eq!(door.bottom, 128.0);
    assert_eq!(door.top, 768.0);
}

fn game_dir() -> std::path::PathBuf {
    std::env::var_os("HB_GAME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../original"))
}

/// Against the game: every live box entry names a cell that has a box, and
/// that box is parked at one end of the entry's two heights - its bottom at
/// the second, so it opens upward, or its top already at the first, so it
/// starts open and the first move arrives at once. That is what says the two
/// heights are the top's target and the bottom's, which is what the travel
/// is computed from.
#[test]
fn the_shipped_doors_rest_where_their_second_height_says() {
    let path = game_dir().join("system/GAME.POD");
    if !path.exists() {
        eprintln!("skipping: {} is not there", path.display());
        return;
    }
    let pod = hb_pod::Pod::open(&path).unwrap();
    let (mut entries, mut resting, mut boxed, mut shot_open) = (0, 0, 0, 0);
    for stem in ["float", "hoth", "iowah", "jurasic", "kreash", "morbos", "roid", "ship"] {
        let terrain = hb_formats::terrain::Terrain::load(|ext| {
            pod.read("data", &format!("{stem}.{ext}")).ok().map(<[u8]>::to_vec)
        })
        .unwrap();
        let Ok(bytes) = pod.read("data", &format!("{stem}.qke")) else { continue };
        let quake = hb_formats::quake::parse(bytes).unwrap();
        let doors = Doors::new(&quake.boxes, |(x, z), set| {
            let layer = if set == 1 { &terrain.boxes_a } else { &terrain.boxes_b };
            (layer.bottom.at(x, z), layer.top.at(x, z))
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
