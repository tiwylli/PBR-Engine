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

/// Rounded box SDF based on the formulation from IQ.
pub struct SdfRoundBox {
    object_to_world: MyTransform,
    world_to_object: MyTransform,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
    half_extent: Vec3,
    radius: f64,
}

impl SdfRoundBox {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let transform = json_to_transform(json, "transform");
        let inverse = transform.inverse();

        let half_extent = json_to_vec3(json, "half_extent", Vec3::new(1.0, 1.0, 1.0));
        let radius = json_to_f64(json, "radius", 0.25).max(0.0);

        let bounds = json_to_bounds(json).unwrap_or_else(|| {
            let extent = Vec3::new(
                half_extent.x + radius,
                half_extent.y + radius,
                half_extent.z + radius,
            );
            AABB::from_points(
                Point3::new(-extent.x, -extent.y, -extent.z),
                Point3::new(extent.x, extent.y, extent.z),
            )
        });

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("Round box `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{}` for round box", name)),
            )
        });

        let settings = parse_object_settings(json);

        Self {
            object_to_world: transform,
            world_to_object: inverse,
            bounds,
            material,
            settings,
            half_extent,
            radius,
        }
    }
}

impl SDFObject for SdfRoundBox {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let local = self.world_to_object.point(&world_p);
        let p = Vec3::new(local.x, local.y, local.z);

        let q = Vec3::new(
            p.x.abs() - self.half_extent.x + self.radius,
            p.y.abs() - self.half_extent.y + self.radius,
            p.z.abs() - self.half_extent.z + self.radius,
        );

        let q_max = Vec3::new(q.x.max(0.0), q.y.max(0.0), q.z.max(0.0));
        let inner = q.x.max(q.y.max(q.z)).min(0.0);

        q_max.magnitude() + inner - self.radius
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
