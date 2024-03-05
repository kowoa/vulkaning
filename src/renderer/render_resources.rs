use std::collections::HashMap;

use super::model::Model;

pub struct RenderResources<'a> {
    pub models: HashMap<String, &'a mut Model>,
}
