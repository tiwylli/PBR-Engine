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

#[derive(Clone, Copy)]
enum NoiseSphereMode {
    Solid,
    Shell,
}

/// Sphere SDF whose surface band is perturbed by fractal value noise.
pub struct SdfNoiseSphere {
    object_to_world: MyTransform,
    world_to_object: MyTransform,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
    radius: f64,
    band_width: f64,
    noise_amplitude: f64,
    noise_frequency: f64,
    noise_octaves: u32,
    noise_translate: Vec3,
    mode: NoiseSphereMode,
}

impl SdfNoiseSphere {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let transform = json_to_transform(json, "transform");
        let inverse = transform.inverse();
        let bounds = json_to_bounds(json).unwrap_or_else(|| {
            AABB::from_points(Point3::new(-3.0, -3.0, -3.0), Point3::new(3.0, 3.0, 3.0))
        });

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("Noise sphere `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{}` for noise sphere", name)),
            )
        });

        let settings = parse_object_settings(json);
        let radius = json_to_f64(json, "radius", 1.0).max(1.0e-4);
        let band_width = json_to_f64(json, "band_width", 0.2).abs();
        let noise_amplitude = json_to_f64(json, "noise_amplitude", 0.15);
        let noise_frequency = json_to_f64(json, "noise_frequency", 4.0).max(1.0e-3);
        let noise_octaves = json_to_f64(json, "noise_octaves", 4.0).max(1.0) as u32;
        let noise_translate = json_to_vec3(json, "noise_translate", Vec3::new(0.0, 0.0, 0.0));
        let mode = json
            .get("mode")
            .map(|value| {
                let raw: &str = value
                    .get::<String>()
                    .expect("Noise sphere `mode` must be a string");
                if raw.eq_ignore_ascii_case("shell") {
                    NoiseSphereMode::Shell
                } else if raw.eq_ignore_ascii_case("solid") {
                    NoiseSphereMode::Solid
                } else {
                    panic!(
                        "Unknown noise sphere mode `{}` (expected `solid` or `shell`)",
                        raw
                    );
                }
            })
            .unwrap_or(NoiseSphereMode::Solid);

        Self {
            object_to_world: transform,
            world_to_object: inverse,
            bounds,
            material,
            settings,
            radius,
            band_width,
            noise_amplitude,
            noise_frequency,
            noise_octaves,
            noise_translate,
            mode,
        }
    }

    fn noise_value(&self, p: Vec3) -> f64 {
        let noise_p = p * self.noise_frequency + self.noise_translate;
        fbm(noise_p, 1.0, self.noise_octaves) * 2.0 - 1.0
    }

    fn distance_solid(&self, base: f64, p: Vec3) -> f64 {
        if self.band_width <= std::f64::EPSILON || self.noise_amplitude == 0.0 {
            return base;
        }

        let abs_base = base.abs();
        if abs_base >= self.band_width {
            return base;
        }

        let mut fade = 1.0 - (abs_base / self.band_width);
        fade = fade * fade * (3.0 - 2.0 * fade); // smoothstep

        base + self.noise_amplitude * fade * self.noise_value(p)
    }

    fn distance_shell(&self, base: f64, p: Vec3) -> f64 {
        if self.band_width <= std::f64::EPSILON {
            return base.abs();
        }

        let shell = base.abs() - self.band_width;
        if shell >= 0.0 || self.noise_amplitude == 0.0 {
            return shell;
        }

        shell + self.noise_amplitude * self.noise_value(p)
    }
}

fn hash(x: i32, y: i32, z: i32) -> f64 {
    let mut n = (x as i64)
        .wrapping_mul(15731)
        .wrapping_add((y as i64).wrapping_mul(789221))
        .wrapping_add((z as i64).wrapping_mul(1376312589));
    n = (n << 13) ^ n;
    1.0 - (((n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff) as f64 / 1073741824.0)
}

fn fade(t: f64) -> f64 {
    // smootherstep
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn value_noise(p: &Vec3) -> f64 {
    let xi = p.x.floor() as i32;
    let yi = p.y.floor() as i32;
    let zi = p.z.floor() as i32;

    let xf = p.x - xi as f64;
    let yf = p.y - yi as f64;
    let zf = p.z - zi as f64;

    let u = fade(xf);
    let v = fade(yf);
    let w = fade(zf);

    let mut accum = 0.0;
    for dx in 0..=1 {
        for dy in 0..=1 {
            for dz in 0..=1 {
                let corner = hash(xi + dx, yi + dy, zi + dz);
                let tx = if dx == 1 { u } else { 1.0 - u };
                let ty = if dy == 1 { v } else { 1.0 - v };
                let tz = if dz == 1 { w } else { 1.0 - w };
                accum += corner * tx * ty * tz;
            }
        }
    }
    accum
}

fn fbm(mut p: Vec3, freq: f64, octaves: u32) -> f64 {
    let mut amplitude = 0.5;
    let mut sum = 0.0;
    let mut total = 0.0;
    p = p * freq;

    for _ in 0..octaves {
        sum += value_noise(&p) * amplitude;
        total += amplitude;
        p = p * 2.0;
        amplitude *= 0.5;
    }

    if total > 0.0 {
        sum / total
    } else {
        0.0
    }
}

impl SDFObject for SdfNoiseSphere {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let local = self.world_to_object.point(&world_p);
        let p = Vec3::new(local.x, local.y, local.z);
        let base = p.magnitude() - self.radius;

        match self.mode {
            NoiseSphereMode::Solid => self.distance_solid(base, p),
            NoiseSphereMode::Shell => self.distance_shell(base, p),
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
}
