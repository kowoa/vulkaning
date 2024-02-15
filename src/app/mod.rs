use crate::renderer::{window::Window, Renderer};
use color_eyre::eyre::Result;

pub mod egui_app;
pub mod winit_app;

// AppType is the state in the typestate pattern
pub trait AppType {}

pub struct App<T: AppType> {
    inner: Option<T>,
}
