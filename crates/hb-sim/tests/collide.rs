//! Keeping the ship out of the boxes.

use hb_sim::collide::{push_out, Push, Solid, OVERSHOOT, SHIP};

fn block() -> Solid {
    Solid { min: [0.0, 0.0, 0.0], max: [8.0, 6.0, 8.0] }
}

#[test]
fn a_ship_outside_is_left_alone() {
    let (at, push) = push_out([20.0, 3.0, 4.0], SHIP, &[block()]);
    assert_eq!(at, [20.0, 3.0, 4.0]);
    assert_eq!(push, None);
    // Touching by more than its own unit is still clear.
    let (at, push) = push_out([9.5, 3.0, 4.0], SHIP, &[block()]);
    assert_eq!((at[0], push), (9.5, None));
}

#[test]
fn a_ship_against_a_wall_is_pushed_off_it() {
    // Half a unit from the face: pushed the missing half out, and the
    // engine's fraction over it (`0x427841`).
    let (at, push) = push_out([8.5, 3.0, 4.0], SHIP, &[block()]);
    assert_eq!(push, Some(Push::Side));
    assert!((at[0] - (8.5 + 0.5 * OVERSHOOT)).abs() < 1e-3, "{at:?}");
    assert!(at[0] > 8.0 + SHIP, "clear of the face");
    assert_eq!((at[1], at[2]), (3.0, 4.0), "only the way it came");
}

#[test]
fn a_ship_on_top_is_pushed_up_and_knows_it() {
    let (at, push) = push_out([4.0, 6.5, 4.0], SHIP, &[block()]);
    assert_eq!(push, Some(Push::Up));
    assert!(at[1] > 6.9 && at[1] < 7.1, "{at:?}");
}

#[test]
fn a_ship_inside_leaves_by_the_nearest_face() {
    // Just inside the +z face, well away from the others.
    let (at, push) = push_out([4.0, 3.0, 7.5], SHIP, &[block()]);
    assert_eq!(push, Some(Push::Side));
    assert!(at[2] > 8.0, "{at:?}");
    assert_eq!((at[0], at[1]), (4.0, 3.0));
    // Just under the top: up and out.
    let (at, push) = push_out([4.0, 5.5, 4.0], SHIP, &[block()]);
    assert_eq!(push, Some(Push::Up));
    assert!(at[1] > 6.0, "{at:?}");
}

#[test]
fn a_corner_between_two_boxes_settles_outside_both() {
    let boxes = [
        Solid { min: [0.0, 0.0, 0.0], max: [8.0, 6.0, 8.0] },
        Solid { min: [8.0, 0.0, 0.0], max: [16.0, 6.0, 8.0] },
    ];
    let (at, _) = push_out([8.0, 3.0, 8.2], SHIP, &boxes);
    for b in &boxes {
        let near: Vec<f32> = (0..3).map(|k| at[k].clamp(b.min[k], b.max[k])).collect();
        let d: f32 = (0..3).map(|k| (at[k] - near[k]).powi(2)).sum::<f32>().sqrt();
        assert!(d >= SHIP - 1e-3, "{at:?} is {d} from a box");
    }
}

/// A real level: flying at a box from outside ends up outside it.
#[test]
fn the_boxes_of_a_level_stop_a_ship() {
    let Some(pod) = hb_pod::game_pod("GAME.POD") else { return };
    let terrain = hb_formats::terrain::Terrain::load(|ext| {
        pod.read("data", &format!("float.{ext}")).ok().map(<[u8]>::to_vec)
    })
    .unwrap();
    let grid = hb_world::Grid::new(&terrain);

    // Find a cell with a box and walk into it from outside.
    let fixed = |v: f32| (v * 65536.0) as i32;
    let mut tried = 0;
    for cx in 0..128i32 {
        for cz in 0..128i32 {
            let cell = hb_world::Cell::new(cx, cz);
            let Some((bottom, top)) = grid.box_span(hb_world::Layer::BoxA, cell) else { continue };
            if !grid.has_box(hb_world::Layer::BoxA, cell) || top - bottom < 1 << 16 {
                continue;
            }
            let (ox, oz) = cell.signed_origin();
            let middle = [
                ox as f32 / 65536.0 + 4.0,
                (bottom + top) as f32 / 131072.0,
                oz as f32 / 65536.0 + 4.0,
            ];
            let reach = (SHIP * 65536.0) as i32;
            let solids: Vec<Solid> = grid
                .boxes_near(fixed(middle[0]), fixed(middle[2]), reach)
                .into_iter()
                .map(Solid::of)
                .collect();
            assert!(!solids.is_empty(), "the box under the ship should be found");
            let (at, push) = push_out(middle, SHIP, &solids);
            assert!(push.is_some(), "a ship in the middle of a box is pushed out");
            assert_ne!(at, middle);
            tried += 1;
            if tried > 20 {
                return;
            }
        }
    }
    assert!(tried > 0, "float has boxes");
}
