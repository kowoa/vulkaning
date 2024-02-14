use std::sync::{Arc, Mutex};

use crate::renderer::Renderer;


pub struct EguiApp {
    renderer: Renderer,

    theme: egui_ash::Theme,
    text: String,
    rotate_y: f32,
}

impl egui_ash::App for EguiApp {
    fn ui(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("my_side_panel").show(&ctx, |ui| {
            ui.heading("Hello");
            ui.label("Hello egui!");
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Theme");
                let id = ui.make_persistent_id("theme_combo_box_side");
                egui::ComboBox::from_id_source(id)
                    .selected_text(format!("{:?}", self.theme))
            });
        });
    }
}

pub struct EguiAppCreator;
impl egui_ash::AppCreator<Arc<Mutex<gpu_allocator::vulkan::Allocator>>>
    for EguiAppCreator
{
    type App = EguiApp;

    fn create(
        &self,
        cc: egui_ash::CreationContext,
    ) -> (
        Self::App,
        egui_ash::AshRenderState<Arc<Mutex<gpu_allocator::vulkan::Allocator>>>,
    ) {
        todo!()
    }
}
