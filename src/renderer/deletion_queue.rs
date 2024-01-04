use std::collections::VecDeque;

pub struct DeletionQueue {
    deletors: VecDeque<fn()>,
}

impl DeletionQueue {
    pub fn new() -> Self {
        Self {
            deletors: VecDeque::new(),
        }
    }

    pub fn push(&mut self, deletor: fn()) {
        self.deletors.push_back(deletor);
    }

    pub fn flush(&mut self) {
        for deleter in self.deletors.drain(..) {
            deleter();
        }
    }
}