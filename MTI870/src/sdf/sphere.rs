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

/// Signed-distance sphere with optional transform/material overrides.
pub struct SdfSphere {
    object_to_world: MyTransform,
    world_to_object: MyTransform,
    radius: f64,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
}

impl SdfSphere {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let transform = json_to_transform(json, "transform");
        let inverse = transform.inverse();
        let radius = json_to_f64(json, "radius", 1.0);
        let bounds = json_to_bounds(json).expect("SDF sphere requires a `bounds` entry");

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("SDF sphere `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{}` for SDF sphere", name)),
            )
        });

        let settings = parse_object_settings(json);

        Self {
            object_to_world: transform,
            world_to_object: inverse,
            radius,
            bounds,
            material,
            settings,
        }
    }
}

impl SDFObject for SdfSphere {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let local = self.world_to_object.point(&world_p);
        let v = Vec3::new(local.x, local.y, local.z);
        v.magnitude() - self.radius
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
