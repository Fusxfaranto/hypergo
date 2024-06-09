use std::{f64::consts::PI, ops};

use cgmath::{
    assert_abs_diff_eq,
    num_traits::{Float, Pow},
    vec2, vec3, vec4, BaseFloat, InnerSpace, Matrix, Matrix4, Vector2, Vector3, Zero,
};
use more_asserts::assert_gt;

use super::*;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PointHyperbolic {
    x: f64,
    y: f64,
    w: f64,
}

impl Point for PointHyperbolic {
    fn distance(self, b: Self) -> f64 {
        /*         fn to_hyperboloid(v: Self) -> Vector3<f64> {
                   let w = (1.0 / (1.0 - v.x * v.x - v.y * v.y)).sqrt();
                   vec3(v.x * w, v.y * w, w)
               }
               let a_h = to_hyperboloid(a);
               let b_h = to_hyperboloid(b);
               //println!("a {:?} {:?}, b {:?} {:?}", a, a_h, b, b_h);
               let bl = a_h.z * b_h.z - a_h.x * b_h.x - a_h.y * b_h.y;
        */

        let bl = self.w * b.w - self.x * b.x - self.y * b.y;
        assert_gt!(bl, 0.99);
        let d = bl.max(1.0).acosh();
        //println!("d {d}");
        d
    }

    fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            w: 1.0,
        }
    }

    fn from_flat(x: f64, y: f64) -> Self {
        let w = (1.0 / (1.0 - x * x - y * y)).sqrt();
        Self {
            x: x * w,
            y: y * w,
            w,
        }
    }

    fn from_projective(x: f64, y: f64, w: f64) -> Self {
        assert_abs_diff_eq!(w * w, 1.0 + x * x + y * y, epsilon = 1e-9);
        Self { x, y, w }
    }

    fn angle(&self) -> f64 {
        self.y.atan2(self.x)
    }

    fn flat_magnitude(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt() / self.w
    }
}

impl AbsDiffEq for PointHyperbolic {
    type Epsilon = f64;

    fn default_epsilon() -> Self::Epsilon {
        1e-9
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        f64::abs_diff_eq(&self.x, &other.x, epsilon)
            && f64::abs_diff_eq(&self.y, &other.y, epsilon)
            && f64::abs_diff_eq(&self.w, &other.w, epsilon)
    }
}

impl ops::Mul<f64> for PointHyperbolic {
    type Output = PointHyperbolic;

    // TODO pretty sure this is wrong
    fn mul(self, rhs: f64) -> Self {
        Self {
            x: rhs * self.x,
            y: rhs * self.y,
            w: rhs * self.w,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SpinorHyperbolic {
    s: f64,
    xy: f64,
    yw: f64,
    wx: f64,
}

impl Spinor for SpinorHyperbolic {
    type Point = PointHyperbolic;

    fn new(s: f64, xy: f64, yw: f64, wx: f64) -> Self {
        Self { s, xy, yw, wx }
    }

    fn apply(&self, v: Self::Point) -> Self::Point {
        // TODO faster implementation
        let m = self.into_mat4();
        let v_out = m * vec4(v.x, v.y, 0.0, v.w);
        return Self::Point {
            x: v_out.x,
            y: v_out.y,
            w: v_out.w,
        };
    }

    fn reverse(&self) -> Self {
        Self {
            s: self.s,
            xy: -self.xy,
            yw: -self.yw,
            wx: -self.wx,
        }
    }

    fn magnitude2(&self) -> f64 {
        self.s * self.s + self.xy * self.xy - self.yw * self.yw - self.wx * self.wx
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

    fn translation_to(v: Self::Point) -> Self {
        /*
        let v_mag = v.magnitude();

        let b2 = v_mag.atanh() / 2.0;
        let v_norm = v / v_mag;
        Self {
            s: b2.cosh(),
            xy: 0.0,
            yw: v_norm.y * b2.sinh(),
            wx: -v_norm.x * b2.sinh(),
        } */

        let w_factor = (2.0 * (v.w + 1.0)).sqrt();

        Self {
            s: (0.5 * (v.w + 1.0)).sqrt(),
            xy: 0.0,
            yw: v.y / w_factor,
            wx: -v.x / w_factor,
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
    /*
    fn tiling_neighbor_directions() -> Vec<Vec<Self>> {
        let mut res = vec![];
        let d_3_7 = 1.0905496635070862;
        res.push(vec![
            Self::translation(d_3_7, 0.0),
            Self::translation(d_3_7, 2.0 * PI / 7.0),
            Self::translation(d_3_7, 4.0 * PI / 7.0),
            Self::translation(d_3_7, 6.0 * PI / 7.0),
            Self::translation(d_3_7, 8.0 * PI / 7.0),
            Self::translation(d_3_7, 10.0 * PI / 7.0),
            Self::translation(d_3_7, 12.0 * PI / 7.0),
        ]);
        let d_7_3 = 0.5662563067353151;
        res.push(vec![
            Self::translation(d_7_3, 0.0),
            Self::translation(d_7_3, 2.0 * PI / 3.0),
            Self::translation(d_7_3, 4.0 * PI / 3.0),
        ]);
        let d_4_5 = 1.2537393258123553;
        res.push(vec![
            Self::translation(d_4_5, 0.0),
            Self::translation(d_4_5, 2.0 * PI / 5.0),
            Self::translation(d_4_5, 4.0 * PI / 5.0),
            Self::translation(d_4_5, 6.0 * PI / 5.0),
            Self::translation(d_4_5, 8.0 * PI / 5.0),
        ]);
        let d_5_4 = 1.061275061905036;
        res.push(vec![
            Self::translation(d_5_4, 0.0),
            Self::translation(d_5_4, PI / 2.0),
            Self::translation(d_5_4, PI),
            Self::translation(d_5_4, 3.0 * PI / 2.0),
        ]);
        res
    } */

    fn tiling_get_distance(sides: u32, angle: f64) -> f64 {
        2.0 * ((PI / (sides as f64)).cos() / (0.5 * angle).sin()).acosh()
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
impl ops::Mul<f64> for SpinorHyperbolic {
    type Output = SpinorHyperbolic;

    fn mul(self, rhs: f64) -> SpinorHyperbolic {
        SpinorHyperbolic {
            s: rhs * self.s,
            xy: rhs * self.xy,
            yw: rhs * self.yw,
            wx: rhs * self.wx,
        }
    }
}

impl AbsDiffEq for SpinorHyperbolic {
    type Epsilon = f64;

    fn default_epsilon() -> Self::Epsilon {
        1e-9
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        f64::abs_diff_eq(&self.s, &other.s, epsilon)
            && f64::abs_diff_eq(&self.xy, &other.xy, epsilon)
            && f64::abs_diff_eq(&self.yw, &other.yw, epsilon)
            && f64::abs_diff_eq(&self.wx, &other.wx, epsilon)
    }
}

#[cfg(test)]
mod tests {
    use more_asserts::assert_lt;

    use super::*;

    #[test]
    fn test_translation_to() {
        let v = PointHyperbolic::from_flat(0.7, -0.4);
        let s = SpinorHyperbolic::translation_to(v);
        assert_abs_diff_eq!(s.apply(PointHyperbolic::zero()), v, epsilon = 1e-9);
        assert_abs_diff_eq!(
            s.reverse().apply(v),
            PointHyperbolic::zero(),
            epsilon = 1e-9
        );
    }

    #[test]
    fn test_distance() {
        let a = PointHyperbolic::from_flat(-7.617857059728038e-33, 0.7861513777574234);
        let b = PointHyperbolic::from_flat(0.0, 0.7861513777574233);
        assert_lt!(a.distance(b), 1.0);
    }
}
