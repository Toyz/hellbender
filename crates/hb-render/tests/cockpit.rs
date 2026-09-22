//! The cockpit art the engine names, against what is in the archives.

use hb_render::cockpit;

fn startup() -> Option<hb_pod::Pod> {
    let dir = std::env::var_os("HB_GAME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../original")
        });
    let pod = dir.join("system/STARTUP.POD");
    if !pod.exists() {
        eprintln!("skipping: {} is not there", pod.display());
        return None;
    }
    hb_pod::Pod::open(pod).ok()
}

/// Four views and 27 hands, in every one of the three modes.
#[test]
fn every_picture_the_engine_names_is_there() {
    let Some(pod) = startup() else { return };
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
    let Some(pod) = startup() else { return };
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
