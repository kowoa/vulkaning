mod assets;
mod camera;
mod misc;

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowCloseRequested};
use bevy::winit::WinitWindows;

use self::assets::{ImageAssetsLoadState, ObjAssetsLoadState};

use super::camera::Camera;
use super::{AssetData, Renderer};

pub struct RenderPlugin;
impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            camera::CameraPlugin,
            misc::MiscPlugin,
            assets::AssetsPlugin,
        ))
        .insert_state(AllAssetsLoadState::NotLoaded)
        .init_resource::<AssetData>()
        .add_systems(PreStartup, create_renderer)
        .add_systems(OnEnter(AllAssetsLoadState::Loaded), init_render_resources)
        .add_systems(
            Update,
            check_all_assets_loaded
                .run_if(in_state(AllAssetsLoadState::NotLoaded)),
        )
        .add_systems(
            Update,
            draw_frame.run_if(in_state(AllAssetsLoadState::Loaded)),
        )
        .add_systems(
            PostUpdate,
            cleanup.run_if(in_state(AllAssetsLoadState::Loaded)),
        );
    }
}

#[derive(States, Debug, Hash, Eq, PartialEq, Clone, Copy)]
enum AllAssetsLoadState {
    NotLoaded,
    Loaded,
}

fn create_renderer(world: &mut World) {
    let mut window_ents = world.query_filtered::<Entity, With<PrimaryWindow>>();
    let winit_windows = world.get_non_send_resource::<WinitWindows>().unwrap();
    let window_ent = window_ents.single(world);
    let winit_window = winit_windows.get_window(window_ent).unwrap();

    let renderer = Renderer::new(winit_window).unwrap();
    world.insert_non_send_resource(renderer);
}

fn check_all_assets_loaded(
    mut all_assets_state: ResMut<NextState<AllAssetsLoadState>>,
    obj_assets_state: Res<State<ObjAssetsLoadState>>,
    image_assets_state: Res<State<ImageAssetsLoadState>>,
) {
    if *obj_assets_state.get() == ObjAssetsLoadState::Loaded
        && *image_assets_state.get() == ImageAssetsLoadState::Loaded
    {
        all_assets_state.set(AllAssetsLoadState::Loaded);
    }
}

fn init_render_resources(
    mut commands: Commands,
    renderer: NonSend<Renderer>,
    mut asset_data: ResMut<AssetData>,
) {
    renderer.init_resources(&mut asset_data).unwrap();
    commands.remove_resource::<AssetData>();
}

fn draw_frame(renderer: NonSend<Renderer>, camera: Query<&Camera>) {
    let camera = camera.single();
    renderer.draw_frame(camera).unwrap();
}

fn cleanup(
    mut window_close_evts: EventReader<WindowCloseRequested>,
    mut renderer: NonSendMut<Renderer>,
) {
    if window_close_evts.read().next().is_some() {
        renderer.cleanup();
    }
}
