use glam::{Mat4, Vec3};

#[derive(Copy, Clone)]
#[repr(C)]
pub struct GpuCameraData {
    pub viewproj: Mat4,
}

pub struct Camera {
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    pub right: Vec3,
    pub world_up: Vec3,
    pub zoom_deg: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            forward: Vec3::NEG_Z,
            up: Vec3::Y,
            right: Vec3::X,
            world_up: Vec3::Y,
            zoom_deg: Self::DEFAULT_ZOOM_DEG,
        }
    }
}

impl Camera {
    const DEFAULT_ZOOM_DEG: f32 = 45.0;

    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }

    pub fn look_at(&mut self, target: Vec3) {
        if target == self.position {
            return;
        }
        self.look_to(target - self.position);
    }

    pub fn look_to(&mut self, direction: Vec3) {
        self.forward = direction.normalize();
        self.right = self.forward.cross(self.world_up).normalize();
        self.up = self.right.cross(self.forward).normalize();
    }

    pub fn viewproj_mat(
        &self,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Mat4 {
        self.proj_mat(viewport_width, viewport_height) * self.view_mat()
    }

    fn view_mat(&self) -> Mat4 {
        Mat4::look_to_rh(self.position, self.forward, self.up)
    }

    fn proj_mat(&self, viewport_width: f32, viewport_height: f32) -> Mat4 {
        let mut proj = Mat4::perspective_rh(
            self.zoom_deg.to_radians(),
            viewport_width / viewport_height,
            0.1,
            100.0,
        );
        proj.y_axis.y *= -1.0;
        proj
    }
}
