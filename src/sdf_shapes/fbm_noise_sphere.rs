use std::{collections::HashMap, sync::Arc};

use cgmath::{InnerSpace, Matrix3};
use tinyjson::JsonValue;

use crate::{
    aabb::AABB,
    json::{json_to_f64, json_to_vec3},
    materials::Material,
    transform::{MyTransform, json_to_transform},
    vec::{Point3, Vec3},
};

use super::{
    fbm_noise::{NoiseVariant, json_to_warp, sd_fbm},
    raymarch::RaymarchSettings,
    sdf_object::{SDFObject, json_to_bounds, parse_object_settings},
};

fn sd_sphere(p: Vec3, radius: f64) -> f64 {
    p.magnitude() - radius
}

pub struct SdfFbmNoiseSphere {
    object_to_world: MyTransform,
    world_to_object: MyTransform,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
    radius: f64,
    offset: Vec3,
    octaves: u32,
    frequency: f64,
    gain: f64,
    blend: f64,
    warp_matrix: Matrix3<f64>,
    noise_variant: NoiseVariant,
}

impl SdfFbmNoiseSphere {
    #[must_use]
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let transform = json_to_transform(json, "transform");
        let inverse = transform.inverse();

        let radius = json
            .get("radius")
            .map(|_| json_to_f64(json, "radius", 1.0).max(0.0))
            .unwrap_or_else(|| {
                let half_extent = json_to_vec3(json, "half_extent", Vec3::new(1.0, 1.0, 1.0));
                let corner_radius = json_to_f64(json, "corner_radius", 0.1).max(0.0);
                half_extent.x.max(half_extent.y.max(half_extent.z)) + corner_radius
            });
        let offset = json_to_vec3(json, "offset", Vec3::new(0.5, 0.5, 0.5));

        let octaves = json_to_f64(json, "octaves", 6.0).clamp(1.0, 10.0) as u32;
        let frequency = json_to_f64(json, "frequency", 2.0).max(0.1);
        let gain = json_to_f64(json, "gain", 0.55).clamp(0.01, 0.99);
        let blend = json_to_f64(json, "blend", 0.15).max(0.0);
        let warp_matrix = json_to_warp(json);

        let noise_variant = json
            .get("noise_type")
            .map_or(NoiseVariant::Lattice, |value| {
                let variant: &str = value
                    .get::<String>()
                    .expect("noise_type must be a string (lattice/simplex)");
                match variant.to_lowercase().as_str() {
                    "simplex" => NoiseVariant::Simplex,
                    _ => NoiseVariant::Lattice,
                }
            });

        let bounds = json_to_bounds(json).unwrap_or_else(|| {
            let margin = 1.0;
            let extent = radius + margin;
            AABB::from_points(
                Point3::new(-extent, -extent, -extent),
                Point3::new(extent, extent, extent),
            )
        });

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("FBM noise sphere `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{name}` for fbm noise sphere")),
            )
        });

        let settings = parse_object_settings(json);

        Self {
            object_to_world: transform,
            world_to_object: inverse,
            bounds,
            material,
            settings,
            radius,
            offset,
            octaves,
            frequency,
            gain,
            blend,
            warp_matrix,
            noise_variant,
        }
    }
}

impl SDFObject for SdfFbmNoiseSphere {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let local = self.world_to_object.point(&world_p);
        let p = Vec3::new(local.x, local.y, local.z);
        let base = sd_sphere(p, self.radius);
        sd_fbm(
            p + self.offset,
            base,
            self.octaves,
            self.frequency,
            self.gain,
            self.blend,
            self.warp_matrix,
            self.noise_variant,
        )
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
        0.5
    }
}
