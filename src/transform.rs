use std::collections::HashMap;

use cgmath::{Matrix, SquareMatrix};
use tinyjson::JsonValue;

use crate::json::json_to_mat4;
use crate::ray::Ray;
use crate::vec::{Mat4, Point3, Vec3, Vec4};

// Note: Unfortunatly, we cannot call this class "Transform"
// As it is a trait defined in cgmath.
#[derive(Debug, PartialEq)]
pub struct MyTransform {
    pub m: Mat4,
    pub m_inv: Mat4,
}

impl MyTransform {
    /// Simple transform constructor which compute the inverse
    #[must_use]
    pub fn new(m: Mat4) -> Self {
        Self {
            m,
            m_inv: m.invert().unwrap(),
        }
    }

    /// Create new transformation based on a given matrix
    /// and its inverse
    #[must_use]
    pub const fn new_with_inverse(m: Mat4, m_inv: Mat4) -> Self {
        Self { m, m_inv }
    }

    /// Return the inverse transformation
    #[must_use]
    pub const fn inverse(&self) -> Self {
        Self {
            m: self.m_inv,
            m_inv: self.m,
        }
    }

    /// Apply the homogenous transformation to a 3D direction vector
    #[must_use]
    pub fn vector(&self, v: &Vec3) -> Vec3 {
        let homogeneous = self.m * Vec4::new(v.x, v.y, v.z, 0.0);
        Vec3::new(homogeneous.x, homogeneous.y, homogeneous.z)
    }

    /// Apply transformation to a 3D normal vector
    #[must_use]
    pub fn normal(&self, n: &Vec3) -> Vec3 {
        let homogeneous = self.m_inv.transpose() * Vec4::new(n.x, n.y, n.z, 0.0);
        Vec3::new(homogeneous.x, homogeneous.y, homogeneous.z)
    }

    /// Apply transformation to a 3D point
    #[must_use]
    pub fn point(&self, p: &Point3) -> Point3 {
        Point3::from_homogeneous(self.m * p.to_homogeneous())
    }

    #[must_use]
    pub fn ray(&self, r: &Ray) -> Ray {
        Ray::new(&self.point(&r.o), &self.vector(&r.d)).with_range(r.tmin, r.tmax)
    }
}

/// The multiplication of two transformation
impl std::ops::Mul for MyTransform {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Self {
            m: self.m * rhs.m,
            m_inv: self.m_inv * rhs.m_inv,
        }
    }
}

/// Empty transformation
impl Default for MyTransform {
    fn default() -> Self {
        Self {
            m: Mat4::identity(),
            m_inv: Mat4::identity(),
        }
    }
}

/// Read the JSON given transform or use the identity transform
#[must_use]
pub fn json_to_transform(json: &HashMap<String, JsonValue>, name: &str) -> MyTransform {
    if json.contains_key(name) {
        MyTransform::new(json_to_mat4(json, name).unwrap())
    } else {
        MyTransform::default()
    }
}
