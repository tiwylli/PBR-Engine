use crate::{
    ray::Ray,
    vec::{Point3, Vec3},
};

#[derive(Clone, Debug)]
pub struct AABB {
    pub min: Point3,
    pub max: Point3,
}

impl Default for AABB {
    fn default() -> Self {
        let min = Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let max = Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        Self { min, max }
    }
}

impl AABB {
    #[must_use]
    pub const fn from_points(p1: Point3, p2: Point3) -> Self {
        let min = Point3::new(p1.x.min(p2.x), p1.y.min(p2.y), p1.z.min(p2.z));
        let max = Point3::new(p1.x.max(p2.x), p1.y.max(p2.y), p1.z.max(p2.z));
        Self { min, max }
    }

    pub const fn extend(&mut self, v: Point3) {
        self.min.x = self.min.x.min(v.x);
        self.min.y = self.min.y.min(v.y);
        self.min.z = self.min.z.min(v.z);

        self.max.x = self.max.x.max(v.x);
        self.max.y = self.max.y.max(v.y);
        self.max.z = self.max.z.max(v.z);
    }

    /// Calcul de l'intersection avec le volume englobant
    /// <http://psgraphics.blogspot.de/2016/02/new-simple-ray-box-test-from-andrew.html>
    #[must_use]
    pub fn hit(&self, r: &Ray) -> Option<f64> {
        // Initialise with ray segments
        let mut t_max = r.tmax;
        let mut t_min = r.tmin;

        // For the different dimensions
        for d in 0..3 {
            // Inverse ray distance (can be optimized if this information is cached inside the ray structure)
            let inv_d = 1.0 / r.d[d];

            // Same formula showed in class
            let mut t0 = (self.min[d] - r.o[d]) * inv_d;
            let mut t1 = (self.max[d] - r.o[d]) * inv_d;

            // In case if the direction is inverse:
            // we will hit the plane "max" before "min"
            // so we will swap the two distance so t0 is always the minimum
            if inv_d < 0.0 {
                // The ray goes in the reverse direction
                std::mem::swap(&mut t0, &mut t1);
            }

            // Min/Max updates (std::min, std::max)
            t_min = if t0 > t_min { t0 } else { t_min };
            t_max = if t1 < t_max { t1 } else { t_max };

            // Misses the AABB (max distance)
            if t_max <= t_min {
                return None;
            }
        }

        Some(t_min)
    }

    #[must_use]
    pub fn center(&self) -> Point3 {
        Point3::new(
            self.min.x + self.max.x,
            self.min.y + self.max.y,
            self.min.z + self.max.z,
        ) / 2.0
    }

    #[must_use]
    pub fn diagonal(&self) -> Vec3 {
        self.max - self.min
    }

    #[must_use]
    #[allow(clippy::suboptimal_flops)]
    pub fn area(&self) -> f64 {
        let e = self.diagonal();
        2.0 * (e.x * e.y + e.y * e.z + e.z * e.x)
    }
}

#[must_use]
pub const fn merge_aabb(a: &AABB, b: &AABB) -> AABB {
    AABB {
        min: Point3::new(
            a.min.x.min(b.min.x),
            a.min.y.min(b.min.y),
            a.min.z.min(b.min.z),
        ),
        max: Point3::new(
            a.max.x.max(b.max.x),
            a.max.y.max(b.max.y),
            a.max.z.max(b.max.z),
        ),
    }
}

#[must_use]
pub fn intersect_aabb(a: &AABB, b: &AABB) -> Option<AABB> {
    let min = Point3::new(
        a.min.x.max(b.min.x),
        a.min.y.max(b.min.y),
        a.min.z.max(b.min.z),
    );
    let max = Point3::new(
        a.max.x.min(b.max.x),
        a.max.y.min(b.max.y),
        a.max.z.min(b.max.z),
    );

    if max.x < min.x || max.y < min.y || max.z < min.z {
        None
    } else {
        Some(AABB { min, max })
    }
}
