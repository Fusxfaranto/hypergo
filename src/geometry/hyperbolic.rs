use std::{f64::consts::PI, ops};

use cgmath::{
    assert_abs_diff_eq, num_traits::Pow, vec2, vec4, BaseFloat, Matrix, Matrix4, Vector2,
};

use super::*;

#[derive(Copy, Clone)]
pub struct SpinorHyperbolic {
    s: f64,
    xy: f64,
    yw: f64,
    wx: f64,
}

impl Spinor for SpinorHyperbolic {
    fn apply(&self, v: Vector2<f64>) -> Vector2<f64> {
        // TODO faster implementation
        let m = self.into_mat4();
        let v_out = m * vec4(v.x, v.y, 0.0, 1.0);
        return vec2(v_out.x, v_out.y) / v_out.w;
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
        // TODO signs not totally matching up with old ver
        // appears to be flipped diagonally????
        /*
        Matrix4::new(
            (self.s * self.s + self.wx * self.wx - self.yw * self.yw - self.xy * self.xy).as_(),
            (-2.0 * self.s * self.xy + 2.0 * self.wx * self.yw).as_(),
            0.0.as_(),
            (2.0 * self.s * self.wx - 2.0 * self.yw * self.xy).as_(),
            (2.0 * self.s * self.xy + 2.0 * self.wx * self.yw).as_(),
            (self.s * self.s - self.wx * self.wx + self.yw * self.yw - self.xy * self.xy).as_(),
            0.0.as_(),
            (2.0 * self.s * self.yw + 2.0 * self.wx * self.xy).as_(),
            0.0.as_(),
            0.0.as_(),
            0.0.as_(),
            0.0.as_(),
            (2.0 * self.s * self.wx + 2.0 * self.yw * self.xy).as_(),
            (2.0 * self.s * self.yw - 2.0 * self.wx * self.xy).as_(),
            0.0.as_(),
            (self.s * self.s + self.wx * self.wx + self.yw * self.yw + self.xy * self.xy).as_(),
        )*/
        Matrix4::new(
            (self.s * self.s + self.wx * self.wx - self.yw * self.yw - self.xy * self.xy).as_(),
            (2.0 * self.s * self.xy - 2.0 * self.wx * self.yw).as_(),
            0.0.as_(),
            (-2.0 * self.s * self.wx + 2.0 * self.yw * self.xy).as_(),
            (-2.0 * self.s * self.xy - 2.0 * self.wx * self.yw).as_(),
            (self.s * self.s - self.wx * self.wx + self.yw * self.yw - self.xy * self.xy).as_(),
            0.0.as_(),
            (2.0 * self.s * self.yw + 2.0 * self.wx * self.xy).as_(),
            0.0.as_(),
            0.0.as_(),
            0.0.as_(),
            0.0.as_(),
            (-2.0 * self.s * self.wx - 2.0 * self.yw * self.xy).as_(),
            (2.0 * self.s * self.yw - 2.0 * self.wx * self.xy).as_(),
            0.0.as_(),
            (self.s * self.s + self.wx * self.wx + self.yw * self.yw + self.xy * self.xy).as_(),
        )
        .transpose()
    }

    fn translation(amt: f64, angle: f64) -> Self {
        let b2 = amt / 2.0;
        Self {
            s: b2.cosh(),
            xy: 0.0,
            yw: angle.cos() * b2.sinh(),
            wx: angle.sin() * b2.sinh(),
        }
    }

    fn rotation(angle: f64) -> Self {
        let t2 = angle / 2.0;
        Self {
            s: t2.cos(),
            xy: t2.sin(),
            yw: 0.0,
            wx: 0.0,
        }
    }

    fn distance(a: Vector2<f64>, b: Vector2<f64>) -> f64 {
        // TODO wrong?
        (1.0 - (a.x - b.x).powi(2) - (a.y - b.y).powi(2)).acosh()
    }

    fn tiling_neighbor_directions() -> Vec<Vec<Self>> {
        // TODO
        vec![vec![
            Self::translation(1.0, 0.0),
            Self::translation(1.0, PI / 2.0),
            Self::translation(1.0, PI),
            Self::translation(1.0, 3.0 * PI / 2.0),
        ]]
    }
}

impl One for SpinorHyperbolic {
    fn one() -> Self {
        Self {
            s: 1.0,
            xy: 0.0,
            yw: 0.0,
            wx: 0.0,
        }
    }
}

// TODO use references over copies?
impl ops::Mul<SpinorHyperbolic> for SpinorHyperbolic {
    type Output = SpinorHyperbolic;

    fn mul(self, rhs: SpinorHyperbolic) -> SpinorHyperbolic {
        SpinorHyperbolic {
            s: self.s * rhs.s - self.xy * rhs.xy + self.yw * rhs.yw + self.wx * rhs.wx,
            xy: self.s * rhs.xy + self.xy * rhs.s + self.yw * rhs.wx - self.wx * rhs.yw,
            yw: self.s * rhs.yw + self.yw * rhs.s - self.wx * rhs.xy + self.xy * rhs.wx,
            wx: self.s * rhs.wx + self.wx * rhs.s - self.xy * rhs.yw + self.yw * rhs.xy,
        }
    }
}
