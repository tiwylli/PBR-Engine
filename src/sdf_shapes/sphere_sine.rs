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

/// Sine-displaced sphere with axis-dependent frequencies.
pub struct SdfSphereSine {
    object_to_world: MyTransform,
    world_to_object: MyTransform,
    radius: f64,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
    displacement_freq: Vec3,
    displacement_amp: f64,
    displacement_axis_amp: Vec3,
}

impl SdfSphereSine {
    #[must_use]
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let transform = json_to_transform(json, "transform");
        let inverse = transform.inverse();
        let radius = json_to_f64(json, "radius", 1.0);
        let bounds = json_to_bounds(json).expect("SDF sphere requires a `bounds` entry");
        let displacement_freq = Vec3::new(
            json_to_f64(json, "disp_freq_x", 1.0),
            json_to_f64(json, "disp_freq_y", 1.0),
            json_to_f64(json, "disp_freq_z", 1.0),
        );
        let displacement_amp = json_to_f64(json, "disp_amplitude", 1.0);
        let displacement_axis_amp = Vec3::new(
            json_to_f64(json, "disp_amplitude_x", 1.0),
            json_to_f64(json, "disp_amplitude_y", 1.0),
            json_to_f64(json, "disp_amplitude_z", 1.0),
        );

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("SDF sphere `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{name}` for SDF sphere")),
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
            displacement_freq,
            displacement_amp,
            displacement_axis_amp,
        }
    }
}

impl SDFObject for SdfSphereSine {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let local = self.world_to_object.point(&world_p);
        let v = Vec3::new(local.x, local.y, local.z);
        let disp_x = (self.displacement_freq.x * v.x).sin() * self.displacement_axis_amp.x;
        let disp_y = (self.displacement_freq.y * v.y).sin() * self.displacement_axis_amp.y;
        let disp_z = (self.displacement_freq.z * v.z).sin() * self.displacement_axis_amp.z;
        let displacement = disp_x * disp_y * disp_z;
        displacement.mul_add(self.displacement_amp, v.magnitude() - self.radius)
    }

    // TODO: TRY THIS WITHOUT CHANGING CLAMP STEP IN JSON
    // fn signed_distance(&self, world_p: Point3) -> f64 {
    //     let local = self.world_to_object.point(&world_p);
    //     let v = Vec3::new(local.x, local.y, local.z);

    //     let base = v.magnitude() - self.radius;

    //     let disp_x = (self.displacement_freq.x * v.x).sin() * self.displacement_axis_amp.x;
    //     let disp_y = (self.displacement_freq.y * v.y).sin() * self.displacement_axis_amp.y;
    //     let disp_z = (self.displacement_freq.z * v.z).sin() * self.displacement_axis_amp.z;
    //     let displacement = disp_x * disp_y * disp_z;

    //     let raw = base + self.displacement_amp * displacement;

    //     // crude Lipschitz bound
    //     let lipschitz = 1.0
    //         + self.displacement_amp
    //             * (self.displacement_freq.x.abs()
    //                 + self.displacement_freq.y.abs()
    //                 + self.displacement_freq.z.abs());

    //     // Shrink the returned distance so steps are safe
    //     raw / lipschitz.max(1.0)
    // }

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
        0.6
    }
}
