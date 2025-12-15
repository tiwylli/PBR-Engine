use std::{collections::HashMap, sync::Arc};

use cgmath::InnerSpace;
use tinyjson::JsonValue;

use crate::{
    aabb::AABB,
    json::{json_to_f64, json_to_vec3},
    materials::Material,
    transform::{json_to_transform, MyTransform},
    vec::{Point3, Vec3},
};

use super::{
    raymarch::RaymarchSettings,
    sdf_object::{json_to_bounds, parse_object_settings, SDFObject},
};

/// Sphere SDF whose surface is warped using Mandelbox-style folds.
pub struct SdfMandelMorph {
    object_to_world: MyTransform,
    world_to_object: MyTransform,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
    base_radius: f64,
    fold_scale: f64,
    min_radius: f64,
    fixed_radius: f64,
    box_limit: f64,
    translate: Vec3,
    iterations: u32,
    warp_strength: f64,
}

impl SdfMandelMorph {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let transform = json_to_transform(json, "transform");
        let inverse = transform.inverse();
        let bounds = json_to_bounds(json).unwrap_or_else(|| {
            AABB::from_points(Point3::new(-3.0, -3.0, -3.0), Point3::new(3.0, 3.0, 3.0))
        });

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("Mandelmorph `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{}` for Mandelmorph", name)),
            )
        });

        let settings = parse_object_settings(json);
        let base_radius = json_to_f64(json, "base_radius", 1.0);
        let fold_scale = json_to_f64(json, "fold_scale", 2.0);
        let min_radius = json_to_f64(json, "min_radius", 0.5);
        let fixed_radius = json_to_f64(json, "fixed_radius", 1.0);
        let box_limit = json_to_f64(json, "box_limit", 1.0).max(1.0e-3);
        let translate = json_to_vec3(json, "translate", Vec3::new(0.5, 0.5, 0.0));
        let iterations = json_to_f64(json, "iterations", 6.0).max(1.0) as u32;
        let warp_strength = json_to_f64(json, "warp_strength", 0.3).clamp(0.0, 1.0);

        Self {
            object_to_world: transform,
            world_to_object: inverse,
            bounds,
            material,
            settings,
            base_radius,
            fold_scale,
            min_radius,
            fixed_radius,
            box_limit,
            translate,
            iterations,
            warp_strength,
        }
    }
}

impl SDFObject for SdfMandelMorph {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let local = self.world_to_object.point(&world_p);
        let original = Vec3::new(local.x, local.y, local.z);
        let mut p = original;

        for _ in 0..self.iterations {
            // Box fold
            p.x = (p.x).clamp(-self.box_limit, self.box_limit) * 2.0 - p.x;
            p.y = (p.y).clamp(-self.box_limit, self.box_limit) * 2.0 - p.y;
            p.z = (p.z).clamp(-self.box_limit, self.box_limit) * 2.0 - p.z;

            // Sphere fold
            let r = p.magnitude();
            let min_r = self.min_radius;
            let fixed_r = self.fixed_radius;

            let r = r.max(1.0e-6);
            if r < min_r {
                p = p * (fixed_r / min_r);
            } else if r < fixed_r {
                p = p * (fixed_r / r);
            }

            p = p * self.fold_scale + self.translate;
        }

        let warped = original + (p - original) * self.warp_strength;
        warped.magnitude() - self.base_radius
    }

    fn object_to_world(&self) -> &MyTransform {
        &self.object_to_world
    }

    fn world_bounds(&self) -> AABB {
        self.bounds.clone()
    }

    fn material(&self) -> Option<Arc<dyn Material>> {
        self.material.as_ref().map(Arc::clone)
    }

    fn custom_settings(&self) -> Option<RaymarchSettings> {
        self.settings
    }
}
