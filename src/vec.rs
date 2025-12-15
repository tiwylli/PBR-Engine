use crate::Real;
use cgmath::{self, InnerSpace};

// 2D
pub type Vec2 = cgmath::Vector2<Real>;
pub type Point2 = cgmath::Point2<Real>;
pub type Vec2i = cgmath::Vector2<i32>;
pub type Vec2u = cgmath::Vector2<u32>;

// 3D and color
pub type Vec3 = cgmath::Vector3<Real>;
pub type Point3 = cgmath::Point3<Real>;
pub type Vec3i = cgmath::Vector3<i32>;
pub type Vec3u = cgmath::Vector3<u32>;
pub type Color3 = cgmath::Vector3<Real>;
pub type Color3u = cgmath::Vector3<u32>;
pub type Color3c = cgmath::Vector3<u8>;

// 4D
pub type Vec4 = cgmath::Vector4<Real>;

// Matrices
pub type Mat2 = cgmath::Matrix2<Real>;
pub type Mat3 = cgmath::Matrix3<Real>;
pub type Mat4 = cgmath::Matrix4<Real>;

/// Convert from linear RGB to sRGB
#[must_use]
pub fn to_srgb(c: &Color3) -> Color3 {
    let mut result = Color3::new(0.0, 0.0, 0.0);

    for i in 0..3 {
        let value = c[i];
        if value <= 0.003_130_8 {
            result[i] = 12.92 * value;
        } else {
            result[i] = value.powf(1.0 / 2.4).mul_add(1.0 + 0.055, -0.055);
        }
    }

    result
}

/// Convert from sRGB to linear RGB
#[must_use]
pub fn to_linear_rgb(c: &Color3) -> Color3 {
    let mut result = Color3::new(0.0, 0.0, 0.0);

    for i in 0..3 {
        let value = c[i];

        if value <= 0.04045 {
            result[i] = value * (1.0 / 12.92);
        } else {
            result[i] = ((value + 0.055) / 1.055).powf(2.4);
        }
    }

    result
}

/// Check if the color vector contains a NaN/Inf/negative value
#[must_use]
pub fn is_valid_color(c: &Color3) -> bool {
    for i in 0..3 {
        let value = c[i];
        if value < 0.0 || !value.is_finite() {
            return false;
        }
    }
    true
}

/// Return the associated luminance
#[must_use]
pub fn luminance(c: &Color3) -> Real {
    cgmath::dot(*c, Color3::new(0.212_671, 0.715_160, 0.072_169))
}

/// Construct a frame using only one vector
pub struct Frame {
    x: Vec3,
    y: Vec3,
    z: Vec3,
}

impl Frame {
    #[must_use]
    #[allow(clippy::many_single_char_names, clippy::suboptimal_flops)]
    pub fn new(n: &Vec3) -> Self {
        // Based on "Building an Orthonormal Basis, Revisited" by
        // Tom Duff, James Burgess, Per Christensen, Christophe Hery, Andrew Kensler,
        // Max Liani, and Ryusuke Villemin
        // https://graphics.pixar.com/library/OrthonormalB/paper.pdf
        let sign = n.z.signum();
        let a = -1.0 / (sign + n.z);
        let b = n.x * n.y * a;
        let x = Vec3::new(1.0 + sign * n.x * n.x * a, sign * b, -sign * n.x);
        let y = Vec3::new(b, sign + n.y * n.y * a, -n.y);

        Self { x, y, z: *n }
    }

    #[must_use]
    pub fn to_world(&self, v: &Vec3) -> Vec3 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }

    #[must_use]
    pub fn to_local(&self, v: &Vec3) -> Vec3 {
        Vec3::new(
            cgmath::dot(*v, self.x),
            cgmath::dot(*v, self.y),
            cgmath::dot(*v, self.z),
        )
    }
}

#[must_use]
pub fn spherical_to_directional(theta: f64, phi: f64) -> Vec3 {
    Vec3::new(
        theta.sin() * phi.cos(),
        theta.sin() * phi.sin(),
        theta.cos(),
    )
}

#[must_use]
pub fn reflect(v: &Vec3, n: &Vec3) -> Vec3 {
    let n = n.normalize();
    2.0 * v.dot(n) * n - v
}
