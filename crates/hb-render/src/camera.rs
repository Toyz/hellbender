//! Where the eye is, and how world space reaches the screen.

use hb_formats::Angle;

/// World units are 16.16 fixed point and a terrain cell is 8.0 of them, so a
/// position is an `i32` triple in the same units the `.NAV` and `.DEF` files
/// use.
#[derive(Debug, Clone, Copy, Default)]
pub struct Camera {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Heading, in the engine's own convention. The circle is 16 bits, so
    /// 0x4000 is a quarter turn.
    ///
    /// 0 looks along +z and the heading increases toward +x, so 0x4000 looks
    /// along +x. That is measured, not chosen: in `DEMO1.DMO` the third angle
    /// of each recorded pose equals `atan2(dx, dz)` of the direction the ship
    /// is travelling, with a median error of 0.8 degrees over 790 samples.
    pub yaw: Angle,
    /// Positive pitch tilts the view downward.
    pub pitch: Angle,
    /// Horizontal field of view as a fraction of a turn. The engine's own is
    /// not known; 90 degrees is this renderer's choice.
    pub fov: Angle,
    /// How far the ground is drawn, in world units. Beyond it the fog ramp has
    /// saturated anyway.
    pub far: i32,
}

impl Camera {
    pub fn looking_at(x: i32, y: i32, z: i32, yaw: Angle) -> Camera {
        Camera {
            x,
            y,
            z,
            yaw,
            pitch: Angle(0),
            fov: Angle(0x4000),
            far: 220 << 16,
        }
    }

    /// World to view space: translate, then rotate by -yaw about y and -pitch
    /// about x. Returns 16.16 values, with +z into the screen.
    pub fn to_view(&self, x: i32, y: i32, z: i32) -> [f32; 3] {
        let (dx, dy, dz) = (
            (x - self.x) as f32 / 65536.0,
            (y - self.y) as f32 / 65536.0,
            (z - self.z) as f32 / 65536.0,
        );
        // The engine's heading is 0 along +z and increases toward +x - the
        // recorded demo flight says so, to a median of 0.8 degrees. So a
        // point along the heading has to land on the view axis, which is a
        // rotation by +yaw here, not -yaw.
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sp, cp) = (-self.pitch.to_radians()).sin_cos();
        let rx = dx * cy - dz * sy;
        let rz = dx * sy + dz * cy;
        let ry = dy * cp - rz * sp;
        let rz = dy * sp + rz * cp;
        [rx, ry, rz]
    }
}
