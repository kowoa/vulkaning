use color_eyre::eyre::Result;
use std::{
    process::ExitCode,
    sync::{Arc, Mutex}, time::{Instant, Duration},
};

use crate::renderer::{window::Window, Renderer};

use super::{App, AppType};

// Declare EguiApp as a AppType state in the typestate pattern
impl AppType for EguiApp {}

pub struct EguiApp {
    renderer: Renderer,
    window: Window,
    theme: egui_ash::Theme,
    text: String,
    rotate_y: f32,
    last_frame_time: Instant,
    frame_count: u32,
}

impl EguiApp {
    /// Returns the current fps
    pub fn update_fps(&mut self) -> f64 {
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
impl App<EguiApp> {
    pub fn new() -> Self {
        Self { inner: None }
    }

    pub fn run(self) -> Result<ExitCode> {
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
}


impl egui_ash::App for App<EguiApp> {
    fn ui(&mut self, ctx: &egui::Context) {
        let inner = self.inner.as_mut().unwrap();

        let fps = (inner.update_fps() * 100.0).round() / 100.0;

        let esc_press = ctx.input(|i| i.key_down(egui::Key::Escape));
        if esc_press {
            inner.window.request_close().unwrap();
        }

        egui::Window::new("top left window")
            .id(egui::Id::new("top_left_window"))
            .resizable(false)
            .interactable(false)
            .title_bar(false)
            .show(ctx, |ui| {
                ui.label(format!("FPS: {:.2}", fps));                
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

        match inner.theme {
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
            let inner = self.inner.as_ref().unwrap();
            let renderer = inner.renderer.clone();
            move |size, egui_cmd| {
                let swapchain_image_index = renderer
                    .draw_frame(size.width, size.height, Some(egui_cmd))
                    .unwrap();
                renderer.present_frame(swapchain_image_index).unwrap();
            }
        }))
    }
}

pub struct EguiAppCreator;
impl egui_ash::AppCreator<Arc<Mutex<gpu_allocator::vulkan::Allocator>>>
    for EguiAppCreator
{
    type App = App<EguiApp>;

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
        let renderer = Renderer::new(&window, Some(cc.main_window)).unwrap();
        let ash_render_state = renderer.ash_render_state();
        let inner = EguiApp {
            renderer,
            window,
            theme,
            text: "Hello text!".into(),
            rotate_y: 0.0,
            last_frame_time: Instant::now(),
            frame_count: 0,
        };
        let mut app = Self::App::new();
        app.inner = Some(inner);

        (app, ash_render_state)
    }
}
