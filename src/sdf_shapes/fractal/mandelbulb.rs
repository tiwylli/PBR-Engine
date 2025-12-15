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

use super::super::{
    raymarch::RaymarchSettings,
    sdf_object::{SDFObject, json_to_bounds, parse_object_settings},
};

/// Placeholder implementation for a Mandelbulb fractal.
pub struct SdfMandelbulb {
    object_to_world: MyTransform,
    world_to_object: MyTransform,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
    power: f64,
    max_iterations: u32,
    bailout: f64,
    solid_radius: f64,
}

impl SdfMandelbulb {
    #[must_use]
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let transform = json_to_transform(json, "transform");
        let inverse = transform.inverse();
        let bounds = json_to_bounds(json).unwrap_or_else(|| {
            // Conservative bounding sphere in case the user omits bounds.
            AABB::from_points(Point3::new(-4.0, -4.0, -4.0), Point3::new(4.0, 4.0, 4.0))
        });

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("Mandelbulb `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{name}` for Mandelbulb")),
            )
        });

        let settings = parse_object_settings(json);
        let power = json_to_f64(json, "power", 8.0);
        let max_iterations = json_to_f64(json, "max_iterations", 12.0) as u32;
        let bailout = json_to_f64(json, "bailout", 8.0);
        let solid_radius = json_to_f64(json, "solid_radius", 0.0).max(0.0);

        Self {
            object_to_world: transform,
            world_to_object: inverse,
            bounds,
            material,
            settings,
            power,
            max_iterations,
            bailout,
            solid_radius,
        }
    }
}

impl SDFObject for SdfMandelbulb {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let local = self.world_to_object.point(&world_p);
        let c = Vec3::new(local.x, local.y, local.z);
        let mut z = c;
        let mut dr = 1.0;
        let mut r = z.magnitude();
        let power = self.power.max(2.0);

        let mut iter = 0;
        while iter < self.max_iterations {
            if r > self.bailout {
                break;
            }
            if r <= f64::EPSILON {
                break;
            }

            // Convert to spherical coordinates
            let mut theta = (z.z / r).clamp(-1.0, 1.0).acos();
            let mut phi = z.y.atan2(z.x);

            let zr = r.powf(power);
            dr = (power * r.powf(power - 1.0)).mul_add(dr, 1.0);

            theta *= power;
            phi *= power;

            let sin_theta = theta.sin();
            z = Vec3::new(
                zr * sin_theta * phi.cos(),
                zr * sin_theta * phi.sin(),
                zr * theta.cos(),
            ) + c;

            r = z.magnitude();
            iter += 1;
        }

        let de = if r <= f64::EPSILON {
            0.0
        } else {
            0.5 * r.ln() * r / dr
        };

        if self.solid_radius > 0.0 && r < self.solid_radius {
            -(self.solid_radius - r)
        } else {
            de
        }
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

    fn max_raymarch_distance(&self) -> Option<f64> {
        None
    }

    fn step_scale(&self) -> f64 {
        0.4
    }
}
