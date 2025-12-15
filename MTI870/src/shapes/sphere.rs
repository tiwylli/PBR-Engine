use std::{collections::HashMap, sync::Arc};

use crate::{
    aabb::AABB,
    constants,
    json::{json_to_bool, json_to_f64},
    vec::{sample_cone, sample_spherical, Frame, Point3, Vec2, Vec3},
    NUMBER_INTERSECTIONS,
};
use cgmath::{dot, Array, EuclideanSpace, InnerSpace};
use tinyjson::JsonValue;

use crate::{materials::Material, ray::Ray, transform::MyTransform};

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
    ) -> Sphere {
        let radius = json_to_f64(json, "radius", 1.0);
        let solid_angle_sampling = json_to_bool(json, "use_sphere_solid_angle_sampling", false);
        Sphere {
            radius,
            transform,
            material,
            solid_angle_sampling,
        }
    }

    fn world_center_radius(&self) -> (Point3, f64) {
        let center = self.transform.point(&Point3::new(0.0, 0.0, 0.0));
        let unit = self.transform.vector(&Vec3::new(1.0, 0.0, 0.0)).magnitude();
        (center, self.radius * unit)
    }

    fn sample_surface_direct(&self, p: &Point3, sample: &Vec2) -> super::EmitterSample {
        let dir_local: Vec3 = sample_spherical(sample);
        let p_local: Point3 = Point3::from_vec(dir_local * self.radius);

        let y: Point3 = self.transform.point(&p_local);
        let n_world: Vec3 = self.transform.normal(&dir_local).normalize();

        let area = 4.0 * std::f64::consts::PI * self.radius * self.radius;
        let p_a = 1.0 / area;

        let d: Vec3 = y - *p;
        let dist2 = d.magnitude2();
        let dist = dist2.sqrt();
        let dir = if dist > 0.0 {
            d / dist
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        let cos_theta = n_world.dot(dir).abs().max(1e-12);
        let p_omega = p_a * dist2 / cos_theta;

        let mut es = super::EmitterSample::new();
        es.y = y;
        es.n = n_world;
        es.pdf = p_omega;
        es
    }

    //ChadGPT helped layout code structure and not forget warnings
    fn sample_solid_angle(&self, p: &Point3, sample: &Vec2) -> super::EmitterSample {
        const INSIDE_EPS: f64 = 1.0 + 1e-6;

        let mut es = super::EmitterSample::new();
        let (center, radius) = self.world_center_radius();

        let to_center = center - *p;
        let dist_sq = to_center.magnitude2();
        let dist = dist_sq.sqrt();
        let axis = if dist > 0.0 {
            to_center / dist
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        let frame = Frame::new(&axis);

        let inside = dist <= radius * INSIDE_EPS;
        let (dir_world, pdf) = if inside {
            let wi_local = sample_spherical(sample);
            let wi_world = frame.to_world(&wi_local).normalize();
            (wi_world, constants::INV_FOURPI)
        } else {
            let dist_sq = dist_sq.max(0.0);
            let radius_sq = radius * radius;
            let cos_theta_max = ((dist_sq - radius_sq).max(0.0).sqrt() / dist).clamp(-1.0, 1.0);
            let theta_max = cos_theta_max.acos();
            let wi_local = sample_cone(sample, theta_max);
            let wi_world = frame.to_world(&wi_local).normalize();
            let denom = (1.0 - cos_theta_max).max(1e-12);
            let pdf = constants::INV_TWOPI / denom;
            (wi_world, pdf)
        };

        let shadow_ray = Ray::new(p, &dir_world);
        if let Some(hit) = self.hit(&shadow_ray) {
            es.y = hit.p;
            es.n = hit.n;
            es.pdf = pdf;
        } else {
            es.pdf = 0.0;
        }
        es
    }
}

impl Shape for Sphere {
    fn hit<'a>(&'a self, r: &Ray) -> Option<Intersection<'a>> {
        /*
        TODO: Cacluler l'intersection d'une sphère centrée en [0, 0, 0] et avec un rayon (m_radius).
        Regardez comment est calculer l'intersection d'un plan (disponible dans `render/shape/mod.rs`).
        Référez vous aux diapositives du cours ou "Ray tracing in one weekend" section 5
        https://raytracing.github.io/books/RayTracingInOneWeekend.html#addingasphere

        En regle générale, les étapes sont:
        1) Transformation du rayon en coordonnée locale en utilisant m_transform
        2) Calcul de l'intersection dans ces coordonnées locale
            - Essayer de retourner faux le plus tot possible si il n'y a pas d'intersection
            - Si on est sur de l'intersection, calculer les information de l'intersection (position, distance, normale, ...)
        3) Transformation du point d'intersection en coordonnée monde.
        */

        // Transform the ray defined in world space to the local space
        //info!("{:?},{:?}", r.o, r.d);
        let ray: Ray = self.transform.inverse().ray(r);
        //info!("{:?},{:?}", ray.o, ray.d);

        let center = Point3::new(0.0, 0.0, 0.0);
        let oc = center - ray.o;

        let a = dot(ray.d, ray.d);
        let b = -2.0 * dot(ray.d, oc);
        let c = dot(oc, oc) - (self.radius * self.radius);
        let d = (b * b) - (4.0 * a * c);

        let mut t;
        // if +: 2 racines, if 0 : 1 racines, if - : 0 racines
        if d < 0.0 {
            return None;
        } else if d == 0.0 {
            //1 racine
            t = -b / (2.0 * a);
        } else {
            let tp = (-b + d.sqrt()) / (2.0 * a);
            let tm: f64 = (-b - d.sqrt()) / (2.0 * a);

            t = tm;
            if t < ray.tmin || ray.tmax < t {
                t = tp;
                if t < ray.tmin || ray.tmax < t {
                    return None;
                }
            }
        }

        if t < ray.tmin || ray.tmax < t {
            return None; // no intersection
        }

        let p = ray.point_at(t);
        let n = (ray.point_at(t) - center).normalize();

        // Finally transform all intersection
        // information from local space to world space
        let t = t; // Distance to not change
        let p = self.transform.point(&p); // Transform the intersection point
        let n = self.transform.normal(&n).normalize(); // Transform the local normal
        let material = self.material.as_ref();

        Some(Intersection {
            t,
            p,
            n,
            material,
            shape: self,
        })
    }

    fn sample_direct(
        &self,
        p: &Point3,
        sample: &crate::vec::Vec2,
    ) -> (super::EmitterSample, &dyn Shape) {
        let es = if self.solid_angle_sampling {
            self.sample_solid_angle(p, sample)
        } else {
            self.sample_surface_direct(p, sample)
        };
        (es, self)
    }

    fn pdf_direct(
        &self,
        _shape: &dyn Shape,
        p: &Point3,
        y: &Point3,
        n: &crate::vec::Vec3,
    ) -> crate::Real {
        if !self.solid_angle_sampling {
            let area = 4.0 * std::f64::consts::PI * self.radius * self.radius;
            let p_a = 1.0 / area;
            let d: Vec3 = *y - *p;
            let dist2 = d.magnitude2();
            let dist = dist2.sqrt();
            let dir = if dist > 0.0 {
                d / dist
            } else {
                Vec3::new(0.0, 0.0, 1.0)
            };
            let cos_theta = n.dot(dir).abs().max(1e-12);
            return p_a * dist2 / cos_theta;
        }

        let (center, radius) = self.world_center_radius();
        let to_center = center - *p;
        let dist = to_center.magnitude();
        if dist <= radius * (1.0 + 1e-6) {
            return constants::INV_FOURPI;
        }

        let cos_theta_max =
            ((dist * dist - radius * radius).max(0.0).sqrt() / dist).clamp(-1.0, 1.0);
        let denom = (1.0 - cos_theta_max).max(1e-12);
        constants::INV_TWOPI / denom
    }

    fn material(&self) -> &dyn Material {
        self.material.as_ref()
    }

    fn add_shape(&mut self, _: Box<dyn Shape>) {}
    fn build(&mut self) {}
    fn aabb(&self) -> AABB {
        NUMBER_INTERSECTIONS.with(|f| *f.borrow_mut() += 1);

        let lp = self.transform.point(&Point3::new(0.0, 0.0, 0.0));
        let mx = self
            .transform
            .vector(&Vec3::new(self.radius, 0.0, 0.0))
            .magnitude();
        let my = self
            .transform
            .vector(&Vec3::new(0.0, self.radius, 0.0))
            .magnitude();
        let mz = self
            .transform
            .vector(&Vec3::new(0.0, 0.0, self.radius))
            .magnitude();
        let m = mx.max(my).max(mz);
        AABB::from_points(lp - Vec3::from_value(m), lp + Vec3::from_value(m))
    }
}
