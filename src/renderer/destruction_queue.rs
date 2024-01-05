use std::{collections::VecDeque, rc::Rc};

pub trait Destroy {
    fn destroy(&self, device: &ash::Device);
}

pub struct DestructionQueue {
    destroyers: VecDeque<Rc<dyn Destroy>>,
}

impl DestructionQueue {
    pub fn new() -> Self {
        Self {
            destroyers: VecDeque::new(),
        }
    }

    pub fn push(&mut self, destroyer: Rc<dyn Destroy>) {
        self.destroyers.push_back(destroyer);
    }

    pub fn flush(&mut self, device: &ash::Device) {
        for destroyer in self.destroyers.drain(..) {
            destroyer.destroy(device);
        }
    }
}