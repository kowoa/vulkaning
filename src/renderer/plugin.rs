use bevy::log;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::winit::WinitWindows;

use crate::egui_app;

pub struct RenderPlugin;
impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        //app.add_systems(Startup, set_up_winit);
        app.add_systems(Startup, start_app);
    }
}

fn set_up_winit(
    mut winit_windows: NonSendMut<WinitWindows>,
    mut window_ents: Query<Entity, With<PrimaryWindow>>,
) {
    let window_ent = window_ents.single();
    let winit_window = winit_windows.get_window(window_ent).unwrap();
    //winit_window.set_fullscreen(Some(Fullscreen::Borderless(None)));
    log::info!("did not panic!");
    /*
        for (entity, mut window) in &mut windows {
            let winit_window = winit_windows.get_window(entity).unwrap();
        }
    */
}

fn start_app() {
    let _ = egui_app::run().unwrap();
}
