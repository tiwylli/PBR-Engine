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
    raymarch::RaymarchSettings,
    sdf_object::{SDFObject, json_to_bounds, parse_object_settings},
};

fn sd_rounded_box(p: Vec3, half_extent: Vec3, radius: f64) -> f64 {
    let q = Vec3::new(
        p.x.abs() - half_extent.x + radius,
        p.y.abs() - half_extent.y + radius,
        p.z.abs() - half_extent.z + radius,
    );
    let outside = Vec3::new(q.x.max(0.0), q.y.max(0.0), q.z.max(0.0));
    let inside = q.x.max(q.y.max(q.z)).min(0.0);
    outside.magnitude() + inside - radius
}

fn smooth_max(a: f64, b: f64, k: f64) -> f64 {
    if k <= 0.0 {
        return a.max(b);
    }
    let h = ((k - (a - b).abs()).max(0.0)) / k;
    (h * h * k).mul_add(0.25, a.max(b))
}

#[derive(Clone, Copy)]
pub(crate) enum NoiseVariant {
    Lattice,
    Simplex,
}

fn hash_vec3(i: Vec3) -> f64 {
    let mut n = i.x as i64 * 15731 + i.y as i64 * 789_221 + i.z as i64 * 13_763_125_899;
    n = (n << 13) ^ n;
    let nn = n * (n * n * 15731 + 789_221) + 13_763_125_899;
    1.0 - ((nn & 0x7fff_ffff) as f64 / 1_073_741_824.0)
}

fn sd_base_lattice(p: Vec3) -> f64 {
    let cell = Vec3::new(p.x.floor(), p.y.floor(), p.z.floor());
    let frac = Vec3::new(p.x - cell.x, p.y - cell.y, p.z - cell.z);
    let mut result = f64::INFINITY;
    for dx in 0..=1 {
        for dy in 0..=1 {
            for dz in 0..=1 {
                let corner = Vec3::new(f64::from(dx), f64::from(dy), f64::from(dz));
                let hash = hash_vec3(cell + corner);
                let radius = (hash.abs()) * (hash.abs()) * 0.7;
                let d = (frac - corner).magnitude() - radius;
                result = result.min(d);
            }
        }
    }
    result
}

fn sd_base_simplex(p: Vec3) -> f64 {
    const K1: f64 = 1.0 / 3.0;
    const K2: f64 = 1.0 / 6.0;
    let sum = p.x + p.y + p.z;
    let i = Vec3::new(
        sum.mul_add(K1, p.x).floor(),
        sum.mul_add(K1, p.y).floor(),
        sum.mul_add(K1, p.z).floor(),
    );
    let d0 = p
        - (i - Vec3::new(
            (i.x + i.y + i.z) * K2,
            (i.x + i.y + i.z) * K2,
            (i.x + i.y + i.z) * K2,
        ));

    let e = Vec3::new(
        if d0.y < d0.x { 1.0 } else { 0.0 },
        if d0.z < d0.y { 1.0 } else { 0.0 },
        if d0.x < d0.z { 1.0 } else { 0.0 },
    );
    let i1 = Vec3::new(e.x * (1.0 - e.z), e.y * (1.0 - e.x), e.z * (1.0 - e.y));
    let i2 = Vec3::new(
        e.x.mul_add(-(1.0 - e.z), 1.0),
        e.y.mul_add(-(1.0 - e.x), 1.0),
        e.z.mul_add(-(1.0 - e.y), 1.0),
    );

    let d1 = d0 - (i1 - Vec3::new(K2, K2, K2));
    let d2 = d0 - (i2 - Vec3::new(2.0 * K2, 2.0 * K2, 2.0 * K2));
    let d3 = d0 - (Vec3::new(1.0, 1.0, 1.0) - Vec3::new(3.0 * K2, 3.0 * K2, 3.0 * K2));

    let r0 = hash_vec3(i);
    let r1 = hash_vec3(i + i1);
    let r2 = hash_vec3(i + i2);
    let r3 = hash_vec3(i + Vec3::new(1.0, 1.0, 1.0));

    let sph = |d: Vec3, r: f64| (r * r).mul_add(-0.55, d.magnitude());

    sph(d0, r0)
        .min(sph(d1, r1))
        .min(sph(d2, r2).min(sph(d3, r3)))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sd_fbm(
    mut p: Vec3,
    mut d: f64,
    octaves: u32,
    frequency: f64,
    gain: f64,
    blend: f64,
    warp: Matrix3<f64>,
    noise_variant: NoiseVariant,
) -> f64 {
    let mut amp = 1.0;
    let mut freq = 1.0;
    for _ in 0..octaves {
        let base = match noise_variant {
            NoiseVariant::Lattice => sd_base_lattice(p * freq),
            NoiseVariant::Simplex => sd_base_simplex(p * freq),
        };
        let n = amp * base;
        d = smooth_max(d, -n, blend * amp);
        p = warp * p;
        amp *= gain;
        freq *= frequency;
    }
    d
}

pub(crate) fn json_to_warp(json: &HashMap<String, JsonValue>) -> Matrix3<f64> {
    json.get("warp_matrix").map_or_else(
        || Matrix3::new(0.0, 0.80, 0.60, -0.80, 0.36, -0.48, -0.60, -0.48, 0.64),
        |matrix| {
            let rows: &Vec<JsonValue> = matrix
                .get()
                .expect("warp_matrix must be an array of 3 rows");
            assert!(rows.len() == 3, "warp_matrix must contain exactly 3 rows");
            let mut data = [[0.0f64; 3]; 3];
            for (r, row_value) in rows.iter().enumerate() {
                let cols: &Vec<JsonValue> = row_value
                    .get()
                    .expect("warp_matrix rows must be arrays of 3 numbers");
                assert!(cols.len() == 3, "warp_matrix rows must have 3 elements");
                for (c, value) in cols.iter().enumerate() {
                    data[r][c] = *value
                        .get::<f64>()
                        .expect("warp_matrix entries must be numbers");
                }
            }
            Matrix3::new(
                data[0][0], data[0][1], data[0][2], data[1][0], data[1][1], data[1][2], data[2][0],
                data[2][1], data[2][2],
            )
        },
    )
}

pub struct SdfFbmNoise {
    object_to_world: MyTransform,
    world_to_object: MyTransform,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
    half_extent: Vec3,
    corner_radius: f64,
    offset: Vec3,
    octaves: u32,
    frequency: f64,
    gain: f64,
    blend: f64,
    warp_matrix: Matrix3<f64>,
    noise_variant: NoiseVariant,
}

impl SdfFbmNoise {
    #[must_use]
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let transform = json_to_transform(json, "transform");
        let inverse = transform.inverse();

        let half_extent = json_to_vec3(json, "half_extent", Vec3::new(1.0, 1.0, 1.0));
        let corner_radius = json_to_f64(json, "radius", 0.1).max(0.0);
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
            AABB::from_points(
                Point3::new(
                    -half_extent.x - margin,
                    -half_extent.y - margin,
                    -half_extent.z - margin,
                ),
                Point3::new(
                    half_extent.x + margin,
                    half_extent.y + margin,
                    half_extent.z + margin,
                ),
            )
        });

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("FBM noise `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{name}` for fbm noise")),
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
            corner_radius,
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

impl SDFObject for SdfFbmNoise {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let local = self.world_to_object.point(&world_p);
        let p = Vec3::new(local.x, local.y, local.z);
        let base = sd_rounded_box(p, self.half_extent, self.corner_radius);
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
