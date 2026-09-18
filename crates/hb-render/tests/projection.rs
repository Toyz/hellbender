//! The camera and the rasteriser, checked without needing the game.

use hb_formats::act::Palette;
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
fn a_quarter_turn_looks_along_negative_x() {
    let camera = Camera::looking_at(0, 0, 0, Angle(0x4000));
    let view = camera.to_view(-10 << 16, 0, 0);
    assert!(view[2] > 9.9, "{view:?}");
    // And three quarters looks the other way.
    let camera = Camera::looking_at(0, 0, 0, Angle(0xc000));
    let view = camera.to_view(10 << 16, 0, 0);
    assert!(view[2] > 9.9, "{view:?}");
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
        intensity: 255,
        fog_distance: 100.0,
        index_zero_is_clear: false,
    };
    let at = |x: f32, y: f32, depth: f32| Vertex { x, y, depth, u: 0.0, v: 0.0 };
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
        intensity: 255,
        fog_distance: 100.0,
        index_zero_is_clear: false,
    };
    let at = |x: f32| Vertex { x, y: 8.0, depth: 1.0, u: 0.0, v: 0.0 };
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
