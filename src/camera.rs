#![allow(clippy::cast_sign_loss)]

use std::collections::HashMap;

use cgmath::{InnerSpace, SquareMatrix};
use tinyjson::JsonValue;

use crate::{
    deg2rad,
    json::{json_to_f64, json_to_mat4, json_to_vec2i},
    materials::random_in_unit_disk,
    ray::Ray,
    samplers::Sampler,
    transform::MyTransform,
    vec::{Mat4, Point3, Vec2, Vec2i, Vec2u, Vec3},
};

#[derive(Debug)]
pub struct CameraPerspective {
    pub resolution: Vec2u,
    pub transform: MyTransform,
    /// Focal distance (where the plan is placed)
    pub fdist: f64,
    /// Camera opening (if there is depth of field)
    lens_radius: f64,
    /// Vectors to express the camera
    origin: Point3,
    up_left_corner: Point3,
    horizontal: Vec3,
    vertical: Vec3,
}

impl CameraPerspective {
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let transform =
            MyTransform::new(json_to_mat4(json, "transform").unwrap_or_else(Mat4::identity));
        let resolution = json_to_vec2i(json, "resolution", Vec2i::new(512, 512));
        let aspect_ratio = f64::from(resolution.x) / f64::from(resolution.y);

        let fdist = json_to_f64(json, "fdist", 1.0);
        let lens_radius = json_to_f64(json, "aperture", 0.0) / 2.0;
        let vfov = json_to_f64(json, "vfov", 90.0);

        let viewport_height = 2.0 * deg2rad(vfov / 2.0).tan() * fdist;
        let viewport_width = aspect_ratio * viewport_height;

        let origin = Point3::new(0.0, 0.0, 0.0); // Rappelez vous, dans l'espace local, l'origin de la camera est en (0,0,0)
        let horizontal = Vec3::new(viewport_width, 0.0, 0.0); // Vecteur horizontal definissant le plan image
        let vertical = Vec3::new(0.0, -viewport_height, 0.0); // Vecteur vertial deffinissant le plan image
        // Le point 3D correspondant au point en haut a gauche sur le plan image
        let up_left_corner = Point3::new(-viewport_width / 2.0, viewport_height / 2.0, -fdist);

        Self {
            resolution: Vec2u::new(resolution.x as u32, resolution.y as u32),
            transform,
            fdist,
            lens_radius,
            origin,
            up_left_corner,
            horizontal,
            vertical,
        }
    }

    pub fn generate_ray(&self, pos_img: &Vec2, sampler: &mut dyn Sampler) -> Ray {
        let u = (pos_img.x + 0.5) / f64::from(self.resolution.x);
        let v = (pos_img.y + 0.5) / f64::from(self.resolution.y);
        let origin = if self.lens_radius == 0.0 {
            self.origin
        } else {
            self.origin + random_in_unit_disk(sampler) * self.lens_radius
        };
        let direction =
            ((self.up_left_corner + u * self.horizontal + v * self.vertical) - origin).normalize();
        Ray::new(
            &self.transform.point(&origin),
            &self.transform.vector(&direction),
        )
    }

    pub fn scale(&mut self, factor: f32) {
        self.resolution.x = ((self.resolution.x as f32) * factor) as u32;
        self.resolution.y = ((self.resolution.y as f32) * factor) as u32;
    }
}
