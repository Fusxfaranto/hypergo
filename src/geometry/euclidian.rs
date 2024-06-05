use std::ops;

use cgmath::{assert_abs_diff_eq, vec2, vec4, Matrix, Matrix4, Vector2};

use super::*;

#[derive(Copy, Clone)]
pub struct SpinorEuclidian {
    s: f64,
    xy: f64,
    yw: f64,
    wx: f64,
}

impl Spinor for SpinorEuclidian {
    fn apply(&self, v: Vector2<f64>) -> Vector2<f64> {
        // TODO faster implementation
        let m = self.into_mat4();
        let v_out = m * vec4(v.x, v.y, 0.0, 1.0);
        assert_abs_diff_eq!(v_out.w, 1.0);
        return vec2(v_out.x, v_out.y);
    }

    fn reverse(&self) -> Self {
        Self {
            s: self.s,
            xy: -self.xy,
            yw: -self.yw,
            wx: -self.wx,
        }
    }

    fn into_mat4<S: 'static + BaseFloat>(&self) -> Matrix4<S>
    where
        f32: AsPrimitive<S>,
        f64: AsPrimitive<S>,
    {
        // TODO support non-unit?
        Matrix4::new(
            (self.s * self.s - self.xy * self.xy).as_(),
            (2.0 * self.s * self.xy).as_(),
            0.0.as_(),
            (-2.0 * self.s * self.wx + 2.0 * self.yw * self.xy).as_(),
            (-2.0 * self.s * self.xy).as_(),
            (self.s * self.s - self.xy * self.xy).as_(),
            0.0.as_(),
            (2.0 * self.s * self.yw + 2.0 * self.wx * self.xy).as_(),
            0.0.as_(),
            0.0.as_(),
            0.0.as_(),
            0.0.as_(),
            0.0.as_(),
            0.0.as_(),
            0.0.as_(),
            (self.s * self.s + self.xy * self.xy).as_(),
        )
        .transpose() // TODO really?? why????
    }

    fn identity() -> Self {
        Self {
            s: 1.0,
            xy: 0.0,
            yw: 0.0,
            wx: 0.0,
        }
    }

    fn translation(amt: f64, angle: f64) -> Self {
        // TODO ??? pretty much just guessing at this one
        let b2 = amt / 2.0;
        Self {
            s: 1.0,
            xy: 0.0,
            yw: -angle.sin() * b2,
            wx: angle.cos() * b2,
        }
    }
}

// TODO use references over copies?
impl ops::Mul<SpinorEuclidian> for SpinorEuclidian {
    type Output = SpinorEuclidian;

    fn mul(self, rhs: SpinorEuclidian) -> SpinorEuclidian {
        SpinorEuclidian {
            s: rhs.s * self.s - rhs.xy * self.xy,
            xy: rhs.xy * self.s + rhs.s * self.xy,
            yw: rhs.yw * self.s + rhs.wx * self.xy + rhs.s * self.yw - rhs.xy * self.wx,
            wx: rhs.wx * self.s - rhs.yw * self.xy + rhs.xy * self.yw + rhs.s * self.wx,
        }
    }
}
