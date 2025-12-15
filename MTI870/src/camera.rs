use std::collections::HashMap;

use cgmath::{Angle, InnerSpace, SquareMatrix, Zero};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};
use tinyjson::JsonValue; // NEW

use crate::{
    deg2rad,
    json::{json_to_f64, json_to_mat4, json_to_vec2i},
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
    // Thin-lens basis on the aperture plane (world space)
    lens_u: Vec3, // camera right (unit)
    lens_v: Vec3, // camera up (unit)
}

impl CameraPerspective {
    pub fn from_json(json: &HashMap<String, JsonValue>) -> CameraPerspective {
        let transform =
            MyTransform::new(json_to_mat4(json, "transform").unwrap_or(Mat4::identity()));
        let resolution = json_to_vec2i(json, "resolution", Vec2i::new(512, 512));
        let aspect_ratio = resolution.x as f64 / resolution.y as f64;

        let fdist = json_to_f64(json, "fdist", 1.0);
        let lens_radius = json_to_f64(json, "aperture", 0.0) / 2.0;
        let vfov = json_to_f64(json, "vfov", 90.0);

        /*
        TODO: Calculer la taille du plan image
        en fonction de l'ouverture de la caméra.

        Suivre les explications de la section 11 de "Ray Tracing in One Weekend":
        https://raytracing.github.io/books/RayTracingInOneWeekend.html#positionablecamera

        Attention, vfov est spécifié en degrés. Utiliser deg2rad pour
        transformer cette valeur en radians avant d'utiliser des fonctions
        trigonométriques.
        */

        //let viewport_height = 2.0; // TODO: Changer ici
        let viewport_height = 2.0 * (deg2rad(vfov) / 2.0).tan() * fdist;
        let viewport_width = aspect_ratio * viewport_height;

        /*
        TODO: Copier une partie de votre mise en œuvre
        effectuée dans la première tâche du devoir 1.
        Utiliser "m_transform" pour transformer les vecteurs
        et points calculés lors de votre première tâche du devoir 1.
        */

        let mut origin = Point3::new(0.0, 0.0, 0.0);
        // Rappelez vous, dans l'espace local, l'origin de la camera est en (0,0,0)
        //let horizontal = Vec3::zero(); // Vecteur horizontal definissant le plan image
        //let vertical = Vec3::zero(); // Vecteur vertial deffinissant le plan image
        // Le point 3D correspondant au point en haut a gauche sur le plan image
        //let up_left_corner = Point3::new(0.0, 0.0, 0.0);

        //votrecodeici!("Devoir 1: construction de la camera perspective");
        let u = Vec3::new(viewport_width, 0.0, 0.0);
        let v = Vec3::new(0.0, -viewport_height, 0.0);
        let mut horizontal = u / resolution.x as f64;
        let mut vertical = v / resolution.y as f64;
        let viewport_upper_left = origin - Vec3::new(0.0, 0.0, fdist) - u / 2.0 - v / 2.0;
        let mut up_left_corner = viewport_upper_left + 0.5 * (horizontal + vertical);

        origin = MyTransform::point(&transform, &origin);
        horizontal = MyTransform::vector(&transform, &horizontal);
        vertical = MyTransform::vector(&transform, &vertical);
        up_left_corner = MyTransform::point(&transform, &up_left_corner);

        // Build lens basis from LookAt axes via Transform
        let lens_u = transform.vector(&Vec3::new(1.0, 0.0, 0.0)).normalize();
        let lens_v = transform.vector(&Vec3::new(0.0, 1.0, 0.0)).normalize();

        CameraPerspective {
            resolution: Vec2u::new(resolution.x as u32, resolution.y as u32),
            transform,
            fdist,
            lens_radius,
            origin,
            up_left_corner,
            horizontal,
            vertical,
            lens_u,
            lens_v,
        }
    }

    // Concentric disk mapping (Shirley-Chiu) ChadGPT helped for this one
    fn sample_unit_disk(sampler: &mut dyn Sampler) -> (f64, f64) {
        // Adapte au trait que tu as: next_1d() ou autre; remplace si besoin.
        let u1 = sampler.next();
        let u2 = sampler.next();

        // Map [0,1)^2 -> [-1,1]^2
        let mut sx = 2.0 * u1 - 1.0;
        let mut sy = 2.0 * u2 - 1.0;

        if sx == 0.0 && sy == 0.0 {
            return (0.0, 0.0);
        }

        let (r, theta) = if sx.abs() > sy.abs() {
            let r = sx;
            let theta = FRAC_PI_4 * (sy / sx);
            (r, theta)
        } else {
            let r = sy;
            let theta = FRAC_PI_2 - FRAC_PI_4 * (sx / sy);
            (r, theta)
        };

        (r * theta.cos(), r * theta.sin())
    }

    // pub fn generate_ray(&self, pos_img: &Vec2, sampler: &mut dyn Sampler) -> Ray {
    //     /*
    //     TODO: Copier une partie de votre mise en oeuvre
    //     effectuer dans la première tâche du devoir 1.

    //     Attention, pos_img est un vecteur 2D avec des valeurs entre (0, 0) et (m_resolution.x, m_resolution.y).
    //     */
    //     //votrecodeici!("Devoir 1: generation d'un rayon pour une camera perspective");
    //     //Ray::default()

    //     let ray_origin = self.up_left_corner
    //         + 0.5 * (self.horizontal + self.vertical)
    //         + (pos_img.x as f64 * self.horizontal)
    //         + (pos_img.y as f64 * self.vertical);

    //     let ray_direction = (ray_origin - self.origin).normalize();

    //     Ray::new(&self.origin, &ray_direction)
    // }

    pub fn generate_ray(&self, pos_img: &Vec2, sampler: &mut dyn Sampler) -> Ray {
        // Point on focal/image plane for this pixel sample (world space)
        let p_focal = self.up_left_corner
            + 0.5 * (self.horizontal + self.vertical)
            + (pos_img.x as f64 * self.horizontal)
            + (pos_img.y as f64 * self.vertical);

        if self.lens_radius > 0.0 {
            let (dx, dy) = Self::sample_unit_disk(sampler);
            let lens_offset = (self.lens_u * dx + self.lens_v * dy) * self.lens_radius;

            let ray_origin = self.origin + lens_offset;
            let ray_dir = (p_focal - ray_origin).normalize();
            Ray::new(&ray_origin, &ray_dir)
        } else {
            // Pinhole fallback
            let ray_dir = (p_focal - self.origin).normalize();
            Ray::new(&self.origin, &ray_dir)
        }
    }
}
