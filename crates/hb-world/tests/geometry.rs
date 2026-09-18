//! The cell geometry, checked against the constants in the binary.
//!
//! These need no game data: they are about arithmetic transcribed from
//! `HELLBEND.EXE`, and the point is that the transcription is self-consistent.

use hb_formats::terrain::CELL_SIZE;
use hb_world::grid::{centroid, sample_point, triangle, SAMPLE_HIGH, SAMPLE_LOW};
use hb_world::{Cell, Diagonal, Half};

#[test]
fn a_cell_is_eight_units_and_the_grid_wraps() {
    assert_eq!(CELL_SIZE, 1 << 19);
    assert_eq!(CELL_SIZE as f32 / 65536.0, 8.0);
    // 128 cells of 8.0 units.
    assert_eq!(hb_world::grid::WORLD_SIZE as f64 / 65536.0, 1024.0);
    // Indices wrap rather than clamp, which is what `and eax, 0x7f` does.
    assert_eq!(Cell::new(128, 0), Cell::new(0, 0));
    assert_eq!(Cell::new(-1, -1), Cell::new(127, 127));
    // And a world position resolves by shifting, not dividing.
    assert_eq!(Cell::containing(0, 0), Cell::new(0, 0));
    assert_eq!(Cell::containing(CELL_SIZE * 3 + 17, 0), Cell::new(3, 0));
}

#[test]
fn the_diagonal_alternates_like_a_checkerboard() {
    assert_eq!(Cell::new(0, 0).diagonal(), Diagonal::Main);
    assert_eq!(Cell::new(1, 0).diagonal(), Diagonal::Anti);
    assert_eq!(Cell::new(0, 1).diagonal(), Diagonal::Anti);
    assert_eq!(Cell::new(1, 1).diagonal(), Diagonal::Main);
    // Neighbours never share a diagonal, in either axis.
    for x in 0..16 {
        for z in 0..16 {
            let here = Cell::new(x, z).diagonal();
            assert_ne!(here, Cell::new(x + 1, z).diagonal());
            assert_ne!(here, Cell::new(x, z + 1).diagonal());
            assert_eq!(here, Cell::new(x + 1, z + 1).diagonal());
        }
    }
}

#[test]
fn the_sample_constants_are_the_ones_the_engine_ships() {
    assert_eq!(SAMPLE_LOW, 0x5555);
    assert_eq!(SAMPLE_HIGH, 0xABB9);
    // One is a third to within a rounding; the other is not two thirds, and
    // the port keeps the engine's value rather than correcting it.
    let third = SAMPLE_LOW as f64 / 65536.0;
    assert!((third - 1.0 / 3.0).abs() < 0.0001, "{third}");
    let high = SAMPLE_HIGH as f64 / 65536.0;
    assert!((high - 2.0 / 3.0).abs() > 0.004, "{high} is suspiciously close to 2/3");
    assert!((high - 2.0 / 3.0).abs() < 0.005, "{high} has drifted");
}

#[test]
fn every_sample_point_lands_inside_its_own_triangle() {
    // The corner sets are derived from the sample points, so this checks the
    // derivation: each half's centroid must be the point the engine computes,
    // to within the 0xABB9 fudge.
    let tolerance = CELL_SIZE / 100; // 1% of a cell
    for x in 0..8 {
        for z in 0..8 {
            for half in [Half::First, Half::Second] {
                let cell = Cell::new(x, z);
                let (sx, sz) = sample_point(cell, half);
                let (cx, cz) = centroid(triangle(cell, half));
                assert!(
                    (sx - cx).abs() < tolerance && (sz - cz).abs() < tolerance,
                    "cell ({x},{z}) {half:?}: sample ({sx},{sz}) vs centroid ({cx},{cz})"
                );
            }
        }
    }
}

#[test]
fn the_two_halves_of_a_cell_share_exactly_two_corners() {
    for x in 0..8 {
        for z in 0..8 {
            let cell = Cell::new(x, z);
            let first = triangle(cell, Half::First).corners;
            let second = triangle(cell, Half::Second).corners;
            let shared = first.iter().filter(|c| second.contains(c)).count();
            assert_eq!(shared, 2, "cell ({x},{z}) halves must share the diagonal");
            // Between them the two halves use all four corners.
            let mut all: Vec<_> = first.iter().chain(second.iter()).collect();
            all.sort_by_key(|c| format!("{c:?}"));
            all.dedup();
            assert_eq!(all.len(), 4, "cell ({x},{z}) must cover every corner");
        }
    }
}
