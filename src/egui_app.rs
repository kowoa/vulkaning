use color_eyre::eyre::Result;
use std::{
    process::ExitCode,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::renderer::Renderer;

pub fn run() -> Result<ExitCode> {
    let exit_code = egui_ash::run(
        "vulkaning",
        EguiAppCreator,
        egui_ash::RunOption {
            viewport_builder: Some(
                egui::ViewportBuilder::default()
                    .with_title("vulkaning")
                    .with_resizable(false)
                    .with_inner_size((1600.0, 900.0)),
            ),
            follow_system_theme: true,
            default_theme: egui_ash::Theme::Dark,
            present_mode: ash::vk::PresentModeKHR::FIFO,
            ..Default::default()
        },
    );

    Ok(exit_code)
}

pub struct EguiApp {
    renderer: Renderer,

    last_frame_time: Instant,
    frame_count: u32,

    theme: egui_ash::Theme,
    exit_signal: egui_ash::ExitSignal,
}

impl EguiApp {
    /// Returns the current fps
    fn update_fps(&mut self) -> f64 {
        let elapsed = self.last_frame_time.elapsed();
        let fps = (self.frame_count as f64) / elapsed.as_secs_f64();
        if elapsed >= Duration::from_secs(1) {
            self.frame_count = 0;
            self.last_frame_time = Instant::now();
        }
        self.frame_count += 1;
        fps
    }
}

impl egui_ash::App for EguiApp {
    fn ui(&mut self, ctx: &egui::Context) {
        let fps = (self.update_fps() * 100.0).round() / 100.0;

        let esc_press = ctx.input(|i| i.key_down(egui::Key::Escape));
        if esc_press {
            self.exit_signal.send(ExitCode::SUCCESS);
        }

        egui::Window::new("top left window")
            .id(egui::Id::new("top_left_window"))
            .resizable(false)
            .interactable(true)
            .title_bar(false)
            .show(ctx, |ui| {
                ui.label(format!("FPS: {:.2}", fps));

                // Slider for changing background
                ui.label("Background:");
                let mut bg_index = self.renderer.get_background_index();
                let response = ui.add(egui::Slider::new(&mut bg_index, 0..=1));
                if response.changed() {
                    self.renderer.set_background_index(bg_index);
                }
            });
        /*
        egui::SidePanel::left("my_side_panel").show(ctx, |ui| {
            ui.heading("FPS:");
            ui.label(format!("{}", fps));
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Theme");
                let id = ui.make_persistent_id("theme_combo_box_side");
                egui::ComboBox::from_id_source(id)
                    .selected_text(format!("{:?}", inner.theme))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut inner.theme,
                            egui_ash::Theme::Dark,
                            "Dark",
                        );
                        ui.selectable_value(
                            &mut inner.theme,
                            egui_ash::Theme::Light,
                            "Light",
                        );
                    });
            });
            ui.separator();
            ui.hyperlink("https://github.com/emilk/egui");
            ui.separator();
            ui.text_edit_singleline(&mut inner.text);
            ui.separator();
            ui.label("Rotate");
            ui.add(egui::widgets::Slider::new(
                &mut inner.rotate_y,
                -180.0..=180.0,
            ));
        });
        */
        /*
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
                        .selected_text(format!("{:?}", inner.theme))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut inner.theme,
                                egui_ash::Theme::Dark,
                                "Dark",
                            );
                            ui.selectable_value(
                                &mut inner.theme,
                                egui_ash::Theme::Light,
                                "Light",
                            );
                        });
                });
                ui.separator();
                ui.hyperlink("https://github.com/emilk/egui");
                ui.separator();
                ui.text_edit_singleline(&mut inner.text);
                ui.separator();
                ui.label("Rotate");
                ui.add(egui::widgets::Slider::new(
                    &mut inner.rotate_y,
                    -180.0..=180.0,
                ));
            });
        */

        match self.theme {
            egui_ash::Theme::Dark => {
                ctx.set_visuals(egui::style::Visuals::dark())
            }
            egui_ash::Theme::Light => {
                ctx.set_visuals(egui::style::Visuals::light())
            }
        }
    }

    fn handle_event(&mut self, event: egui_ash::event::Event) {
        match event {
            egui_ash::event::Event::AppEvent { event } => match event {
                egui_ash::event::AppEvent::LoopExiting => {
                    bevy::log::info!("Loop exiting ...");
                }
                _ => {}
            },
            _ => {}
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
                    .draw_frame(size.width, size.height) //, Some(egui_cmd))
                    .unwrap();
                renderer.present_frame(swapchain_image_index).unwrap();
            }
        }))
    }
}

impl Drop for EguiApp {
    fn drop(&mut self) {
        self.renderer.cleanup();
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
        let theme = if cc.context.style().visuals.dark_mode {
            egui_ash::Theme::Dark
        } else {
            egui_ash::Theme::Light
        };

        let renderer = Renderer::new(cc.main_window).unwrap();
        let ash_render_state = renderer.ash_render_state();
        let app = EguiApp {
            renderer,
            theme,
            last_frame_time: Instant::now(),
            frame_count: 0,
            exit_signal: cc.exit_signal,
        };

        (app, ash_render_state)
    }
}
