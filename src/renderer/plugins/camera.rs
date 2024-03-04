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

fn rotate_camera(
    mut camera: Query<&mut Camera>,
    mut cursor_moved_evts: EventReader<CursorMoved>,
    mut window: Query<&Window>,
) {
    for evt in cursor_moved_evts.read() {
        let window = window.get(evt.window).unwrap();
        let viewport_width = window.width();
        let viewport_height = window.height();
        let last_mouse_pos = if let Some(delta) = evt.delta {
            evt.position - delta
        } else {
            evt.position
        };
        let curr_mouse_pos = evt.position;
        let mut camera = camera.single_mut();
        camera.rotate(
            last_mouse_pos,
            curr_mouse_pos,
            viewport_width,
            viewport_height,
        );
    }
}
