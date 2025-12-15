use std::{collections::HashMap, sync::Arc};

use cgmath::InnerSpace;
use tinyjson::JsonValue;

use crate::{
    aabb::AABB,
    json::json_to_f64,
    materials::Material,
    transform::{MyTransform, json_to_transform},
    vec::{Point3, Vec3},
};

use super::{
    raymarch::RaymarchSettings,
    sdf_object::{SDFObject, json_to_bounds, parse_object_settings},
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

fn sd_menger(mut p: Vec3, half_size: f64, iterations: u32) -> f64 {
    // Normalize so the cube spans [-1, 1] in all axes.
    p /= half_size;

    let mut d = sd_box(p, Vec3::new(1.0, 1.0, 1.0));
    let mut scale = 1.0;
    for _ in 0..iterations {
        // Fold space into the central [-1, 1] cell of a 3x3x3 grid.
        p = Vec3::new(
            (p.x * 3.0).rem_euclid(2.0) - 1.0,
            (p.y * 3.0).rem_euclid(2.0) - 1.0,
            (p.z * 3.0).rem_euclid(2.0) - 1.0,
        );
        scale *= 3.0;

        let r = Vec3::new(
            3.0f64.mul_add(-p.x.abs(), 1.0),
            3.0f64.mul_add(-p.y.abs(), 1.0),
            3.0f64.mul_add(-p.z.abs(), 1.0),
        );
        let da = r.x.abs().max(r.y.abs());
        let db = r.y.abs().max(r.z.abs());
        let dc = r.z.abs().max(r.x.abs());
        let c = (da.min(db.min(dc)) - 1.0) / scale;
        d = d.max(c);
    }

    d * half_size
}

pub struct SdfMengerSponge {
    object_to_world: MyTransform,
    world_to_object: MyTransform,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
    half_size: f64,
    iterations: u32,
}

impl SdfMengerSponge {
    #[must_use]
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let transform = json_to_transform(json, "transform");
        let inverse = transform.inverse();

        let half_size = json_to_f64(json, "half_size", 1.0).max(f64::EPSILON);
        let iterations = json_to_f64(json, "iterations", 4.0).clamp(1.0, 8.0) as u32;

        let bounds = json_to_bounds(json).unwrap_or_else(|| {
            AABB::from_points(
                Point3::new(-half_size, -half_size, -half_size),
                Point3::new(half_size, half_size, half_size),
            )
        });

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("Menger sponge `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{name}` for menger sponge")),
            )
        });

        let settings = parse_object_settings(json);

        Self {
            object_to_world: transform,
            world_to_object: inverse,
            bounds,
            material,
            settings,
            half_size,
            iterations,
        }
    }
}

impl SDFObject for SdfMengerSponge {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let local = self.world_to_object.point(&world_p);
        let p = Vec3::new(local.x, local.y, local.z);
        sd_menger(p, self.half_size, self.iterations)
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

    fn step_scale(&self) -> f64 {
        0.75
    }
}
