use std::{iter, mem};

use cgmath::{vec2, vec3, InnerSpace, Matrix4, SquareMatrix, Vector3};
use log::info;

use super::*;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    position: [f32; 3],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

const SQRT2: f64 = 1.4142135623730951;
const STONE_VERTS: &[Vector2<f64>] = &[
    vec2(-STONE_RADIUS / SQRT2, STONE_RADIUS / SQRT2),
    vec2(-STONE_RADIUS, 0.0),
    vec2(-STONE_RADIUS / SQRT2, -STONE_RADIUS / SQRT2),
    vec2(0.0, -STONE_RADIUS),
    vec2(STONE_RADIUS / SQRT2, -STONE_RADIUS / SQRT2),
    vec2(STONE_RADIUS, 0.0),
    vec2(STONE_RADIUS / SQRT2, STONE_RADIUS / SQRT2),
    vec2(0.0, STONE_RADIUS),
];

const STONE_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 5, 0, 5, 6, 0, 6, 7];

const LINK_INDICES: &[u16] = &[0, 2, 1, 1, 2, 3];

#[derive(Debug)]
pub struct Model {
    pub verts: Vec<Vertex>,
    pub indices: Vec<u16>,
}

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

fn make_link_verts<SpinorT: Spinor>(link_len: f64) -> Vec<SpinorT::Point> {
    const LINK_WIDTH: f64 = 0.025;
    let t = SpinorT::translation_to(SpinorT::Point::from_flat(link_len, 0.0));

    let b1 = SpinorT::Point::from_flat(-LINK_WIDTH / 2.0, -LINK_WIDTH / 2.0);
    let b2 = SpinorT::Point::from_flat(-LINK_WIDTH / 2.0, LINK_WIDTH / 2.0);
    let b3 = t.apply(SpinorT::Point::from_flat(
        LINK_WIDTH / 2.0,
        -LINK_WIDTH / 2.0,
    ));
    let b4 = t.apply(SpinorT::Point::from_flat(
        LINK_WIDTH / 2.0,
        LINK_WIDTH / 2.0,
    ));

    vec![b1, b2, b3, b4]
}

pub fn make_models<SpinorT: Spinor>(link_len: f64) -> Vec<Model> {
    iter::once((
        STONE_VERTS
            .iter()
            .map(|&v| SpinorT::Point::from_flat_vec(v))
            .collect(),
        &STONE_INDICES,
    ))
    .chain(iter::once((
        make_link_verts::<SpinorT>(link_len),
        &LINK_INDICES,
    )))
    .map(|t| Model {
        verts: t
            .0
            .iter()
            .map(|&p| Vertex {
                position: p.to_projective().into(),
            })
            .collect(),
        indices: t.1.to_vec(),
    })
    .collect()
}

// potential optimizations, since these are going to be called more
// - don't allocate every call
// - skip items out of viewable range
const TEST_TRANS: f64 = 0.0;
impl<SpinorT: Spinor> GameState<SpinorT> {
    pub fn make_link_instances(&self) -> Vec<Instance> {
        let test_trans = SpinorT::translation(TEST_TRANS, 0.0);
        let mut instances = Vec::new();

        for (idx1, idx2) in self.board.links.iter() {
            //let tf1 = self.board.points[*idx1 as usize].transform;
            //let rel_pos2 = tf1.reverse().apply(self.board.points[*idx2 as usize].pos);
            let tf1 = self.board.points[*idx1 as usize].relative_transform;
            let rel_pos2 = (tf1.reverse() * self.board.points[*idx2 as usize].relative_transform)
                .apply(SpinorT::Point::zero());
            let angle = -rel_pos2.angle();

            instances.push(Instance {
                transform: (test_trans * tf1 * SpinorT::rotation(angle))
                    .into_mat4()
                    .into(),
                color: [0.1, 0.1, 0.1, 1.0],
            });
        }
        instances
    }

    pub fn make_stone_instances(&self) -> Vec<Instance> {
        let mut instances = Vec::new();

        let scale_mat = Matrix4::from_scale(self.board.tiling_parameters.stone_scale as f32);

        let test_trans = SpinorT::translation(TEST_TRANS, 0.0);

        for point in self.board.points.iter() {
            if point.ty == StoneType::Empty {
                continue;
            }

            instances.push(Instance {
                transform: ((test_trans * point.relative_transform).into_mat4() * scale_mat).into(),
                color: match point.ty {
                    StoneType::Empty => [0.0, 0.2, 0.0, 0.2],
                    StoneType::Black => [0.0, 0.0, 0.0, 1.0],
                    StoneType::White => [1.0, 1.0, 1.0, 1.0],
                },
            });
            /*             if point.pos.distance(SpinorT::Point::zero()) > 10.1 {
                info!("transform {:?}", instances.last().unwrap().transform);
            } */
        }

        if let Some(score) = &self.score {
            let territory_scale_mat =
                Matrix4::from_scale(0.3 * self.board.tiling_parameters.stone_scale as f32);

            for (i, point) in self.board.points.iter().enumerate() {
                if point.ty == StoneType::Empty {
                    if score.territory[i] == StoneType::Empty {
                        continue;
                    }

                    instances.push(Instance {
                        transform: ((test_trans * point.relative_transform).into_mat4()
                            * territory_scale_mat)
                            .into(),
                        color: match score.territory[i] {
                            StoneType::Empty => [0.0, 0.2, 0.0, 0.2],
                            StoneType::Black => [0.0, 0.0, 0.0, 0.5],
                            StoneType::White => [0.35, 0.35, 0.35, 0.4],
                        },
                    });
                }
            }
        }

        if self.hover_idx >= 0 {
            let hover_point = &self.board.points[self.hover_idx as usize];
            if let StoneType::Empty = hover_point.ty {
                instances.push(Instance {
                    transform: ((test_trans * hover_point.relative_transform).into_mat4()
                        * scale_mat)
                        .into(),
                    color: match self.turn {
                        Turn::Black => [0.0, 0.0, 0.0, 0.5],
                        Turn::White => [0.35, 0.35, 0.35, 0.4],
                    },
                });
            }
        }
        instances
    }
}
