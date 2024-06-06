use std::ops;

use cgmath::InnerSpace;
use cgmath::{num_traits::AsPrimitive, vec2, AbsDiffEq, BaseFloat, Matrix4, One, Vector2};
use wgpu::SurfaceConfiguration;

pub mod euclidian;
pub mod hyperbolic;

pub trait Spinor: Copy + Clone + ops::Mul<Output = Self> + One + AbsDiffEq {
    fn translation(amt: f64, angle: f64) -> Self;
    fn translation_to(v: Vector2<f64>) -> Self;
    fn rotation(angle: f64) -> Self;
    fn reverse(&self) -> Self;
    fn apply(&self, v: Vector2<f64>) -> Vector2<f64>;
    fn into_mat4<S: 'static + BaseFloat>(&self) -> Matrix4<S>
    where
        f32: AsPrimitive<S>,
        f64: AsPrimitive<S>;

    // these aren't really tied to spinors, but they're tied to the geometry,
    // so here they sit for now
    fn distance(a: Vector2<f64>, b: Vector2<f64>) -> f64;
    fn tiling_neighbor_directions() -> Vec<Vec<Self>>;
}

pub struct ViewState<SpinorT: Spinor> {
    // TODO can we scale with unnormalized spinor instead of keeping separate scale?
    scale: f64,
    camera: SpinorT,
    pending_camera: SpinorT,
}

impl<SpinorT: Spinor> ViewState<SpinorT> {
    pub fn new() -> Self {
        let scale = 0.8;

        Self {
            scale,
            camera: SpinorT::one(),
            pending_camera: SpinorT::one(),
        }
    }

    pub fn pixel_to_world_coords(
        &self,
        config: &SurfaceConfiguration,
        x: f64,
        y: f64,
    ) -> Vector2<f64> {
        let scaled = (1.0 / self.scale)
            * vec2(
                2.0 * x / config.width as f64 - 1.0,
                -2.0 * y / config.height as f64 + 1.0,
            );

        // TODO hack
        #[cfg(feature = "euclidian_geometry")]
        fn limit(v: Vector2<f64>) -> Vector2<f64> {
            v
        }

        #[cfg(not(feature = "euclidian_geometry"))]
        fn limit(v: Vector2<f64>) -> Vector2<f64> {
            const LIMIT: f64 = 0.8;
            let mag2 = v.magnitude2();
            //println!("{mag2}");
            if mag2 < LIMIT {
                v
            } else {
                v * (LIMIT / mag2).sqrt()
            }
        }

        self.camera.apply(limit(scaled))
    }

    // TODO ???????
    /*
    fn pixel_delta_to_world(&self, config: &SurfaceConfiguration, x: f64, y: f64) -> Vector2<f32> {
        let mut translationless_camera_inverse = Matrix4::identity();
        translationless_camera_inverse.w.w = self.scale;
        let v = translationless_camera_inverse
            * vec4(
                1.0 * x as f32 / config.width as f32,
                -1.0 * y as f32 / config.height as f32,
                0.0,
                1.0,
            );
        vec2(v.x, v.y) / v.w * 2.1
    } */

    pub fn adjust_scale(&mut self, amt: f64) {
        self.scale *= amt + 1.0;
    }

    pub fn translate(&mut self, amt: f64, angle: f64) {
        self.camera = self.camera * SpinorT::translation(amt, angle);
    }

    pub fn rotate(&mut self, angle: f64) {
        self.camera = self.camera * SpinorT::rotation(angle);
    }

    pub fn set_drag(&mut self, pos_from: Vector2<f64>, pos_to: Vector2<f64>) {
        //println!("pos_from {:?}, pos_to {:?}", pos_from, pos_to);
        self.pending_camera =
            SpinorT::translation_to(pos_to).reverse() * SpinorT::translation_to(pos_from)
    }

    pub fn apply_drag(&mut self) {
        self.camera = self.pending_camera * self.camera;
        self.pending_camera = SpinorT::one();
    }

    pub fn get_camera_mat(&self) -> Matrix4<f32> {
        let mut scale_mat = Matrix4::<f32>::one();
        scale_mat.w.w = 1.0 / self.scale as f32;

        scale_mat * (self.pending_camera * self.camera).reverse().into_mat4()
    }
}
