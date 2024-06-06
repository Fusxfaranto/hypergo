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
    ) -> SpinorT::Point {
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
            const LIMIT: f64 = 0.99;
            let mag = v.magnitude2();
            //println!("{mag2}");
            if mag < LIMIT {
                v
            } else {
                v * (LIMIT / mag).sqrt()
            }
        }

        self.camera
            .apply(SpinorT::Point::from_flat_vec(limit(scaled)))
    }

    pub fn adjust_scale(&mut self, amt: f64) {
        self.scale *= amt + 1.0;
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
        scale_mat.w.w = 1.0 / self.scale as f32;

        scale_mat * (self.pending_camera * self.camera).reverse().into_mat4()
    }
}
