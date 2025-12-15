use std::{collections::HashMap, sync::Arc};

use cgmath::InnerSpace;
use tinyjson::JsonValue;

use crate::{
    aabb::AABB,
    json::json_to_f64,
    materials::Material,
    transform::{json_to_transform, MyTransform},
    vec::{Point3, Vec3},
};

use super::{
    raymarch::RaymarchSettings,
    sdf_object::{json_to_bounds, parse_object_settings, SDFObject},
};

fn sd_box(p: Vec3, half_extent: Vec3) -> f64 {
    let q = Vec3::new(
        p.x.abs() - half_extent.x,
        p.y.abs() - half_extent.y,
        p.z.abs() - half_extent.z,
    );

    let outside = Vec3::new(q.x.max(0.0), q.y.max(0.0), q.z.max(0.0));
    let inside = q.x.max(q.y.max(q.z)).min(0.0);
    outside.magnitude() + inside
}

fn menger_distance(p: Vec3, iterations: u32) -> f64 {
    let mut d = sd_box(p, Vec3::new(1.0, 1.0, 1.0));
    let mut scale = 1.0;

    for _ in 0..iterations {
        let cell = Vec3::new(
            (p.x * scale).rem_euclid(2.0) - 1.0,
            (p.y * scale).rem_euclid(2.0) - 1.0,
            (p.z * scale).rem_euclid(2.0) - 1.0,
        );

        scale *= 3.0;

        let r = Vec3::new(
            1.0 - 3.0 * cell.x.abs(),
            1.0 - 3.0 * cell.y.abs(),
            1.0 - 3.0 * cell.z.abs(),
        );

        let c = r.x.max(r.y).min(r.y.max(r.z));
        d = d.max(c / scale);
    }

    d
}

/// Distance estimator for a classic Menger sponge fractal cube.
pub struct SdfMenger {
    object_to_world: MyTransform,
    world_to_object: MyTransform,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
    scale: f64,
    iterations: u32,
}

impl SdfMenger {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let transform = json_to_transform(json, "transform");
        let inverse = transform.inverse();

        let scale = json_to_f64(json, "scale", 0.25).max(1.0e-4);
        let iterations = json_to_f64(json, "iterations", 4.0).max(0.0) as u32;

        let bounds = json_to_bounds(json).unwrap_or_else(|| {
            AABB::from_points(
                Point3::new(-scale, -scale, -scale),
                Point3::new(scale, scale, scale),
            )
        });

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("Menger sponge `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{}` for Menger sponge", name)),
            )
        });

        let settings = parse_object_settings(json);

        Self {
            object_to_world: transform,
            world_to_object: inverse,
            bounds,
            material,
            settings,
            scale,
            iterations,
        }
    }
}

impl SDFObject for SdfMenger {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let local = self.world_to_object.point(&world_p);
        let p = Vec3::new(local.x, local.y, local.z) / self.scale;
        menger_distance(p, self.iterations) * self.scale
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
