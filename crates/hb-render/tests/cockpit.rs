//! The cockpit art the engine names, against what is in the archives.

use hb_render::cockpit;

/// Four views and 27 hands, in every one of the three modes.
#[test]
fn every_picture_the_engine_names_is_there() {
    let Some(pod) = hb_pod::game_pod("STARTUP.POD") else { return };
    for mode in [200, 400, 480] {
        for which in 0..4 {
            let name = cockpit::view(which, mode);
            assert!(pod.read("art", &name).is_ok(), "{name} is missing");
        }
        for set in 0..3 {
            for cell in 0..9 {
                let name = cockpit::hand(set, cell, mode);
                assert!(pod.read("art", &name).is_ok(), "{name} is missing");
            }
        }
    }
}

/// A hand is as wide as its box says, and as tall give or take the row the
/// engine's own divide loses: 110 * 200 / 480 is 45 where the picture is 46.
#[test]
fn the_hand_is_the_size_its_box_says() {
    let Some(pod) = hb_pod::game_pod("STARTUP.POD") else { return };
    for (mode, w, h) in [(200, 320, 200), (400, 320, 400), (480, 640, 480)] {
        let [_, _, bw, bh] = cockpit::hand_box(w, h);
        let bytes = pod.read("art", &cockpit::hand(cockpit::REST, 4, mode)).unwrap();
        assert_eq!(bytes.len() % bw, 0, "mode {mode}: {} is not {bw} wide", bytes.len());
        let rows = bytes.len() / bw;
        assert!(rows == bh || rows == bh + 1, "mode {mode}: {rows} rows against a box of {bh}");
    }
}

/// The middle of the nine is the centred hand, and a quarter push moves off it.
#[test]
fn the_cell_moves_at_a_quarter_push() {
    assert_eq!(cockpit::CELLS[cockpit::cell(0.0, 0.0)], "mm");
    assert_eq!(cockpit::CELLS[cockpit::cell(0.2, -0.2)], "mm");
    assert_eq!(cockpit::CELLS[cockpit::cell(0.3, 0.0)], "mr");
    assert_eq!(cockpit::CELLS[cockpit::cell(-0.3, 0.0)], "ml");
    assert_eq!(cockpit::CELLS[cockpit::cell(0.0, 0.3)], "fm");
    assert_eq!(cockpit::CELLS[cockpit::cell(0.0, -0.3)], "bm");
    assert_eq!(cockpit::CELLS[cockpit::cell(-1.0, 1.0)], "fl");
}

/// The player's own ship is in `STARTUP.POD` and loads with the level.
#[test]
fn the_players_ship_is_there() {
    let Some(level) = hb_render::Level::from_disc("morbos") else { return };
    let mesh = level.ship_mesh.expect("no ship.bin");
    let model = level.meshes[mesh].as_ref().expect("the slot is empty");
    assert!(!model.vertices.is_empty(), "the ship has no vertices");
    assert!(!model.polygons.is_empty(), "the ship has no polygons");
}

/// Eve's welcome, wrapped to the panel, fits inside it.
#[test]
fn the_panel_wraps_a_long_line() {
    use hb_formats::hud_font::HudFont;
    let dir = hb_pod::game_dir();
    let Ok(exe) = std::fs::read(dir.join("HELLBEND.EXE")) else {
        eprintln!("skipping: no executable");
        return;
    };
    let font = HudFont::read(&exe).unwrap();
    let eve = hb_sim::phrases::phrase(128).expect("no phrase 128").text;
    for (w, _h) in [(320usize, 200usize), (640, 480)] {
        let width = (hb_render::hud::PANEL[2] as usize * w / 640).saturating_sub(4);
        let lines = hb_render::hud::wrap(&font, &[eve.to_string()], width);
        for line in &lines {
            assert!(
                font.width(line) <= width,
                "{w}: {:?} is {} wide in a {width} panel",
                line,
                font.width(line)
            );
        }
        println!("{w}: {} lines, widest {}", lines.len(), lines.iter().map(|l| font.width(l)).max().unwrap());
    }
}
