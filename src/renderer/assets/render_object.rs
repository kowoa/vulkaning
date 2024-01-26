use std::rc::Rc;

use glam::Mat4;

use super::{model::Model, pipeline::Pipeline};

pub struct RenderObject {
    pub model: Rc<Model>,
    pub pipeline: Rc<Pipeline>,
    pub transform: Mat4,
}

impl RenderObject {
    pub fn new(
        model: Rc<Model>,
        pipeline: Rc<Pipeline>,
        transform: Mat4,
    ) -> Self {
        Self {
            model,
            pipeline,
            transform,
        }
    }
}
