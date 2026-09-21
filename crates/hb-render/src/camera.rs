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
    /// Positive roll lowers the left wing - the sign the recorded demo uses,
    /// where left turns hold a positive roll.
    pub roll: Angle,
}

impl Camera {
    pub fn looking_at(x: i32, y: i32, z: i32, yaw: Angle) -> Camera {
        Camera {
            x,
            y,
            z,
            yaw,
            pitch: Angle(0),
            roll: Angle(0),
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
        if self.roll.0 == 0 {
            return [rx, ry, rz];
        }
        // Rolled with the left wing down, the view's right axis tilts up.
        let (sr, cr) = self.roll.to_radians().sin_cos();
        [rx * cr + ry * sr, ry * cr - rx * sr, rz]
    }

    /// How view space reaches a screen of this size: the scale on x and y and
    /// the centre, as the engine's viewport setup derives them from the view
    /// rectangle - for the in-game view the whole screen,
    /// `setViewport(0, 0, W, H)` at `0x45a0f1`.
    ///
    /// `0x42c9d0` is the same setup for a *small* rectangle, and it is the
    /// one the reticle, the objective arrow, the weapon picture and the
    /// briefing's globe are drawn through. It pushes the whole camera state
    /// onto parallel stacks at `0x53a4a8` and up first, then sets the centre
    /// to `(x + w/2, y + h/2)` and the scale to `(w/2, h/2)`, both in 16.16,
    /// and puts the zoom at `0x512550` back to 1.0. So a viewport is this
    /// same projection over a different rectangle, which is why one
    /// `Camera::screen` serves the whole port.
    ///
    /// Each scale is half the rectangle's size, rounded down to even, less
    /// one, and the projection is `x * sx / z + cx` and `y * sy / z + cy` with
    /// clip codes that test `|x| <= z` and `|y| <= z` (`0x42a805`). So the
    /// field of view is 90 degrees across **and 90 degrees down**, whatever
    /// the screen's shape: at 320x200 the scales are 159 and 99.
    pub fn screen(width: usize, height: usize) -> ([f32; 2], [f32; 2]) {
        let half = |n: usize| ((n / 2) & !1) as f32;
        let (sx, sy) = (half(width) - 1.0, half(height) - 1.0);
        ([sx, sy], [sx + 1.0, half(height)])
    }

    /// A view-space direction turned back into world space: the inverse of
    /// [`Camera::to_view`]'s rotation, without the translation.
    pub fn to_world_direction(&self, view: [f32; 3]) -> [f32; 3] {
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sp, cp) = (-self.pitch.to_radians()).sin_cos();
        let (sr, cr) = self.roll.to_radians().sin_cos();
        let [vx, vy, rz] = view;
        let (rx, ry) = (vx * cr - vy * sr, vy * cr + vx * sr);
        let dy = ry * cp + rz * sp;
        let rz1 = -ry * sp + rz * cp;
        [rx * cy + rz1 * sy, dy, -rx * sy + rz1 * cy]
    }
}
