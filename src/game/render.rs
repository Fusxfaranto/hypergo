use std::mem;

use cgmath::{Matrix4, SquareMatrix, Vector3};

use super::*;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    transform: [[f32; 4]; 4],
    color: [f32; 3],
}

impl Instance {
    const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 =>Float32x3];
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

impl GameState {
    pub fn make_link_instances(&self) -> Vec<Instance> {
        let mut instances = Vec::new();
        for (idx1, idx2) in self.board.links.iter() {
            let pos1 = self.board.points[*idx1 as usize].pos;
            let pos2 = self.board.points[*idx2 as usize].pos;
            let dist = pos1.distance(pos2);
            let dir = (pos2 - pos1) / dist;

            let mut rotate_mat = Matrix4::identity();
            rotate_mat.x.x = dir.y;
            rotate_mat.x.y = -dir.x;
            rotate_mat.y.x = dir.x;
            rotate_mat.y.y = dir.y;

            instances.push(Instance {
                transform: (Matrix4::from_translation(Vector3::new(pos1.x, pos1.y, 0.0))
                    * rotate_mat)
                    .into(),
                color: [0.1, 0.1, 0.1],
            });
        }
        instances
    }

    // incremental updates would be nice, eventually
    pub fn make_stone_instances(&self) -> Vec<Instance> {
        let mut instances = Vec::new();
        for point in self.board.points.iter() {
            instances.push(Instance {
                transform: Matrix4::from_translation(Vector3::new(point.pos.x, point.pos.y, 0.0))
                    .into(),
                color: match point.ty {
                    StoneType::Empty => [0.0, 0.8, 0.0],
                    StoneType::Black => [0.0, 0.0, 0.0],
                    StoneType::White => [1.0, 1.0, 1.0],
                },
            })
        }
        instances
    }
}
