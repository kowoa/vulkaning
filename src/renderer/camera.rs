use std::f32::consts::PI;

use bevy::{ecs::component::Component, log};
use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};

#[derive(Copy, Clone)]
#[repr(C)]
pub struct GpuCameraData {
    pub viewproj: Mat4,
    pub near: f32,
    pub far: f32,
}

#[derive(Component)]
pub struct Camera {
    position: Vec3,
    forward: Vec3,
    up: Vec3,
    right: Vec3,
    world_up: Vec3,
    fov_y_deg: f32,
    pub near: f32,
    pub far: f32,
    pivot: Vec3,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 5.0),
            forward: Vec3::NEG_Z,
            up: Vec3::Y,
            right: Vec3::X,
            world_up: Vec3::Y,
            fov_y_deg: Self::DEFAULT_FOV_Y_DEG,
            near: 0.1,
            far: 100.0,
            pivot: Vec3::ZERO,
        }
    }
}

impl Camera {
    const DEFAULT_FOV_Y_DEG: f32 = 45.0;

    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
        self.look_at(self.pivot);
    }

    pub fn look_at(&mut self, target: Vec3) {
        if target == self.position {
            return;
        }
        self.pivot = target;
        self.forward = (target - self.position).normalize();
        self.right = self.forward.cross(self.world_up).normalize();
        self.up = self.right.cross(self.forward).normalize();
    }

    pub fn zoom(&mut self, delta: f32) {
        // Subtracting because lower FOV means zooming in
        self.fov_y_deg = (self.fov_y_deg - delta).clamp(1.0, 179.0);
    }

    pub fn rotate(
        &mut self,
        last_mouse_pos: Vec2,
        curr_mouse_pos: Vec2,
        viewport_width: f32,
        viewport_height: f32,
    ) {
        // Get the homogeneous positions of the camera eye and pivot
        let pos =
            Vec4::new(self.position.x, self.position.y, self.position.z, 1.0);
        let piv = Vec4::new(self.pivot.x, self.pivot.y, self.pivot.z, 1.0);

        // Calculate the amount of rotation given the mouse movement
        let delta_angle_x = 2.0 * PI / viewport_width; // Left to right = 2*PI = 360deg
        let delta_angle_y = PI / viewport_height; // Top to bottom = PI = 180deg
        let angle_x = (last_mouse_pos.x - curr_mouse_pos.x) * delta_angle_x;
        let angle_y = (last_mouse_pos.y - curr_mouse_pos.y) * delta_angle_y;

        // Handle case where the camera's forward is the same as its up
        let cos_angle = self.forward.dot(self.up);
        let delta_angle_y = if cos_angle * delta_angle_y.signum() > 0.99 {
            0.0
        } else {
            delta_angle_y
        };

        // Rotate the camera around the pivot point on the up axis
        let rot_x = Mat4::from_axis_angle(self.up, angle_x);
        let pos = (rot_x * (pos - piv)) + piv;

        // Rotate the camera around the pivot point on the right axis
        let rot_y = Mat4::from_axis_angle(self.right, angle_y);
        let pos = (rot_y * (pos - piv)) + piv;

        self.set_position(pos.xyz());
    }

    pub fn viewproj_mat(
        &self,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Mat4 {
        self.proj_mat(viewport_width, viewport_height) * self.view_mat()
    }

    pub fn view_mat(&self) -> Mat4 {
        Mat4::look_to_rh(self.position, self.forward, self.up)
    }

    pub fn proj_mat(&self, viewport_width: f32, viewport_height: f32) -> Mat4 {
        let mut proj = Mat4::perspective_rh(
            self.fov_y_deg.to_radians(),
            viewport_width / viewport_height,
            self.near,
            self.far,
        );
        proj.y_axis.y *= -1.0;
        proj
    }
}
