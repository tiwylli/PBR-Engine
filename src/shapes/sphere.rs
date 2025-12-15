use std::{collections::HashMap, f64, sync::Arc};

use cgmath::{EuclideanSpace, InnerSpace, Matrix3, MetricSpace};
use tinyjson::JsonValue;

use crate::{
    NUMBER_INTERSECTIONS, Real,
    aabb::AABB,
    json::{json_to_bool, json_to_f64},
    materials::Material,
    ray::Ray,
    samplers::{pdf_cone_cos_theta_max, sample_cone_cos_theta_max, sample_spherical},
    shapes::{EmitterSample, surfacial_to_solid_angle},
    transform::MyTransform,
    vec::{Frame, Point3, Vec2, Vec3},
};

use super::{Intersection, Shape};

pub struct Sphere {
    radius: f64,
    transform: MyTransform,
    material: Arc<dyn Material>,
    solid_angle_sampling: bool,
}

impl Sphere {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        transform: MyTransform,
        material: Arc<dyn Material>,
    ) -> Self {
        let radius = json_to_f64(json, "radius", 1.0);
        let solid_angle_sampling = json_to_bool(json, "solid_angle_sampling", false);
        Self {
            radius,
            transform,
            material,
            solid_angle_sampling,
        }
    }
}

impl Shape for Sphere {
    #[allow(clippy::many_single_char_names)]
    fn hit<'a>(&'a self, r: &Ray) -> Option<Intersection<'a>> {
        NUMBER_INTERSECTIONS.with(|f| *f.borrow_mut() += 1);
        let r = self.transform.inverse().ray(r);
        let a = r.d.dot(r.d);
        let b = 2.0 * r.d.dot(r.o.to_vec());
        let c = self.radius.mul_add(-self.radius, r.o.dot(r.o.to_vec()));
        let det = b.mul_add(b, -(4.0 * a * c));
        if det >= 0.0 {
            let sqrt = det.sqrt();
            let t1 = (-b - sqrt) / (2.0 * a);
            let t2 = (-b + sqrt) / (2.0 * a);

            let t = if t1 >= r.tmin && t1 <= r.tmax {
                t1
            } else if t2 >= r.tmin && t2 <= r.tmax {
                t2
            } else {
                return None;
            };
            let p = r.point_at(t);
            let base_n = (p - Point3::origin()) / self.radius;
            let phi = (p.z / self.radius).atan2(p.x / self.radius);
            let u = (f64::consts::PI + phi) / (2.0 * f64::consts::PI);
            let theta = (p.y / self.radius).acos();
            let v = (f64::consts::PI - theta) / f64::consts::PI;
            let uv = Vec2::new(u, v);

            let n = if self.material.have_normal_map() {
                let tangent = Vec3::new(-phi.sin(), 0.0, phi.cos()).normalize();
                let bitangent = Vec3::new(
                    theta.cos() * phi.cos(),
                    -theta.sin(),
                    theta.cos() * phi.sin(),
                )
                .normalize();
                let tbn = Matrix3::from_cols(tangent, bitangent, base_n);
                let n = tbn * self.material.get_normal_map_value(&uv, &p);
                if n.dot(-r.d) < 0.0 { base_n } else { n }
            } else {
                base_n
            };
            Some(Intersection {
                t,
                p: self.transform.point(&p),
                n: self.transform.normal(&n).normalize(),
                uv,
                material: self.material.as_ref(),
                shape: self,
            })
        } else {
            None
        }
    }

    fn sample_direct(&self, p: &Point3, sample: &Vec2) -> (EmitterSample, &dyn Shape) {
        if self.solid_angle_sampling {
            let c = self.transform.point(&Point3::origin());
            let d2 = p.distance2(c);
            let r2 = self.radius * self.radius;
            if r2 < d2 {
                let cos_theta_max = ((d2 - r2) / d2).sqrt();
                let frame = Frame::new(&(c - p).normalize());
                let dir_local = sample_cone_cos_theta_max(sample, cos_theta_max);
                let dir_world = frame.to_world(&dir_local);
                if let Some(intersection) = self.hit(&Ray::new(p, &dir_world)) {
                    return (
                        EmitterSample {
                            y: intersection.p,
                            n: intersection.n,
                            // pdf: self.pdf_direct(self, p, &intersection.p, &intersection.n),
                            pdf: pdf_cone_cos_theta_max(&dir_local, cos_theta_max),
                        },
                        self,
                    );
                }
            }
            panic!("Tried to sample point from inside sphere or intersection failed");
        } else {
            let local_v = self.radius * sample_spherical(sample);
            let local = Point3::from_vec(local_v);
            let world = self.transform.point(&local);
            let n = self.transform.normal(&local_v).normalize();
            (
                EmitterSample {
                    y: world,
                    n,
                    pdf: self.pdf_direct(self, p, &world, &n),
                },
                self,
            )
        }
    }

    fn pdf_direct(&self, _shape: &dyn Shape, p: &Point3, y: &Point3, n: &Vec3) -> Real {
        if self.solid_angle_sampling {
            let c = self.transform.point(&Point3::origin());
            let d2 = p.distance2(c);
            let r2 = self.radius * self.radius;
            let cos_theta_max = ((d2 - r2) / d2).sqrt();
            let frame = Frame::new(&(c - p).normalize());
            let dir = frame.to_local(&(y - p).normalize());
            pdf_cone_cos_theta_max(&dir, cos_theta_max)
        } else {
            let pdf = 1.0 / (self.radius * self.radius * 4.0 * f64::consts::PI);
            surfacial_to_solid_angle(pdf, p, y, n)
        }
    }

    fn material(&self) -> &dyn Material {
        self.material.as_ref()
    }

    fn add_shape(&mut self, _: Box<dyn Shape>) {}

    fn build(&mut self) {}

    #[allow(clippy::suboptimal_flops)]
    fn aabb(&self) -> AABB {
        let center = self.transform.point(&Point3::new(0.0, 0.0, 0.0));

        // columns of the linear part of the transform
        let cx = self.transform.vector(&Vec3::new(1.0, 0.0, 0.0));
        let cy = self.transform.vector(&Vec3::new(0.0, 1.0, 0.0));
        let cz = self.transform.vector(&Vec3::new(0.0, 0.0, 1.0));

        // row norms times radius -> half-extent per world axis
        let ex = (cx.x * cx.x + cy.x * cy.x + cz.x * cz.x).sqrt() * self.radius;
        let ey = (cx.y * cx.y + cy.y * cy.y + cz.y * cz.y).sqrt() * self.radius;
        let ez = (cx.z * cx.z + cy.z * cy.z + cz.z * cz.z).sqrt() * self.radius;

        let half_extents = Vec3::new(ex, ey, ez);
        AABB::from_points(center - half_extents, center + half_extents)
    }
}
