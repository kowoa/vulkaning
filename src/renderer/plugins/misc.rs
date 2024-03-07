use bevy::{
    prelude::*,
    window::{PrimaryWindow, WindowCloseRequested},
};

use crate::renderer::Renderer;

// Uncategorized plugin containing miscellaneous systems
pub struct MiscPlugin;
impl Plugin for MiscPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, request_close_on_esc);
    }
}

fn request_close_on_esc(
    windows: Query<Entity, With<PrimaryWindow>>,
    mut window_close_evts: EventWriter<WindowCloseRequested>,
    input: Res<ButtonInput<KeyCode>>,
) {
    if input.just_released(KeyCode::Escape) {
        window_close_evts.send(WindowCloseRequested {
            window: windows.single(),
        });
    }
}
