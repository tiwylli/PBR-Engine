use std::{collections::HashMap, sync::Arc};

use cgmath::InnerSpace;
use tinyjson::JsonValue;

use crate::{
    aabb::AABB,
    json::{json_to_f64, json_to_vec3},
    materials::Material,
    transform::{MyTransform, json_to_transform},
    vec::{Point3, Vec3},
};

use super::{
    raymarch::RaymarchSettings,
    sdf_object::{SDFObject, json_to_bounds, parse_object_settings},
};

/// Infinite plane SDF that can act as a floor or ceiling.
pub struct SdfPlane {
    object_to_world: MyTransform,
    world_to_object: MyTransform,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
    normal: Vec3,
    offset: f64,
}

impl SdfPlane {
    #[must_use]
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let transform = json_to_transform(json, "transform");
        let inverse = transform.inverse();
        let bounds = json_to_bounds(json).unwrap_or_else(|| {
            // Reasonable default covering a big slab around the origin.
            AABB::from_points(
                Point3::new(-1000.0, -1.0, -1000.0),
                Point3::new(1000.0, 1.0, 1000.0),
            )
        });

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("Plane `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{name}` for plane")),
            )
        });

        let settings = parse_object_settings(json);
        let normal_input = json_to_vec3(json, "normal", Vec3::new(0.0, 1.0, 0.0));
        let normal = if normal_input.magnitude2() <= f64::EPSILON {
            Vec3::new(0.0, 1.0, 0.0)
        } else {
            normal_input.normalize()
        };
        let offset = json_to_f64(json, "offset", 0.0);

        Self {
            object_to_world: transform,
            world_to_object: inverse,
            bounds,
            material,
            settings,
            normal,
            offset,
        }
    }
}

impl SDFObject for SdfPlane {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let local = self.world_to_object.point(&world_p);
        let local_vec = Vec3::new(local.x, local.y, local.z);
        self.normal.dot(local_vec) + self.offset
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

    fn gradient(&self, _world_p: Point3) -> Option<Vec3> {
        let world_normal = self.object_to_world.normal(&self.normal);
        if world_normal.magnitude2() <= f64::EPSILON {
            None
        } else {
            Some(world_normal.normalize())
        }
    }
}
