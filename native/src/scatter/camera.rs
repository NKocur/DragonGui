// Based on j:/Projects/DragonSci/src/camera.rs. DragonGUI keeps the same camera
// model but adjusts the initial fit distance for narrow embedded viewports.
//
// CameraState / state() / apply_state() are retained for future Python
// bindings even though they are unused in the M2 demo.
#![allow(dead_code)]

use glam::{Mat4, Vec2, Vec3};

pub struct Camera {
    pub target: Vec3,
    pub distance: f32,
    /// Horizontal rotation in radians
    pub yaw: f32,
    /// Vertical rotation in radians, clamped away from poles
    pub pitch: f32,
    pub fov_y: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
    /// When true, use an orthographic projection instead of perspective.
    pub parallel: bool,
    /// Independent half-extents for orthographic projection.
    /// When both are > 0 AND `parallel` is true, these override the
    /// distance/fov_y/aspect calculation, enabling different X and Y scales.
    /// Reset to 0.0 by `Camera::fit` and `apply_state`.
    pub ortho_half_w: f32,
    pub ortho_half_h: f32,
}

/// Snapshot of camera state returned to / accepted from Python.
#[derive(Clone, Copy)]
pub struct CameraState {
    pub target: [f32; 3],
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub parallel: bool,
}

impl Camera {
    pub fn fit(center: Vec3, radius: f32, aspect: f32) -> Self {
        let fov_y = 45_f32.to_radians();
        let safe_aspect = aspect.max(0.001);
        let vertical_fit = radius / (fov_y * 0.5).tan();
        let horizontal_fit = vertical_fit / safe_aspect;
        let distance = vertical_fit.max(horizontal_fit) * 1.035;
        let far = (distance + radius * 4.0).max(radius * 100.0);
        Self {
            target: center,
            distance,
            yaw: 0.4,
            pitch: 0.4,
            fov_y,
            aspect,
            near: radius * 0.001,
            far,
            parallel: false,
            ortho_half_w: 0.0,
            ortho_half_h: 0.0,
        }
    }

    pub fn fit_preserving_view(&self, center: Vec3, radius: f32, aspect: f32) -> Self {
        let mut next = Self::fit(center, radius, aspect);
        next.yaw = self.yaw;
        next.pitch = self.pitch;
        next.parallel = self.parallel;
        next
    }

    pub fn position(&self) -> Vec3 {
        let (sin_y, cos_y) = self.yaw.sin_cos();
        let (sin_p, cos_p) = self.pitch.sin_cos();
        self.target + Vec3::new(cos_p * sin_y, sin_p, cos_p * cos_y) * self.distance
    }

    pub fn view_matrix(&self) -> Mat4 {
        // For exactly top/front/side views the default up=Y can be degenerate.
        // Pick Z as up when looking straight down/up.
        let up = if self.pitch.abs() > 1.5 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        Mat4::look_at_rh(self.position(), self.target, up)
    }

    pub fn proj_matrix(&self) -> Mat4 {
        if self.parallel {
            let (half_w, half_h) = if self.ortho_half_w > 0.0 && self.ortho_half_h > 0.0 {
                (self.ortho_half_w, self.ortho_half_h)
            } else {
                let h = self.distance * (self.fov_y * 0.5).tan();
                (h * self.aspect, h)
            };
            Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, self.near, self.far)
        } else {
            Mat4::perspective_rh(self.fov_y, self.aspect, self.near, self.far)
        }
    }

    pub fn view_proj(&self) -> Mat4 {
        self.proj_matrix() * self.view_matrix()
    }

    pub fn orbit(&mut self, delta: Vec2) {
        self.yaw += delta.x * 0.008;
        self.pitch = (self.pitch - delta.y * 0.008).clamp(-1.55, 1.55);
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance * (1.0 - delta * 0.12)).max(self.near * 10.0);
    }

    /// Pan in the camera's local XY plane
    pub fn pan(&mut self, delta: Vec2) {
        let view = self.view_matrix();
        let right = Vec3::new(view.x_axis.x, view.x_axis.y, view.x_axis.z);
        let up = Vec3::new(view.y_axis.x, view.y_axis.y, view.y_axis.z);
        let scale = self.distance * 0.001;
        self.target -= right * delta.x * scale;
        self.target += up * delta.y * scale;
    }

    pub fn state(&self) -> CameraState {
        CameraState {
            target: self.target.to_array(),
            distance: self.distance,
            yaw: self.yaw,
            pitch: self.pitch,
            parallel: self.parallel,
        }
    }

    pub fn apply_state(&mut self, s: CameraState) {
        self.target = Vec3::from(s.target);
        self.distance = s.distance.max(self.near * 10.0);
        self.yaw = s.yaw;
        self.pitch = s.pitch.clamp(-1.55, 1.55);
        self.parallel = s.parallel;
        self.ortho_half_w = 0.0;
        self.ortho_half_h = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_increases_distance_for_portrait_viewports() {
        let square = Camera::fit(Vec3::ZERO, 10.0, 1.0);
        let portrait = Camera::fit(Vec3::ZERO, 10.0, 0.5);
        let landscape = Camera::fit(Vec3::ZERO, 10.0, 2.0);
        let very_narrow = Camera::fit(Vec3::ZERO, 10.0, 0.01);

        assert!(portrait.distance > square.distance * 1.9);
        assert!((landscape.distance - square.distance).abs() < 0.001);
        assert!(very_narrow.far > very_narrow.distance);
    }

    #[test]
    fn fit_preserving_view_keeps_projection_and_angles() {
        let mut camera = Camera::fit(Vec3::ZERO, 5.0, 1.0);
        camera.yaw = 1.25;
        camera.pitch = -0.5;
        camera.parallel = true;

        let next = camera.fit_preserving_view(Vec3::new(1.0, 2.0, 3.0), 8.0, 1.6);

        assert_eq!(next.target, Vec3::new(1.0, 2.0, 3.0));
        assert!(next.distance > camera.distance);
        assert_eq!(next.yaw, camera.yaw);
        assert_eq!(next.pitch, camera.pitch);
        assert_eq!(next.parallel, camera.parallel);
        assert_eq!(next.ortho_half_w, 0.0);
        assert_eq!(next.ortho_half_h, 0.0);
    }
}
