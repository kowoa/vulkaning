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
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    pub right: Vec3,
    pub world_up: Vec3,
    pub zoom_deg: f32,
    pub near: f32,
    pub far: f32,
    pub pivot: Vec3,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 5.0),
            forward: Vec3::NEG_Z,
            up: Vec3::Y,
            right: Vec3::X,
            world_up: Vec3::Y,
            zoom_deg: Self::DEFAULT_ZOOM_DEG,
            near: 0.1,
            far: 100.0,
            pivot: Vec3::ZERO,
        }
    }
}

impl Camera {
    const DEFAULT_ZOOM_DEG: f32 = 45.0;

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

    pub fn rotate(&mut self, delta_radians: Vec2) {
        // Get the homogeneous positions of the camera eye and pivot
        let pos =
            Vec4::new(self.position.x, self.position.y, self.position.z, 1.0);
        let piv = Vec4::new(self.pivot.x, self.pivot.y, self.pivot.z, 1.0);

        // Rotate the camera around the pivot point on the up axis
        let rot_x = Mat4::from_axis_angle(self.up, delta_radians.x);
        let pos = (rot_x * (pos - piv)) + piv;

        // Rotate the camera around the pivot point on the right axis
        let rot_y = Mat4::from_axis_angle(self.right, delta_radians.y);
        let pos = (rot_y * (pos - piv)) + piv;

        self.position = pos.xyz();
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
            self.zoom_deg.to_radians(),
            viewport_width / viewport_height,
            self.near,
            self.far,
        );
        proj.y_axis.y *= -1.0;
        proj
    }
}
