use std::{collections::HashMap, f64, sync::Arc};

use super::{Intersection, Shape};
use crate::{
    NUMBER_INTERSECTIONS, Real,
    aabb::AABB,
    json::json_to_vec2,
    materials::Material,
    ray::Ray,
    shapes::{EmitterSample, surfacial_to_solid_angle},
    transform::MyTransform,
    vec::{Point3, Vec2, Vec3},
};
use cgmath::Array;
use cgmath::InnerSpace;
use tinyjson::JsonValue;

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
    ) -> Self {
        let half_size = json_to_vec2(json, "size", Vec2::new(1.0, 1.0)) / 2.0;
        Self {
            half_size,
            transform,
            material,
        }
    }
}

impl Shape for Quad {
    #[allow(clippy::many_single_char_names)]
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
        let local_p = Point3::new(p.x, p.y, 0.0);
        let denom_x = self.half_size.x * 2.0;
        let denom_y = self.half_size.y * 2.0;
        let u = if denom_x.abs() > f64::EPSILON {
            (local_p.x + self.half_size.x) / denom_x
        } else {
            0.5
        };
        let v = if denom_y.abs() > f64::EPSILON {
            (local_p.y + self.half_size.y) / denom_y
        } else {
            0.5
        };
        let uv = Vec2::new(u, v);

        // Finally, we transform all intersection
        // information from local space to world space
        // let t = t; // Distance to not change
        let p = self.transform.point(&local_p); // Transform the intersection point
        let mut n_local = self.material.get_normal_map_value(&uv, &p);
        if n_local.dot(-ray.d) < 0.0 {
            n_local = Vec3::unit_z();
        }
        let n = self.transform.normal(&n_local); // Transform the local normal
        let material = self.material.as_ref();

        // We found an intersection!
        Some(Intersection {
            t,
            p,
            n,
            uv,
            material,
            shape: self,
        })
    }

    fn sample_direct(&self, p: &Point3, sample: &Vec2) -> (EmitterSample, &dyn Shape) {
        let local = Point3::new(
            sample.x.mul_add(2.0, -1.0) * self.half_size.x,
            sample.y.mul_add(2.0, -1.0) * self.half_size.y,
            0.0,
        );
        let world = self.transform.point(&local);

        let n = self.transform.normal(&Vec3::unit_z()).normalize();
        (
            EmitterSample {
                y: world,
                n,
                pdf: self.pdf_direct(self, p, &world, &n),
            },
            self,
        )
    }

    fn pdf_direct(&self, _shape: &dyn Shape, p: &Point3, y: &Point3, n: &Vec3) -> Real {
        let pdf = 1.0 / (self.half_size.x * self.half_size.y * 4.0);
        surfacial_to_solid_angle(pdf, p, y, n)
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
