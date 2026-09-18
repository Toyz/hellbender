//! The cell geometry, checked against the constants in the binary.
//!
//! These need no game data: they are about arithmetic transcribed from
//! `HELLBEND.EXE`, and the point is that the transcription is self-consistent.

use hb_formats::terrain::CELL_SIZE;
use hb_world::grid::{
    cell_fraction, centroid, half_containing, sample_point, triangle, triangle_containing,
    Corner, FRACTION_ONE, SAMPLE_HIGH, SAMPLE_LOW,
};
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

#[test]
fn a_position_lands_in_the_half_whose_sample_point_is_nearest() {
    // `half_containing` is transcribed from the engine's branch; the sample
    // points are transcribed from a different routine. They have to agree, and
    // that they do is the cross-check on both.
    for cx in 0..6 {
        for cz in 0..6 {
            let origin = Cell::new(cx, cz).origin();
            for fx in [0x1000, 0x4000, 0x7000, 0x9000, 0xc000, 0xf000] {
                for fz in [0x1000, 0x4000, 0x7000, 0x9000, 0xc000, 0xf000] {
                    // Skip points on the diagonal itself, where either half is
                    // defensible and the engine just picks one.
                    if fx == fz || fx + fz == FRACTION_ONE {
                        continue;
                    }
                    let (x, z) = (origin.0 + (fx << 3), origin.1 + (fz << 3));
                    let half = half_containing(x, z);
                    let here = sample_point(Cell::new(cx, cz), half);
                    let other = sample_point(
                        Cell::new(cx, cz),
                        if half == Half::First { Half::Second } else { Half::First },
                    );
                    let near = |p: (i32, i32)| {
                        let (dx, dz) = (p.0 as i64 - x as i64, p.1 as i64 - z as i64);
                        dx * dx + dz * dz
                    };
                    assert!(
                        near(here) <= near(other),
                        "cell ({cx},{cz}) at ({fx:#x},{fz:#x}): {half:?} is the further half"
                    );
                }
            }
        }
    }
}

#[test]
fn the_cell_fraction_is_the_low_nineteen_bits_scaled_to_sixteen() {
    assert_eq!(cell_fraction(0), 0);
    assert_eq!(cell_fraction(CELL_SIZE), 0);
    assert_eq!(cell_fraction(CELL_SIZE / 2), FRACTION_ONE / 2);
    assert_eq!(cell_fraction(CELL_SIZE - 8), FRACTION_ONE - 1);
    // The engine computes it as (p & 0x7ffff) * 0x10000 / 0x80000.
    for p in [0, 1, 12345, CELL_SIZE - 1, CELL_SIZE * 7 + 99] {
        let engine = ((p & (CELL_SIZE - 1)) as i64 * 0x10000 / CELL_SIZE as i64) as i32;
        assert_eq!(cell_fraction(p), engine, "at {p}");
    }
}

#[test]
fn every_triangle_has_one_edge_along_each_axis() {
    // The normal routine relies on it: it reads one x-aligned and one
    // z-aligned edge out of every triangle, whichever case it is in.
    for x in 0..8 {
        for z in 0..8 {
            for half in [Half::First, Half::Second] {
                let tri = triangle(Cell::new(x, z), half);
                let c = tri.corners;
                let edges = [(c[0], c[1]), (c[1], c[2]), (c[2], c[0])];
                let along_x = edges.iter().filter(|(a, b)| a.offset().1 == b.offset().1).count();
                let along_z = edges.iter().filter(|(a, b)| a.offset().0 == b.offset().0).count();
                assert_eq!(along_x, 1, "cell ({x},{z}) {half:?}");
                assert_eq!(along_z, 1, "cell ({x},{z}) {half:?}");
            }
        }
    }
}

#[test]
fn the_engines_own_corner_triples_come_back_out() {
    // 0x41b0b0's two readable cases, transcribed as corner offsets.
    //
    // parity 0, fz < fx: reads h(x,z), h(x+1,z), h(x+1,z+1).
    assert_eq!(
        triangle(Cell::new(0, 0), Half::Second).corners,
        [Corner::Origin, Corner::X, Corner::Far]
    );
    // A point three quarters along x and a quarter along z is in that half.
    let inside = triangle_containing(CELL_SIZE * 3 / 4, CELL_SIZE / 4);
    assert_eq!(inside.cell, Cell::new(0, 0));
    assert_eq!(inside.half, Half::Second);

    // parity 1, fx + fz < 1: reads h(x,z), h(x+1,z), h(x,z+1).
    assert_eq!(
        triangle(Cell::new(1, 0), Half::Second).corners,
        [Corner::Origin, Corner::X, Corner::Z]
    );
    let inside = triangle_containing(CELL_SIZE + CELL_SIZE / 4, CELL_SIZE / 4);
    assert_eq!(inside.cell, Cell::new(1, 0));
    assert_eq!(inside.half, Half::Second);
}

#[test]
fn the_world_is_centred_on_the_origin() {
    // Course points run to about +/-512 units in x and z, so a cell index is
    // the middle seven bits of a signed coordinate and the two halves of the
    // grid are the two signs.
    assert_eq!(Cell::new(0, 0).signed_origin(), (0, 0));
    assert_eq!(Cell::new(63, 0).signed_origin().0, 63 * CELL_SIZE);
    assert_eq!(Cell::new(64, 0).signed_origin().0, -64 * CELL_SIZE);
    assert_eq!(Cell::new(127, 0).signed_origin().0, -CELL_SIZE);

    // And a signed coordinate round-trips to the cell that holds it. A world
    // unit is 1 << 16; a cell is eight of them.
    for units in [-511, -300, -8, -1, 0, 1, 7, 300, 511] {
        let coord = units << 16;
        let cell = Cell::containing(coord, coord);
        let (ox, _) = cell.signed_origin();
        assert!(
            coord - ox >= 0 && coord - ox < CELL_SIZE,
            "{units} units: coord {coord} not inside cell {cell:?} at origin {ox}"
        );
    }
}
