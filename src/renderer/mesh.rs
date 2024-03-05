use bevy::log;
use std::sync::atomic::{AtomicUsize, Ordering};

use bytemuck::{Pod, Zeroable};

use glam::{Mat4, Vec4};

use super::vertex::Vertex;

#[derive(Pod, Zeroable, Copy, Clone, Debug)]
#[repr(C)]
pub struct MeshPushConstants {
    pub data: Vec4,
    pub render_matrix: Mat4,
}

static MESH_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
pub struct Mesh {
    pub id: usize,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl PartialEq for Mesh {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        let id = MESH_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
        Self {
            id,
            vertices,
            indices,
        }
    }

    pub fn new_triangle() -> Self {
        let vertices = vec![
            Vertex {
                position: [-0.5, -0.5, 0.0].into(),
                normal: [0.0, 0.0, 1.0].into(),
                color: [1.0, 0.0, 0.0].into(),
                texcoord: [0.0, 0.0].into(),
            },
            Vertex {
                position: [0.5, -0.5, 0.0].into(),
                normal: [0.0, 0.0, 1.0].into(),
                color: [0.0, 1.0, 0.0].into(),
                texcoord: [0.5, 1.0].into(),
            },
            Vertex {
                position: [0.0, 0.5, 0.0].into(),
                normal: [0.0, 0.0, 1.0].into(),
                color: [0.0, 0.0, 1.0].into(),
                texcoord: [1.0, 0.0].into(),
            },
        ];

        let indices = vec![0, 1, 2];

        Self::new(vertices, indices)
    }

    pub fn new_quad() -> Self {
        // Clockwise winding order
        let vertices = vec![
            // Top left triangle
            Vertex {
                position: [1.0, 1.0, 0.0].into(),
                normal: [0.0, 1.0, 0.0].into(),
                color: [1.0, 0.0, 0.0].into(),
                texcoord: [0.0, 0.0].into(),
            },
            Vertex {
                position: [-1.0, -1.0, 0.0].into(),
                normal: [0.0, 1.0, 0.0].into(),
                color: [0.0, 1.0, 0.0].into(),
                texcoord: [1.0, 0.0].into(),
            },
            Vertex {
                position: [-1.0, 1.0, 0.0].into(),
                normal: [0.0, 1.0, 0.0].into(),
                color: [0.0, 0.0, 1.0].into(),
                texcoord: [0.0, 1.0].into(),
            },
            // Bottom right triangle
            Vertex {
                position: [-1.0, -1.0, 0.0].into(),
                normal: [0.0, 1.0, 0.0].into(),
                color: [0.0, 1.0, 0.0].into(),
                texcoord: [1.0, 0.0].into(),
            },
            Vertex {
                position: [1.0, 1.0, 0.0].into(),
                normal: [0.0, 1.0, 0.0].into(),
                color: [1.0, 0.0, 1.0].into(),
                texcoord: [1.0, 1.0].into(),
            },
            Vertex {
                position: [1.0, -1.0, 0.0].into(),
                normal: [0.0, 1.0, 0.0].into(),
                color: [0.0, 0.0, 1.0].into(),
                texcoord: [0.0, 1.0].into(),
            },
        ];

        let indices = vec![
            0, 1, 2, // Top left triangle
            3, 4, 5, // Bottom right triangle
        ];

        Self::new(vertices, indices)
    }
}
