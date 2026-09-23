//! The cell geometry, checked against the constants in the binary.
//!
//! These need no game data: they are about arithmetic transcribed from
//! `HELLBEND.EXE`, and the point is that the transcription is self-consistent.

use hb_formats::fixed::to_units;
use hb_formats::terrain::CELL_SIZE;
use hb_world::grid::{
    cell_fraction, centroid, half_containing, sample_point, triangle, triangle_containing,
    Corner, FRACTION_ONE, SAMPLE_HIGH, SAMPLE_LOW,
};
use hb_world::{Cell, Diagonal, Half};
use hb_world::grid::triangle_containing as tri_at;

#[test]
fn a_cell_is_eight_units_and_the_grid_wraps() {
    assert_eq!(CELL_SIZE, 1 << 19);
    assert_eq!(to_units(CELL_SIZE), 8.0);
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

#[test]
fn the_interpolated_height_meets_the_corner_heights_at_the_corners() {
    // No game data: a flat synthetic terrain, then a sloped one, checked
    // against the corner altitudes the same grid reports.
    use hb_formats::terrain::{Altitudes, BoxLayer, ChamberLayer, Indices, Layer, Scale, Terrain, CELLS, SIDE};
    use hb_world::Grid;

    let flat = |v: u8| Altitudes::parse(&vec![v; CELLS], Scale::Up).unwrap();
    let down = |v: u8| Altitudes::parse(&vec![v; CELLS], Scale::Down).unwrap();
    let indices = |per| Indices::parse(&vec![0u8; CELLS * per * 2], per, "t").unwrap();

    // A ramp along x: altitude byte equals the cell's x index.
    let mut ramp = vec![0u8; CELLS];
    for z in 0..SIDE {
        for x in 0..SIDE {
            ramp[z * SIDE + x] = x as u8;
        }
    }
    let terrain = Terrain {
        ground: Altitudes::parse(&ramp, Scale::Up).unwrap(),
        colour: indices(1),
        boxes_a: BoxLayer { bottom: flat(0), top: flat(0), textures: indices(6) },
        chambers: ChamberLayer { floor: down(255), ceiling: down(255), textures: indices(2) },
        boxes_b: BoxLayer { bottom: down(255), top: down(255), textures: indices(6) },
        shading: None,
    };
    let grid = Grid::new(&terrain);

    // At a cell's origin corner the interpolated height is that corner's.
    for cx in [0i32, 1, 17, 60] {
        for cz in [0i32, 1, 9] {
            let (ox, oz) = Cell::new(cx, cz).origin();
            let corner = grid.height_at_grid(Layer::Ground, cx, cz).unwrap();
            let interpolated = grid.height_at(Layer::Ground, ox, oz).unwrap();
            assert_eq!(interpolated, corner, "cell ({cx},{cz})");
        }
    }

    // Halfway along x between two cells the height is halfway between them,
    // on a ramp that rises one altitude step per cell.
    let (ox, oz) = Cell::new(10, 4).origin();
    let low = grid.height_at_grid(Layer::Ground, 10, 4).unwrap();
    let high = grid.height_at_grid(Layer::Ground, 11, 4).unwrap();
    let mid = grid.height_at(Layer::Ground, ox + CELL_SIZE / 2, oz).unwrap();
    assert!(
        (mid - (low + high) / 2).abs() <= 16,
        "midpoint {mid} between {low} and {high}"
    );

    // The query is defined everywhere and never leaves the terrain's range.
    for i in 0..500 {
        let x = i * 7919;
        let z = i * 104_729;
        let h = grid.height_at(Layer::Ground, x, z).unwrap();
        assert!((0..=(255 << 15)).contains(&h), "at ({x},{z}) height {h}");
        // And it lands in the triangle the engine's own test picks.
        let tri = tri_at(x, z);
        assert_eq!(tri.cell, Cell::containing(x, z));
    }
}

#[test]
fn the_height_is_right_for_negative_coordinates_too() {
    // The world's own coordinates are signed and centred. height_at used to
    // take the in-cell fraction as `x - cell.origin()`, which is only right for
    // 0..1024 and gave fractions like -127.5 for anything negative. The
    // engine's recorded demo flight found it; this pins it without the game.
    use hb_formats::terrain::{Altitudes, BoxLayer, ChamberLayer, Indices, Layer, Scale, Terrain, CELLS, SIDE};
    use hb_world::Grid;

    let flat = |v: u8| Altitudes::parse(&vec![v; CELLS], Scale::Up).unwrap();
    let down = |v: u8| Altitudes::parse(&vec![v; CELLS], Scale::Down).unwrap();
    let indices = |per| Indices::parse(&vec![0u8; CELLS * per * 2], per, "t").unwrap();
    let mut ramp = vec![0u8; CELLS];
    for z in 0..SIDE {
        for x in 0..SIDE {
            ramp[z * SIDE + x] = (x * 2) as u8;
        }
    }
    let terrain = Terrain {
        ground: Altitudes::parse(&ramp, Scale::Up).unwrap(),
        colour: indices(1),
        boxes_a: BoxLayer { bottom: flat(0), top: flat(0), textures: indices(6) },
        chambers: ChamberLayer { floor: down(255), ceiling: down(255), textures: indices(2) },
        boxes_b: BoxLayer { bottom: down(255), top: down(255), textures: indices(6) },
        shading: None,
    };
    let grid = Grid::new(&terrain);

    // A signed position and the same position wrapped by a whole world
    // (1024 units, which is 1 << 26) name the same cell and the same point in
    // it, so they must return the same height.
    let world = 1 << 26;
    for units in [-500i32, -300, -129, -1, 3, 100, 400] {
        for frac in [0, 1 << 17, 3 << 17] {
            let x = (units << 16) + frac;
            let z = 7 << 19;
            let signed = grid.height_at(Layer::Ground, x, z).unwrap();
            let wrapped = grid.height_at(Layer::Ground, x.wrapping_add(world), z).unwrap();
            assert_eq!(signed, wrapped, "at {units} units + {frac}");
            // And it stays inside the ramp's range.
            assert!((0..=(255 << 15)).contains(&signed), "at {units} units: {signed}");
        }
    }
}
