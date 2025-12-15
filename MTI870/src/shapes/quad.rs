use std::{collections::HashMap, sync::Arc};

use cgmath::InnerSpace;
use tinyjson::JsonValue;
use cgmath::Array;
use super::{Intersection, Shape};
use crate::{
    NUMBER_INTERSECTIONS, aabb::AABB, json::json_to_vec2, materials::Material, ray::Ray, transform::MyTransform, vec::{Point3, Vec2, Vec3}
};

pub struct Quad {
    half_size: Vec2,
    transform: MyTransform,
    material: Arc<dyn Material>,
}

impl Quad {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        transform: MyTransform,
        material: Arc<dyn Material>,
    ) -> Quad {
        let half_size = json_to_vec2(json, "size", Vec2::new(1.0, 1.0)) / 2.0;
        Quad {
            half_size,
            transform,
            material,
        }
    }
}

impl Shape for Quad {
    fn hit<'a>(&'a self, r: &Ray) -> Option<Intersection<'a>> {
        NUMBER_INTERSECTIONS.with(|f| *f.borrow_mut() += 1);
        // First we transform the ray defined in world space
        // to the local space
        let ray = self.transform.inverse().ray(r);

        // If the ray direction is parallel to the plane
        // no intersection can happens
        if ray.d.z == 0.0 {
            return None; // No intersection
        }

        // Compute the intersection distance
        let t = -ray.o.z / ray.d.z;
        // If the intersection distance is outside
        // the ray bounds, there is no intersection
        if t < ray.tmin || ray.tmax < t {
            return None; // no intersection
        }

        // Use the distance to compute the intersection position
        // this position is defined in local space
        let p = ray.point_at(t);
        // Check this the x and y component of this intersection point
        // is interior of the quad
        if p.x.abs() > self.half_size.x || p.y.abs() > self.half_size.y {
            return None; // no intersection, outside the quad
        }

        // Trick: force the point to be on the plane
        // This operation is not mandatory but help to
        // get a more precise intersection point
        let p = Point3::new(p.x, p.y, 0.0);

        // Finally, we transform all intersection
        // information from local space to world space
        let t = t; // Distance to not change
        let p = self.transform.point(&p); // Transform the intersection point
        let n = self.transform.normal(&Vec3::new(0.0, 0.0, 1.0)).normalize(); // Transform the local normal
        let material = self.material.as_ref();

        // We found an intersection!
        Some(Intersection {
            t,
            p,
            n,
            material,
            shape: self,
        })
    }

    fn sample_direct(&self, p: &Point3, sample: &Vec2) -> (super::EmitterSample, &dyn Shape) {
        // Sample a point uniformly on the rectangle in local space
        // Parametrization: x in [-half_size.x, half_size.x], y in [-half_size.y, half_size.y]
        let sx = (sample.x - 0.5) * 2.0 * self.half_size.x;
        let sy = (sample.y - 0.5) * 2.0 * self.half_size.y;
        let p_local = Point3::new(sx, sy, 0.0);

        // Transform to world
        let y = self.transform.point(&p_local);
        let n_local = Vec3::new(0.0, 0.0, 1.0);
        let n_world = self.transform.normal(&n_local).normalize();

        // Surface area of the quad
        let area = (self.half_size.x * 2.0) * (self.half_size.y * 2.0);
        let p_a = 1.0 / area; // PDF per unit area

        // Convert to solid angle PDF: p_omega = p_a * d^2 / |cos(theta)|
        let d = y - *p;
        let dist2 = d.magnitude2();
        let dist = dist2.sqrt();
        let dir = d / dist.max(1e-12);
        let cos_theta = n_world.dot(dir).abs().max(1e-12);
        let p_omega = p_a * dist2 / cos_theta;

        let mut es = super::EmitterSample::new();
        es.y = y;
        es.n = n_world;
        es.pdf = p_omega;
        return (es, self);
    }

    fn pdf_direct(&self, shape: &dyn Shape, p: &Point3, y: &Point3, n: &Vec3) -> crate::Real {
        // If the sampled shape is not this quad, return 0
        // For simple shapes we expect shape to be the same instance
        // Ensure the provided shape reference matches this shape instance
        // if !std::ptr::eq(self as &dyn Shape, shape) {
        //     return 0.0;
        // }

        // Compute surface PDF p_A = 1 / area
        let area = (self.half_size.x * 2.0) * (self.half_size.y * 2.0);
        let p_a = 1.0 / area;

        // Convert to solid angle PDF: p_omega = p_a * d^2 / |cos(theta)|
        let dvec = *y - *p;
        let dist2 = dvec.magnitude2();
        let dist = dist2.sqrt();
        let dir = dvec / dist.max(1e-12);
        let cos_theta = n.dot(dir).abs().max(1e-12);
        let p_omega = p_a * dist2 / cos_theta;
        return p_omega;
    }

    fn material(&self) -> &dyn Material {
        self.material.as_ref()
    }

    fn add_shape(&mut self, _: Box<dyn Shape>) {}
    fn build(&mut self) {}
    fn aabb(&self) -> AABB {
        AABB::from_points(
            self.transform.point(
                &(Point3::new(-self.half_size.x, -self.half_size.y, 0.0)
                    - Vec3::from_value(0.0001)),
            ),
            self.transform.point(
                &(Point3::new(self.half_size.x, self.half_size.y, 0.0) + Vec3::from_value(0.0001)),
            ),
        )
    }

}
