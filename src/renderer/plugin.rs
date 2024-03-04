use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowCloseRequested};
use bevy::winit::WinitWindows;
use bevy_egui::{egui, EguiContexts, EguiRenderOutput};
use color_eyre::eyre::eyre;

use super::Renderer;

pub struct RenderPlugin;
impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, start_renderer)
            .add_systems(Update, request_close_on_esc)
            //.add_systems(Update, ui_example_system)
            .add_systems(Update, draw_frame)
            .add_systems(PostUpdate, cleanup);
    }
}

fn ui_example_system(mut contexts: EguiContexts) {
    egui::Window::new("Hello").show(contexts.ctx_mut(), |ui| {
        ui.label("Hello World!");
    });
}

fn start_renderer(world: &mut World) {
    let mut window_ents = world.query_filtered::<Entity, With<PrimaryWindow>>();
    let winit_windows = world.get_non_send_resource::<WinitWindows>().unwrap();
    let window_ent = window_ents.single(world);
    let winit_window = winit_windows.get_window(window_ent).unwrap();

    let renderer = Renderer::new(winit_window).unwrap();
    world.insert_non_send_resource(renderer);
}

fn draw_frame(
    windows: Query<&Window, With<PrimaryWindow>>,
    renderer: NonSendMut<Renderer>,
    mut contexts: EguiContexts,
    egui_output: Query<&EguiRenderOutput>,
) {
    let window = windows.single();
    let swapchain_image_index = renderer
        .draw_frame(
            window.width() as u32,
            window.height() as u32,
            contexts.ctx_mut(),
            egui_output.single(),
        )
        .unwrap();
    renderer.present_frame(swapchain_image_index).unwrap();
}

fn request_close_on_esc(
    windows: Query<Entity, With<PrimaryWindow>>,
    mut window_close_evts: EventWriter<WindowCloseRequested>,
    input: Res<ButtonInput<KeyCode>>,
) {
    if input.pressed(KeyCode::Escape) {
        window_close_evts.send(WindowCloseRequested {
            window: windows.single(),
        });
    }
}

fn cleanup(
    mut window_close_evts: EventReader<WindowCloseRequested>,
    mut renderer: NonSendMut<Renderer>,
) {
    for _evt in window_close_evts.read() {
        renderer.cleanup();
    }
}
