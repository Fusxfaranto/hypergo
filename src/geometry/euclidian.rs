use std::{f64::consts::PI, fmt, ops};

use cgmath::{assert_abs_diff_eq, vec2, vec3, vec4, Matrix, Matrix4, Vector2, Zero};

use super::*;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PointEuclidian {
    x: f64,
    y: f64,
    // projective coordinate is always 1, no reason to keep that around
    // w: f64,
}

impl Point for PointEuclidian {
    fn distance(self, b: Self) -> f64 {
        ((self.x - b.x).powi(2) + (self.y - b.y).powi(2)).sqrt()
    }
    fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    fn from_flat(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn from_projective(x: f64, y: f64, w: f64) -> Self {
        Self { x: x / w, y: y / w }
    }

    fn angle(&self) -> f64 {
        self.y.atan2(self.x)
    }

    fn to_projective<S: 'static + BaseFloat>(&self) -> Vector3<S>
    where
        f32: AsPrimitive<S>,
        f64: AsPrimitive<S>,
    {
        vec3(self.x.as_(), self.y.as_(), 1.0.as_())
    }

    /*    fn flat_magnitude(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    } */
}

impl AbsDiffEq for PointEuclidian {
    type Epsilon = f64;

    fn default_epsilon() -> Self::Epsilon {
        1e-9
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        f64::abs_diff_eq(&self.x, &other.x, epsilon) && f64::abs_diff_eq(&self.y, &other.y, epsilon)
    }
}

impl ops::Mul<f64> for PointEuclidian {
    type Output = PointEuclidian;

    fn mul(self, rhs: f64) -> Self {
        Self {
            x: rhs * self.x,
            y: rhs * self.y,
        }
    }
}

impl Display for PointEuclidian {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let precision = f.precision().unwrap_or(3);
        write!(f, "[{:.*?}, {:.*?}]", precision, self.x, precision, self.y)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SpinorEuclidian {
    s: f64,
    xy: f64,
    yw: f64,
    wx: f64,
}

impl Spinor for SpinorEuclidian {
    type Point = PointEuclidian;

    fn new(s: f64, xy: f64, yw: f64, wx: f64) -> Self {
        Self { s, xy, yw, wx }
    }

    fn apply(&self, v: Self::Point) -> Self::Point {
        assert_abs_diff_eq!(self.s * self.s + self.xy * self.xy, 1.0, epsilon = 1e-6);
        Self::Point {
            x: (self.s * self.s - self.xy * self.xy) * v.x
                + (2.0 * self.s * self.xy) * v.y
                + (-2.0 * self.s * self.wx + 2.0 * self.yw * self.xy),
            y: (-2.0 * self.s * self.xy) * v.x
                + (self.s * self.s - self.xy * self.xy) * v.y
                + (2.0 * self.s * self.yw + 2.0 * self.wx * self.xy),
        }
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
        self.s * self.s + self.xy * self.xy
    }

    fn distance(self, b: Self) -> f64 {
        // TODO fast version
        let p = self.apply(Point::zero());
        let q = b.apply(Point::zero());
        p.distance(q)
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

    fn translation(amt: f64, angle: f64) -> Self {
        let b2 = amt / 2.0;
        Self {
            s: 1.0,
            xy: 0.0,
            yw: angle.cos() * b2,
            wx: angle.sin() * b2,
        }
    }

    fn translation_to(v: Self::Point) -> Self {
        Self {
            s: 1.0,
            xy: 0.0,
            yw: v.y / 2.0,
            wx: -v.x / 2.0,
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

    fn tiling_get_distance(sides: u32, angle: f64) -> f64 {
        assert_abs_diff_eq!(
            (PI / (sides as f64)).cos() / (0.5 * angle).sin(),
            1.0,
            epsilon = 1e-11
        );
        1.0
    }

    fn distance_to_flat(d: f64) -> f64 {
        d
    }
}

impl One for SpinorEuclidian {
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
impl ops::Mul<f64> for SpinorEuclidian {
    type Output = SpinorEuclidian;

    fn mul(self, rhs: f64) -> SpinorEuclidian {
        SpinorEuclidian {
            s: rhs * self.s,
            xy: rhs * self.xy,
            yw: rhs * self.yw,
            wx: rhs * self.wx,
        }
    }
}

impl AbsDiffEq for SpinorEuclidian {
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
    use super::*;

    #[test]
    fn test_translation() {
        // TODO is this actually how it should be?
        let v = PointEuclidian::from_flat(0.0, 1.0);
        let s = SpinorEuclidian::translation(1.0, 0.0);
        assert_abs_diff_eq!(s.apply(PointEuclidian::zero()), v);
        assert_abs_diff_eq!(s.reverse().apply(v), PointEuclidian::zero());
    }

    #[test]
    fn test_translation_to() {
        let v = PointEuclidian::from_flat(0.7, -0.3);
        let s = SpinorEuclidian::translation_to(v);
        assert_abs_diff_eq!(s.apply(PointEuclidian::zero()), v);
        assert_abs_diff_eq!(s.reverse().apply(v), PointEuclidian::zero());
    }
}
