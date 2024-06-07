use std::fmt::Debug;
use std::ops;

use cgmath::{num_traits::AsPrimitive, vec2, AbsDiffEq, BaseFloat, Matrix4, One, Vector2};
use cgmath::{InnerSpace, Zero};
use wgpu::SurfaceConfiguration;

pub mod euclidian;
pub mod hyperbolic;

pub trait Point: Copy + Clone + Debug + PartialEq + AbsDiffEq // + ops::Mul<f64, Output = Self>
{
    fn distance(self, b: Self) -> f64;

    fn zero() -> Self;
    fn from_flat(x: f64, y: f64) -> Self;

    fn angle(&self) -> f64;
    fn flat_magnitude(&self) -> f64;

    fn from_flat_vec(v: Vector2<f64>) -> Self {
        Self::from_flat(v.x, v.y)
    }
}

pub trait Spinor: Copy + Clone + ops::Mul<Output = Self> + One + AbsDiffEq {
    type Point: Point;

    fn translation(amt: f64, angle: f64) -> Self;
    fn translation_to(v: Self::Point) -> Self;
    fn rotation(angle: f64) -> Self;
    fn reverse(&self) -> Self;
    fn apply(&self, v: Self::Point) -> Self::Point;
    fn into_mat4<S: 'static + BaseFloat>(&self) -> Matrix4<S>
    where
        f32: AsPrimitive<S>,
        f64: AsPrimitive<S>;

    fn tiling_neighbor_directions() -> Vec<Vec<Self>>;
}

pub struct ViewState<SpinorT: Spinor> {
    // scale for euclidian, poincare factor for hyperbolic
    pub projection_factor: f64,
    camera: SpinorT,
    pending_camera: SpinorT,
}

// TODO lots of cfg! here, break some of it out into trait impls?
impl<SpinorT: Spinor> ViewState<SpinorT> {
    pub fn new() -> Self {
        Self {
            projection_factor: 1.0,
            camera: SpinorT::one(),
            pending_camera: SpinorT::one(),
        }
    }

    pub fn pixel_to_world_coords(
        &self,
        config: &SurfaceConfiguration,
        x: f64,
        y: f64,
    ) -> SpinorT::Point {
        let v = vec2(
            2.0 * x / config.width as f64 - 1.0,
            -2.0 * y / config.height as f64 + 1.0,
        );
        let adjusted = if cfg!(feature = "euclidian_geometry") {
            (1.0 / self.projection_factor) * v
        } else {
            const LIMIT: f64 = 0.99;
            let mag2 = v.magnitude2();
            let limited = if mag2 < LIMIT {
                v
            } else {
                v * (LIMIT / mag2).sqrt()
            };
            let base = (0.5 * (1.0 + mag2.min(LIMIT))) * self.projection_factor + 1.0
                - self.projection_factor;
            limited / base
        };

        self.camera.apply(SpinorT::Point::from_flat_vec(adjusted))
    }

    pub fn adjust_projection_factor(&mut self, amt: f64) {
        if cfg!(feature = "euclidian_geometry") {
            self.projection_factor *= amt + 1.0;
        } else {
            self.projection_factor = (self.projection_factor + amt).clamp(0.0, 1.0);
        }
    }

    pub fn translate(&mut self, amt: f64, angle: f64) {
        self.camera = self.camera * SpinorT::translation(amt, angle);
    }

    pub fn rotate(&mut self, angle: f64) {
        self.camera = self.camera * SpinorT::rotation(angle);
    }

    // TODO something not exactly right with how this works
    pub fn set_drag(&mut self, pos_from: SpinorT::Point, pos_to: SpinorT::Point) {
        //println!("pos_from {:?}, pos_to {:?}", pos_from, pos_to);
        self.pending_camera =
            SpinorT::translation_to(pos_to).reverse() * SpinorT::translation_to(pos_from);

        // TODO would really prefer to be able to do this but it gets scummy
        // fixable without terrible hacks?
        //self.apply_drag();
    }

    pub fn apply_drag(&mut self) {
        self.camera = self.pending_camera * self.camera;
        self.pending_camera = SpinorT::one();
    }

    pub fn get_camera_mat(&self) -> Matrix4<f32> {
        let mut scale_mat = Matrix4::<f32>::one();
        if cfg!(feature = "euclidian_geometry") {
            scale_mat.w.w = 1.0 / self.projection_factor as f32;
        }

        scale_mat * (self.pending_camera * self.camera).reverse().into_mat4()
    }
}
