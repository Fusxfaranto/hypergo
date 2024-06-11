use std::f64::consts::PI;
use std::fmt::Debug;
use std::ops;

use cgmath::{num_traits::AsPrimitive, vec2, AbsDiffEq, BaseFloat, Matrix4, One, Vector2};
use cgmath::{InnerSpace, Vector3, Zero};
use log::info;
use wgpu::SurfaceConfiguration;

pub mod euclidian;
pub mod hyperbolic;

pub trait Point: Copy + Clone + Debug + PartialEq + AbsDiffEq // + ops::Mul<f64, Output = Self>
{
    fn distance(self, b: Self) -> f64;

    fn zero() -> Self;
    fn from_flat(x: f64, y: f64) -> Self;
    fn from_projective(x: f64, y: f64, w: f64) -> Self;

    fn angle(&self) -> f64;
    /*     fn flat_magnitude(&self) -> f64; */

    fn to_projective<S: 'static + BaseFloat>(&self) -> Vector3<S>
    where
        f32: AsPrimitive<S>,
        f64: AsPrimitive<S>;

    fn from_flat_vec(v: Vector2<f64>) -> Self {
        Self::from_flat(v.x, v.y)
    }
}

pub trait Spinor:
    Copy + Clone + Debug + ops::Mul<Output = Self> + ops::Mul<f64, Output = Self> + One + AbsDiffEq
{
    type Point: Point;

    fn new(s: f64, xy: f64, yw: f64, wx: f64) -> Self;
    fn translation(amt: f64, angle: f64) -> Self;
    fn translation_to(v: Self::Point) -> Self;
    fn rotation(angle: f64) -> Self;

    fn reverse(&self) -> Self;
    fn magnitude2(&self) -> f64;
    fn apply(&self, v: Self::Point) -> Self::Point;
    fn into_mat4<S: 'static + BaseFloat>(&self) -> Matrix4<S>
    where
        f32: AsPrimitive<S>,
        f64: AsPrimitive<S>;

    // TODO doesn't really fit here
    fn tiling_get_distance(sides: u32, angle: f64) -> f64;
    fn distance_to_flat(d: f64) -> f64;

    fn magnitude(&self) -> f64 {
        self.magnitude2().sqrt()
    }
    // TODO implement MulAssign?
    fn normalize(&mut self) {
        *self = *self * (1.0 / self.magnitude());
    }
}

#[derive(Copy, Clone, Debug)]
pub struct TilingParameters {
    pub sides: u32,
    pub around_vertex: u32,
    pub angle: f64,
    pub distance: f64,
    // in flat coordinates
    pub link_len: f64,
}

impl TilingParameters {
    pub fn new<SpinorT: Spinor>(sides: u32, around_vertex: u32) -> TilingParameters {
        let angle = 2.0 * PI / (around_vertex as f64);
        let distance = SpinorT::tiling_get_distance(sides, angle);
        let link_len = SpinorT::distance_to_flat(distance);
        Self {
            sides,
            around_vertex,
            angle,
            distance,
            link_len,
        }
    }
}

pub struct ViewState<SpinorT: Spinor> {
    // scale for euclidian, poincare factor for hyperbolic
    pub projection_factor: f64,
    // TODO shouldn't need to be pub (testing things)
    pub camera: SpinorT,
    floating_origin: SpinorT,
}

// TODO lots of cfg! here, break some of it out into trait impls?
impl<SpinorT: Spinor> ViewState<SpinorT> {
    pub fn new() -> Self {
        Self {
            projection_factor: 1.0,
            camera: SpinorT::one(),
            floating_origin: SpinorT::one(),
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

    pub fn reset_camera(&mut self) {
        self.camera = SpinorT::one();
    }

    pub fn translate(&mut self, amt: f64, angle: f64) {
        self.camera = self.camera * SpinorT::translation(amt, angle);
        self.camera.normalize();
    }

    pub fn rotate(&mut self, angle: f64) {
        self.camera = self.camera * SpinorT::rotation(angle);
        self.camera.normalize();
    }

    pub fn drag(&mut self, pos_from: SpinorT::Point, pos_to: SpinorT::Point) {
        // info!("pos_from {:?}, pos_to {:?}", pos_from, pos_to);
        // info!(
        //     "transformation {:?}",
        //     SpinorT::translation_to(pos_from) * SpinorT::translation_to(pos_to).reverse()
        // );
        // TODO i don't really understand the math on this
        /*         self.camera = self.camera
         * SpinorT::translation_to(pos_to)
         * SpinorT::translation_to(pos_from).reverse(); */
        self.camera = SpinorT::translation_to(pos_from)
            * SpinorT::translation_to(pos_to).reverse()
            * self.camera;
        self.camera.normalize();
        //info!("camera {:?}", self.camera);
    }

    pub fn update_floating_origin(&mut self) {
        self.floating_origin = self.camera;
    }

    pub fn get_camera_mat(&self) -> Matrix4<f32> {
        let mut scale_mat = Matrix4::<f32>::one();
        if cfg!(feature = "euclidian_geometry") {
            scale_mat.w.w = 1.0 / self.projection_factor as f32;
        }

        scale_mat * (self.camera.reverse() * self.floating_origin).into_mat4()
    }
}
