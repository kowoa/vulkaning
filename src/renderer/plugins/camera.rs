use bevy::{input::mouse::MouseMotion, prelude::*};

use crate::renderer::{camera::Camera, Renderer};

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(Update, rotate_camera);
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera::default());
}

const RADIANS_PER_DELTA: f32 = 1.0 / 180.0;

fn rotate_camera(
    mut camera: Query<&mut Camera>,
    mut mouse_motion_evts: EventReader<MouseMotion>,
) {
    for evt in mouse_motion_evts.read() {
        let mouse_delta = evt.delta;
        let mut camera = camera.single_mut();
        camera.rotate(evt.delta * RADIANS_PER_DELTA);
    }
}
