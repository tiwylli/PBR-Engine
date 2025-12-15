use std::f64::consts::PI;

use crate::Real;
use cgmath;

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
pub fn to_srgb(c: &Color3) -> Color3 {
    let mut result = Color3::new(0.0, 0.0, 0.0);

    for i in 0..3 {
        let value = c[i];
        if value <= 0.0031308 {
            result[i] = 12.92 * value;
        } else {
            result[i] = (1.0 + 0.055) * value.powf(1.0 / 2.4) - 0.055;
        }
    }

    result
}

/// Convert from sRGB to linear RGB
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
pub fn luminance(c: &Color3) -> Real {
    cgmath::dot(*c, Color3::new(0.212671, 0.715160, 0.072169))
}

/// Construct a frame using only one vector
pub struct Frame {
    x: Vec3,
    y: Vec3,
    z: Vec3,
}

impl Frame {
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

        Frame { x, y, z: *n }
    }

    pub fn to_world(&self, v: &Vec3) -> Vec3 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }

    pub fn to_local(&self, v: &Vec3) -> Vec3 {
        Vec3::new(
            cgmath::dot(*v, self.x),
            cgmath::dot(*v, self.y),
            cgmath::dot(*v, self.z),
        )
    }
}

pub fn sample_spherical(sample: &Vec2) -> Vec3 {
    let cos_theta = 1.0 - 2.0 * sample.x;
    let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
    let phi = 2.0 * PI * sample.y;
    let x = sin_theta * phi.cos();
    let y = sin_theta * phi.sin();
    Vec3::new(x, y, cos_theta)
}

pub fn pdf_spherical(_dir: &Vec3) -> f64 {
    1.0 / (4.0 * PI)
}

pub fn sample_hemisphere(sample: &Vec2) -> Vec3 {
    let theta = (1.0 - sample.x).acos();
    let phi = 2.0 * PI * sample.y;
    let x = theta.sin() * phi.cos();
    let y = theta.sin() * phi.sin();
    let z = theta.cos();
    Vec3::new(x, y, z)
}

pub fn pdf_hemisphere(dir: &Vec3) -> f64 {
    if dir.z > 0.0 {
        1.0 / (2.0 * PI)
    } else {
        0.0
    }
}

pub fn sample_cosine_hemisphere(sample: &Vec2) -> Vec3 {
    let r: f64 = sample.x.sqrt();
    let theta = 2.0 * PI * sample.y;
    let x = r * theta.cos();
    let y = r * theta.sin();
    let z = (1.0 - x * x - y * y).sqrt();
    Vec3::new(x, y, z)
}

pub fn pdf_cosine_hemisphere(dir: &Vec3) -> f64 {
    if dir.z > 0.0 {
        dir.z / PI
    } else {
        0.0
    }
}

pub fn sample_cosine_hemisphere_power(sample: &Vec2, power: f64) -> Vec3 {
    let phi = 2.0 * PI * sample.x;

    let inv = 1.0 / (power + 1.0);
    let cos_theta = (1.0 - sample.y).powf(inv);

    let sin_theta_sq = (1.0 - cos_theta * cos_theta).max(0.0);
    let sin_theta = sin_theta_sq.sqrt();
    let sin_phi = phi.sin();
    let cos_phi = phi.cos();

    Vec3::new(cos_phi * sin_theta, sin_phi * sin_theta, cos_theta)
}

pub fn pdf_cosine_hemisphere_power(dir: &Vec3, power: f64) -> f64 {
    let cos_theta = dir.z;
    if cos_theta <= 0.0 {
        0.0
    } else {
        (power + 1.0) * cos_theta.powf(power) / (2.0 * PI)
    }
}

pub fn sample_cone(sample: &Vec2, theta_max: f64) -> Vec3 {
    let cos_theta_max = theta_max.cos();

    let phi = 2.0 * PI * sample.x;
    let cos_theta = 1.0 - sample.y * (1.0 - cos_theta_max);
    let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
    let sin_phi = phi.sin();
    let cos_phi = phi.cos();
    Vec3::new(cos_phi * sin_theta, sin_phi * sin_theta, cos_theta)
}

pub fn pdf_cone(dir: &Vec3, theta_max: f64) -> f64 {
    let cos_theta_max = theta_max.cos();
    let cos_theta = dir.z;

    if cos_theta < cos_theta_max {
        0.0
    } else {
        1.0 / (2.0 * PI * (1.0 - cos_theta_max))
    }
}
