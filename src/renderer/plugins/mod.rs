mod assets;
mod camera;
mod misc;

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, RequestRedraw, WindowCloseRequested};
use bevy::winit::WinitWindows;
use color_eyre::eyre::eyre;

use self::assets::{ObjAssetsLoadState, ObjAssetsLoading};

use super::camera::Camera;
use super::model::Model;
use super::render_resources::RenderResources;
use super::Renderer;

pub struct RenderPlugin;
impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            camera::CameraPlugin,
            misc::MiscPlugin,
            assets::AssetsPlugin,
        ))
        .insert_state(RenderResourcesLoadState::NotLoaded)
        .init_resource::<RenderResources>()
        .add_systems(PreStartup, create_renderer)
        .add_systems(
            OnEnter(RenderResourcesLoadState::Loaded),
            init_render_resources,
        )
        .add_systems(
            Update,
            check_all_assets_loaded
                .run_if(in_state(RenderResourcesLoadState::NotLoaded)),
        )
        .add_systems(
            Update,
            draw_frame.run_if(in_state(RenderResourcesLoadState::Loaded)),
        )
        .add_systems(
            PostUpdate,
            cleanup.run_if(in_state(RenderResourcesLoadState::Loaded)),
        );
    }
}

#[derive(States, Debug, Hash, Eq, PartialEq, Clone, Copy)]
enum RenderResourcesLoadState {
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
    mut render_res_state: ResMut<NextState<RenderResourcesLoadState>>,
    obj_assets_state: Res<State<ObjAssetsLoadState>>,
) {
    if *obj_assets_state.get() == ObjAssetsLoadState::Loaded {
        render_res_state.set(RenderResourcesLoadState::Loaded);
    }
}

fn init_render_resources(
    mut commands: Commands,
    renderer: NonSend<Renderer>,
    mut loaded_models: ResMut<Assets<Model>>,
) {
    let mut models = HashMap::new();
    for (name, (handle, load_state)) in loading.0.iter_mut() {
        let model = loaded_models.remove(handle.clone_weak()).unwrap();
        models.insert(name.to_owned(), model);
    }
    let mut resources = RenderResources { models };
    renderer.upload_resources(&mut resources).unwrap();
    commands.insert_resource(resources);
    // ObjAssetsLoading is now empty of all its models
    commands.remove_resource::<ObjAssetsLoading>();
}

fn draw_frame(
    renderer: NonSend<Renderer>,
    camera: Query<&Camera>,
    resources: Res<RenderResources>,
) {
    let camera = camera.single();
    renderer.draw_frame(camera, &resources).unwrap();
}

fn cleanup(
    mut commands: Commands,
    mut window_close_evts: EventReader<WindowCloseRequested>,
    mut renderer: NonSendMut<Renderer>,
    mut resources: ResMut<RenderResources>,
) {
    if window_close_evts.read().next().is_some() {
        renderer.cleanup(&mut resources);
        commands.remove_resource::<RenderResources>();
    }
}