use std::{collections::HashMap, sync::Arc};

use cgmath::InnerSpace;
use rand::{rngs::StdRng, Rng, SeedableRng};
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

struct SphereSample {
    center: Vec3,
    radius: f64,
}

/// Collection of randomly positioned spheres used for Voronoi-style carving.
pub struct SdfVoronoiNoise {
    object_to_world: MyTransform,
    world_to_object: MyTransform,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
    spheres: Vec<SphereSample>,
}

impl SdfVoronoiNoise {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let transform = json_to_transform(json, "transform");
        let inverse = transform.inverse();

        let amount = json_to_f64(json, "amount_points", 32.0).max(0.0) as u32;
        let radius_min = json_to_f64(json, "radius_min", 0.02).max(0.0);
        let radius_max = json_to_f64(json, "radius_max", 0.08).max(radius_min);
        let seed = json_to_f64(json, "seed", 1337.0) as u64;

        let raw_min = json_to_vec3(json, "domain_min", Vec3::new(-0.5, -0.5, -0.5));
        let raw_max = json_to_vec3(json, "domain_max", Vec3::new(0.5, 0.5, 0.5));
        let domain_min = Vec3::new(
            raw_min.x.min(raw_max.x),
            raw_min.y.min(raw_max.y),
            raw_min.z.min(raw_max.z),
        );
        let domain_max = Vec3::new(
            raw_min.x.max(raw_max.x),
            raw_min.y.max(raw_max.y),
            raw_min.z.max(raw_max.z),
        );

        let bounds = json_to_bounds(json).unwrap_or_else(|| {
            AABB::from_points(
                Point3::new(
                    domain_min.x - radius_max,
                    domain_min.y - radius_max,
                    domain_min.z - radius_max,
                ),
                Point3::new(
                    domain_max.x + radius_max,
                    domain_max.y + radius_max,
                    domain_max.z + radius_max,
                ),
            )
        });

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("Voronoi noise `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{}` for Voronoi noise", name)),
            )
        });

        let settings = parse_object_settings(json);

        let mut rng = StdRng::seed_from_u64(seed);
        let mut spheres = Vec::with_capacity(amount as usize);

        for _ in 0..amount {
            let cx = rng.gen_range(domain_min.x..domain_max.x);
            let cy = rng.gen_range(domain_min.y..domain_max.y);
            let cz = rng.gen_range(domain_min.z..domain_max.z);
            let radius = if radius_max - radius_min < std::f64::EPSILON {
                radius_min
            } else {
                rng.gen_range(radius_min..=radius_max)
            };

            spheres.push(SphereSample {
                center: Vec3::new(cx, cy, cz),
                radius,
            });
        }

        Self {
            object_to_world: transform,
            world_to_object: inverse,
            bounds,
            material,
            settings,
            spheres,
        }
    }
}

impl SDFObject for SdfVoronoiNoise {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let local = self.world_to_object.point(&world_p);
        let p = Vec3::new(local.x, local.y, local.z);

        let mut dist = f64::INFINITY;
        for sphere in &self.spheres {
            let dv = p - sphere.center;
            dist = dist.min(dv.magnitude() - sphere.radius);
        }

        dist
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
