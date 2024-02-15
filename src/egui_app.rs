use std::sync::{Arc, Mutex};

use crate::renderer::{window::Window, Renderer};

pub struct EguiApp {
    renderer: Renderer,
    window: Window,

    theme: egui_ash::Theme,
    text: String,
    rotate_y: f32,
}

impl egui_ash::App for EguiApp {
    fn ui(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("my_side_panel").show(ctx, |ui| {
            ui.heading("Hello");
            ui.label("Hello egui!");
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Theme");
                let id = ui.make_persistent_id("theme_combo_box_side");
                egui::ComboBox::from_id_source(id)
                    .selected_text(format!("{:?}", self.theme))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.theme,
                            egui_ash::Theme::Dark,
                            "Dark",
                        );
                        ui.selectable_value(
                            &mut self.theme,
                            egui_ash::Theme::Light,
                            "Light",
                        );
                    });
            });
            ui.separator();
            ui.hyperlink("https://github.com/emilk/egui");
            ui.separator();
            ui.text_edit_singleline(&mut self.text);
            ui.separator();
            ui.label("Rotate");
            ui.add(egui::widgets::Slider::new(
                &mut self.rotate_y,
                -180.0..=180.0,
            ));
        });
        egui::Window::new("My Window")
            .id(egui::Id::new("my_window"))
            .resizable(true)
            .scroll2([true, true])
            .show(ctx, |ui| {
                ui.heading("Hello");
                ui.label("Hello egui!");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Theme");
                    let id = ui.make_persistent_id("theme_combo_box_window");
                    egui::ComboBox::from_id_source(id)
                        .selected_text(format!("{:?}", self.theme))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.theme,
                                egui_ash::Theme::Dark,
                                "Dark",
                            );
                            ui.selectable_value(
                                &mut self.theme,
                                egui_ash::Theme::Light,
                                "Light",
                            );
                        });
                });
                ui.separator();
                ui.hyperlink("https://github.com/emilk/egui");
                ui.separator();
                ui.text_edit_singleline(&mut self.text);
                ui.separator();
                ui.label("Rotate");
                ui.add(egui::widgets::Slider::new(
                    &mut self.rotate_y,
                    -180.0..=180.0,
                ));
            });

        match self.theme {
            egui_ash::Theme::Dark => {
                ctx.set_visuals(egui::style::Visuals::dark())
            }
            egui_ash::Theme::Light => {
                ctx.set_visuals(egui::style::Visuals::light())
            }
        }
    }

    fn request_redraw(
        &mut self,
        _viewport_id: egui::ViewportId,
    ) -> egui_ash::HandleRedraw {
        egui_ash::HandleRedraw::Handle(Box::new({
            let renderer = self.renderer.clone();
            move |size, egui_cmd| {
                let swapchain_image_index = renderer
                    .draw_frame(size.width, size.height, egui_cmd)
                    .unwrap();
                renderer.present_frame(swapchain_image_index).unwrap();
            }
        }))
    }
}

pub struct EguiAppCreator;
impl<'a> egui_ash::AppCreator<Arc<Mutex<gpu_allocator::vulkan::Allocator>>>
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
        let theme = if cc.context.style().visuals.dark_mode {
            egui_ash::Theme::Dark
        } else {
            egui_ash::Theme::Light
        };
        let window: Window = Window::new_with_egui(&cc);
        let renderer = Renderer::new(&window).unwrap();
        let app = Self::App {
            renderer,
            window,
            theme,
            text: "Hello text!".into(),
            rotate_y: 0.0,
        };

        let ash_render_state = egui_ash::AshRenderState {
            entry: todo!(),
            instance: todo!(),
            physical_device: todo!(),
            device: todo!(),
            surface_loader: todo!(),
            swapchain_loader: todo!(),
            queue: todo!(),
            queue_family_index: todo!(),
            command_pool: todo!(),
            allocator: todo!(),
        };

        (app, ash_render_state)
    }
}
