use std::mem;

use cgmath::{vec2, vec3, InnerSpace, Matrix4, SquareMatrix, Vector3};

use super::*;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    transform: [[f32; 4]; 4],
    color: [f32; 4],
}

impl Instance {
    const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 =>Float32x4];
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

impl<SpinorT: Spinor> GameState<SpinorT> {
    pub fn make_link_instances(&self) -> Vec<Instance> {
        let mut instances = Vec::new();
        // TODO need to squeeze into some sort of trapezoid
        let stretch_mat = Matrix4::from_nonuniform_scale(self.board.link_len, 1.0, 1.0);
        for (idx1, idx2) in self.board.links.iter() {
            let tf1 = self.board.points[*idx1 as usize].transform;
            let rel_pos2 = tf1.reverse().apply(self.board.points[*idx2 as usize].pos);
            let angle = -rel_pos2.angle();

            instances.push(Instance {
                transform: ((tf1 * SpinorT::rotation(angle)).into_mat4() * stretch_mat).into(),
                color: [0.1, 0.1, 0.1, 1.0],
            });
        }
        instances
    }

    // incremental updates would be nice, eventually
    pub fn make_stone_instances(&self) -> Vec<Instance> {
        let mut instances = Vec::new();
        for point in self.board.points.iter() {
            if point.ty == StoneType::Empty {
                //continue;
            }

            // TODO
            let scale_mat = Matrix4::from_scale(0.5);

            instances.push(Instance {
                transform: (point.transform.into_mat4() * scale_mat).into(),
                color: match point.ty {
                    StoneType::Empty => [0.0, 0.2, 0.0, 0.2],
                    StoneType::Black => [0.0, 0.0, 0.0, 1.0],
                    StoneType::White => [1.0, 1.0, 1.0, 1.0],
                },
            });
            //println!("transform ")
        }
        instances
    }
}
