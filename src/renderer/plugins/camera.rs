use bevy::prelude::*;

use crate::renderer::{camera::Camera, Renderer};

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera);
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera::default());
}
