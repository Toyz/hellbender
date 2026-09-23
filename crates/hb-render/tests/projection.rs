//! The camera and the rasteriser, checked without needing the game.

use hb_formats::act::Palette;
use hb_formats::fixed::from_units;
use hb_formats::raw::{Image, Shape};
use hb_formats::Angle;
use hb_render::raster::{Shade, Vertex};
use hb_render::{Camera, Target};

#[test]
fn yaw_zero_looks_along_positive_z() {
    let camera = Camera::looking_at(0, 0, 0, Angle(0));
    let ahead = camera.to_view(0, 0, 10 << 16);
    let behind = camera.to_view(0, 0, -10 << 16);
    assert!(ahead[2] > 9.9, "{ahead:?}");
    assert!(behind[2] < -9.9, "{behind:?}");
    // Centred: no sideways or vertical offset.
    assert!(ahead[0].abs() < 1e-3 && ahead[1].abs() < 1e-3, "{ahead:?}");
}

#[test]
fn a_quarter_turn_looks_along_positive_x() {
    // The engine's heading increases toward +x - measured from the recorded
    // demo flight. The port had this backwards until the demo said otherwise.
    let camera = Camera::looking_at(0, 0, 0, Angle(0x4000));
    let view = camera.to_view(10 << 16, 0, 0);
    assert!(view[2] > 9.9, "{view:?}");
    // And three quarters looks the other way.
    let camera = Camera::looking_at(0, 0, 0, Angle(0xc000));
    let view = camera.to_view(-10 << 16, 0, 0);
    assert!(view[2] > 9.9, "{view:?}");
}

#[test]
fn the_camera_looks_the_way_a_ship_on_that_heading_travels() {
    // hb-fly moves along (sin h, cos h) in x and z for heading h. The camera
    // has to look the same way or turning and then accelerating goes sideways,
    // which is what it did before the heading convention was fixed.
    for heading in [0u16, 0x2000, 0x4000, 0x6000, 0x8000, 0xa000, 0xc000, 0xe000] {
        let camera = Camera::looking_at(0, 0, 0, Angle(heading));
        let h = Angle(heading).to_radians();
        let (dx, dz) = (from_units(h.sin() * 10.0), from_units(h.cos() * 10.0));
        let view = camera.to_view(dx, 0, dz);
        assert!(view[2] > 9.9, "heading {heading:#06x}: {view:?}");
        assert!(view[0].abs() < 0.01, "heading {heading:#06x}: {view:?}");
    }
}

#[test]
fn height_is_carried_through_unrotated_when_the_pitch_is_zero() {
    let camera = Camera::looking_at(0, 0, 0, Angle(0x1234));
    let view = camera.to_view(0, 7 << 16, 0);
    assert!((view[1] - 7.0).abs() < 1e-3, "{view:?}");
}

#[test]
fn the_rasteriser_fills_a_triangle_and_respects_the_depth_buffer() {
    let mut target = Target::new(64, 64);
    target.clear(0);
    let texture = Image {
        shape: Shape::new(4, 4),
        pixels: vec![7; 16],
    };
    let shade = Shade {
        light: None,
        fog: None,
        fog_start: 48.0, fog_range: 16.0,
        index_zero_is_clear: false,
    };
    let at = |x: f32, y: f32, depth: f32| Vertex { x, y, depth, u: 0.0, v: 0.0, light: 255.0 };
    target.triangle([at(2.0, 2.0, 5.0), at(60.0, 2.0, 5.0), at(2.0, 60.0, 5.0)], &texture, &shade);
    let filled = target.colour.iter().filter(|&&c| c == 7).count();
    assert!(filled > 1_400, "only {filled} pixels filled");

    // A farther triangle over the same area changes nothing.
    let other = Image { shape: Shape::new(4, 4), pixels: vec![9; 16] };
    target.triangle([at(2.0, 2.0, 9.0), at(60.0, 2.0, 9.0), at(2.0, 60.0, 9.0)], &other, &shade);
    assert_eq!(target.colour.iter().filter(|&&c| c == 9).count(), 0);

    // A nearer one replaces it.
    target.triangle([at(2.0, 2.0, 1.0), at(60.0, 2.0, 1.0), at(2.0, 60.0, 1.0)], &other, &shade);
    assert_eq!(target.colour.iter().filter(|&&c| c == 9).count(), filled);
}

#[test]
fn a_degenerate_triangle_draws_nothing_rather_than_dividing_by_zero() {
    let mut target = Target::new(16, 16);
    target.clear(0);
    let texture = Image { shape: Shape::new(2, 2), pixels: vec![3; 4] };
    let shade = Shade {
        light: None,
        fog: None,
        fog_start: 48.0, fog_range: 16.0,
        index_zero_is_clear: false,
    };
    let at = |x: f32| Vertex { x, y: 8.0, depth: 1.0, u: 0.0, v: 0.0, light: 255.0 };
    target.triangle([at(1.0), at(8.0), at(15.0)], &texture, &shade);
    assert!(target.colour.iter().all(|&c| c == 0));
}

#[test]
fn the_target_renders_through_a_palette() {
    let mut target = Target::new(2, 1);
    target.colour[0] = 1;
    target.colour[1] = 2;
    let mut palette = Palette::default();
    palette.colours[1] = [10, 20, 30];
    palette.colours[2] = [40, 50, 60];
    assert_eq!(target.to_rgb(&palette), vec![10, 20, 30, 40, 50, 60]);
}

#[test]
fn a_negative_pitch_reads_as_negative_when_it_is_used_linearly() {
    // -5219 is the nose-up pitch the demo records at 38 seconds. Stored as a
    // u16 it is 60317, which is 331 degrees; the signed reading is -28.7.
    let up = Angle((-5219i32) as u16);
    assert!((up.to_signed_radians().to_degrees() + 28.67).abs() < 0.05);
    assert!(up.to_radians().to_degrees() > 330.0);
    // And the sine and cosine agree either way, which is why only linear uses
    // were affected.
    assert!((up.to_radians().sin() - up.to_signed_radians().sin()).abs() < 1e-5);
}

#[test]
fn texture_coordinates_are_perspective_correct() {
    // A quad seen at a slant: the near edge at depth 1, the far edge at 4.
    // Halfway up the screen between them is not halfway along the texture -
    // it is where 1/z is halfway, which is depth 1.6 and a fifth of the way.
    let mut target = Target::new(64, 64);
    target.clear(0);
    let stripes = Image { shape: Shape::new(1, 256), pixels: (0..=255u8).collect() };
    let shade = Shade { light: None, fog: None, fog_start: 48.0, fog_range: 16.0, index_zero_is_clear: false };
    let v = |x: f32, y: f32, depth: f32, tv: f32| Vertex { x, y, depth, u: 0.0, v: tv, light: 255.0 };
    let (near, far) = (60.0, 4.0);
    target.triangle([v(0.0, near, 1.0, 0.0), v(64.0, near, 1.0, 0.0), v(0.0, far, 4.0, 255.0)], &stripes, &shade);
    target.triangle([v(64.0, near, 1.0, 0.0), v(64.0, far, 4.0, 255.0), v(0.0, far, 4.0, 255.0)], &stripes, &shade);
    let middle = ((near + far) / 2.0) as usize;
    let sampled = target.colour[middle * 64 + 32] as f32;
    // 1/z halfway between 1 and 1/4 is 5/8: depth 1.6, t = (1/1 - 5/8)/(1 - 1/4) = 0.5
    // in 1/z, which is v = 255 * (1.6 - 1) / (4 - 1) = 51.
    assert!((sampled - 51.0).abs() < 4.0, "sampled row {sampled}");
}

#[test]
fn light_is_interpolated_across_a_triangle() {
    use hb_formats::colour::Ramp;
    // A ramp whose row n maps every index to n: the written index is the row.
    let mut bytes = vec![0u8; 4096];
    for row in 0..16 {
        for i in 0..256 {
            bytes[row * 256 + i] = row as u8;
        }
    }
    let ramp = Ramp::parse(&bytes).unwrap();
    let mut target = Target::new(64, 8);
    target.clear(99);
    let flat = Image { shape: Shape::new(1, 1), pixels: vec![1] };
    let shade = Shade { light: Some(&ramp), fog: None, fog_start: 48.0, fog_range: 16.0, index_zero_is_clear: false };
    let v = |x: f32, y: f32, light: f32| Vertex { x, y, depth: 1.0, u: 0.0, v: 0.0, light };
    target.triangle([v(0.0, 0.0, 255.0), v(64.0, 0.0, 0.0), v(0.0, 8.0, 255.0)], &flat, &shade);
    target.triangle([v(64.0, 0.0, 0.0), v(64.0, 8.0, 0.0), v(0.0, 8.0, 255.0)], &flat, &shade);
    // Full light at the left edge is row 0; none at the right is row 15.
    let row = &target.colour[4 * 64..5 * 64];
    assert_eq!(row[0], 0);
    assert_eq!(row[63], 15);
    assert!(row.windows(2).all(|w| w[0] <= w[1]), "{row:?}");
}

#[test]
fn a_view_direction_turns_back_into_the_world_direction_it_came_from() {
    let mut camera = Camera::looking_at(3 << 16, 5 << 16, -7 << 16, Angle(0x2345));
    camera.pitch = Angle(0xf000);
    camera.roll = Angle(0x0c00);
    for world in [[1.0f32, 0.0, 0.0], [0.0, 1.0, 0.0], [0.3, -0.4, 0.8]] {
        let at = |k: usize| from_units(world[k] * 10.0);
        let view = camera.to_view(camera.x + at(0), camera.y + at(1), camera.z + at(2));
        let back = camera.to_world_direction(view);
        for k in 0..3 {
            assert!((back[k] - world[k] * 10.0).abs() < 1e-3, "{world:?} came back as {back:?}");
        }
    }
}

#[test]
fn the_view_is_ninety_degrees_across_and_down_as_the_engine_sets_it() {
    // `0x485940` from `setViewport(0, 0, W, H)`: half the size, rounded down
    // to even, less one; centred one past the scale across, on the half down.
    assert_eq!(Camera::screen(320, 200), ([159.0, 99.0], [160.0, 100.0]));
    assert_eq!(Camera::screen(320, 400), ([159.0, 199.0], [160.0, 200.0]));
    assert_eq!(Camera::screen(640, 480), ([319.0, 239.0], [320.0, 240.0]));
    // A point 45 degrees up and 45 across lands on the corner of the frame.
    let camera = Camera::looking_at(0, 0, 0, Angle(0));
    let [x, y, z] = camera.to_view(10 << 16, 10 << 16, 10 << 16);
    let ([sx, sy], [cx, cy]) = Camera::screen(320, 200);
    assert!((cx + x * sx / z - 319.0).abs() < 1e-3 && (cy - y * sy / z - 1.0).abs() < 1e-3);
}

#[test]
fn rolled_left_the_horizon_drops_on_the_right() {
    let mut camera = Camera::looking_at(0, 0, 0, Angle(0));
    camera.roll = Angle(0x1000);
    // A point ahead and to the right, level with the eye.
    let [x, y, _] = camera.to_view(10 << 16, 0, 20 << 16);
    assert!(x > 0.0 && y < 0.0, "({x}, {y})");
}
