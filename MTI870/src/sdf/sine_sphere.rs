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

/// Sphere SDF whose surface is radially modulated with a sine wave.
pub struct SdfSineSphere {
    object_to_world: MyTransform,
    world_to_object: MyTransform,
    radius: f64,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
    sine_amplitude: f64,
    sine_frequency: Vec3,
    sine_phase: f64,
}

impl SdfSineSphere {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let transform = json_to_transform(json, "transform");
        let inverse = transform.inverse();

        let radius = json_to_f64(json, "radius", 1.0);
        let sine_amplitude = json_to_f64(json, "sine_amplitude", 0.1).max(0.0);
        let sine_phase = json_to_f64(json, "sine_phase", 0.0);
        let sine_frequency = json_to_vec3(json, "sine_frequency", Vec3::new(6.0, 6.0, 6.0));

        let bounds =
            json_to_bounds(json).expect("SDF sine sphere requires a `bounds` entry for culling");

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("SDF sine sphere `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{}` for SDF sine sphere", name)),
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
            sine_amplitude,
            sine_frequency,
            sine_phase,
        }
    }
}

impl SDFObject for SdfSineSphere {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let local = self.world_to_object.point(&world_p);
        let p = Vec3::new(local.x, local.y, local.z);
        let base = p.magnitude() - self.radius;

        if self.sine_amplitude == 0.0 {
            return base;
        }

        let wave =
            p.x * self.sine_frequency.x + p.y * self.sine_frequency.y + p.z * self.sine_frequency.z
                + self.sine_phase;

        base + self.sine_amplitude * wave.sin()
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
