use cgmath::Zero;

use crate::{
    constants::RAY_EPS,
    vec::{Point3, Vec3},
};

#[derive(Clone, Copy, Debug)]
pub struct Ray {
    /// ray origin
    pub o: Point3,
    /// ray direction
    pub d: Vec3,
    /// ray minimal distance
    pub tmin: f64,
    /// ray maximum distance
    pub tmax: f64,
}

impl Ray {
    pub fn new(origin: &Point3, direction: &Vec3) -> Ray {
        Self {
            o: *origin,
            d: *direction,
            tmin: RAY_EPS,
            tmax: std::f64::MAX,
        }
    }

    pub fn with_range(mut self, tmin: f64, tmax: f64) -> Self {
        self.tmax = tmax;
        self.tmin = tmin;
        self
    }

    pub fn with_distance_max(mut self, tmax: f64) -> Self {
        self.tmax = tmax;
        self
    }

    pub fn point_at(&self, t: f64) -> Point3 {
        self.o + t * self.d
    }
}

/// Construct an empty ray
impl Default for Ray {
    fn default() -> Self {
        Self {
            o: Point3::new(0.0, 0.0, 0.0),
            d: Vec3::zero(),
            tmin: RAY_EPS,
            tmax: std::f64::MAX,
        }
    }
}
