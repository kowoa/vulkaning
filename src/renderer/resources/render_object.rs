use std::sync::Arc;

use glam::Mat4;

use super::{model::Model, pipeline::Pipeline};

pub struct RenderObject {
    pub model: Arc<Model>,
    pub pipeline: Arc<Pipeline>,
    pub transform: Mat4,
}

impl RenderObject {
    pub fn new(
        model: Arc<Model>,
        pipeline: Arc<Pipeline>,
        transform: Mat4,
    ) -> Self {
        Self {
            model,
            pipeline,
            transform,
        }
    }
}
