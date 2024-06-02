use std::mem;

use cgmath::{Matrix4, Vector3};

use super::*;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StoneInstance {
    transform: [[f32; 4]; 4],
    color: [f32; 3],
}

impl StoneInstance {
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
    // incremental updates would be nice, eventually
    pub fn make_stone_instances(&self) -> Vec<StoneInstance> {
        let mut instances = Vec::new();
        for point in self.board.points.iter() {
            instances.push(StoneInstance {
                transform: Matrix4::from_translation(Vector3::new(point.pos.x, point.pos.y, 0.0))
                    .into(),
                color: match point.ty {
                    StoneType::Empty => [0.0, 0.8, 0.0],
                    StoneType::Black => [0.0, 0.0, 0.0],
                    StoneType::White => [1.0, 1.0, 1.0],
                },
            })
        }
        instances.last_mut().unwrap().color = [0.8, 0.0, 0.8];
        instances
    }
}
