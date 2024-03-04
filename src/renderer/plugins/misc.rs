use bevy::prelude::*;

use crate::renderer::Renderer;

// Uncategorized plugin containing miscellaneous systems
pub struct MiscPlugin;
impl Plugin for MiscPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, change_background_on_space);
    }
}

fn change_background_on_space(
    input: Res<ButtonInput<KeyCode>>,
    mut renderer: NonSendMut<Renderer>,
) {
    if input.just_released(KeyCode::Space) {
        let i = renderer.get_background_index();
        let i = if i == 0 { 1 } else { 0 };
        renderer.set_background_index(i);
    }
}
