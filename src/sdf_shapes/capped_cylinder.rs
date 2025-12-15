use std::{collections::HashMap, sync::Arc};

use cgmath::InnerSpace;
use tinyjson::JsonValue;

use crate::{
    aabb::AABB,
    json::json_to_f64,
    materials::Material,
    transform::{MyTransform, json_to_transform},
    vec::{Point3, Vec2, Vec3},
};

use super::{
    raymarch::RaymarchSettings,
    sdf_object::{SDFObject, json_to_bounds, parse_object_settings},
};

/// Y-aligned capped cylinder (closed top and bottom) SDF.
pub struct SdfCappedCylinder {
    object_to_world: MyTransform,
    world_to_object: MyTransform,
    radius: f64,
    half_height: f64,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
}

impl SdfCappedCylinder {
    #[must_use]
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let transform = json_to_transform(json, "transform");
        let inverse = transform.inverse();
        let radius = json_to_f64(json, "radius", 1.0);
        let half_height = json_to_f64(json, "half_height", 1.0);
        let bounds = json_to_bounds(json).expect("SDF capped cylinder requires a `bounds` entry");

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("SDF capped cylinder `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{name}` for SDF capped cylinder")),
            )
        });

        let settings = parse_object_settings(json);

        Self {
            object_to_world: transform,
            world_to_object: inverse,
            radius,
            half_height,
            bounds,
            material,
            settings,
        }
    }
}

impl SDFObject for SdfCappedCylinder {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let local = self.world_to_object.point(&world_p);
        let v = Vec3::new(local.x, local.y, local.z);
        let radial = v.x.hypot(v.z);
        let d = Vec2::new(radial - self.radius, v.y.abs() - self.half_height);
        let outside = Vec2::new(d.x.max(0.0), d.y.max(0.0)).magnitude();
        let inside = d.x.max(d.y).min(0.0);
        outside + inside
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

    fn gradient(&self, world_p: Point3) -> Option<Vec3> {
        let local = self.world_to_object.point(&world_p);
        let v = Vec3::new(local.x, local.y, local.z);
        let radial = v.x.hypot(v.z);
        let inside_radial = radial <= self.radius + f64::EPSILON;
        let inside_height = v.y.abs() <= self.half_height + f64::EPSILON;

        let mut closest = Vec3::new(v.x, v.y, v.z);
        if inside_radial && inside_height {
            if radial > f64::EPSILON {
                let scale = self.radius / radial;
                closest.x = v.x * scale;
                closest.z = v.z * scale;
            } else {
                closest.x = self.radius;
                closest.z = 0.0;
            }
        } else if inside_radial {
            closest.y = self.half_height * v.y.signum();
        } else if inside_height {
            if radial > f64::EPSILON {
                let scale = self.radius / radial;
                closest.x = v.x * scale;
                closest.z = v.z * scale;
            }
        } else {
            closest.y = self.half_height * v.y.signum();
            if radial > f64::EPSILON {
                let scale = self.radius / radial;
                closest.x = v.x * scale;
                closest.z = v.z * scale;
            }
        }

        let local_normal = v - closest;
        if local_normal.magnitude2() <= f64::EPSILON {
            return None;
        }
        let world_normal = self.object_to_world.normal(&local_normal.normalize());
        if world_normal.magnitude2() <= f64::EPSILON {
            None
        } else {
            Some(world_normal.normalize())
        }
    }
}
