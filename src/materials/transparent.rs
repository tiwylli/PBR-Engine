use std::collections::HashMap;

use cgmath::InnerSpace;
use tinyjson::JsonValue;

use crate::{
    texture::{Texture, json_to_texture},
    vec::{Color3, Point3, Vec2, Vec3},
};

use super::{Material, SampledDirection};
// ChadGPT assisted in writing this material for missing knowledge and code architecture

/// Perfect transparent material (no refraction): straight-through transmission.
/// Delta distribution: `evaluate() = 0`, `pdf() = 0`; only `sample()` returns a direction.
pub struct Transparent {
    transmittance: Texture<Color3>, // tint / attenuation (e.g., [1,1,1] for clear)
    normal_map: Option<Texture<Vec3>>,
}

impl Transparent {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let transmittance = json_to_texture(json, "transmittance", Vec3::new(1.0, 1.0, 1.0));
        let normal_map = super::json_to_normal_map(json);

        Self {
            transmittance,
            normal_map,
        }
    }
}

impl Material for Transparent {
    fn sample(&self, wo: &Vec3, uv: &Vec2, p: &Point3, _sample: &Vec2) -> Option<SampledDirection> {
        // Two-sided: if wo is tangent, we cannot define a sensible transmission.
        if wo.magnitude2() == 0.0 {
            return None;
        }
        // Straight-through transmission: continue in the same global direction,
        // which in local shading coordinates is the opposite vector.
        let wi = -*wo;

        // If wi is degenerate (shouldn't happen), abort.
        if wi.magnitude2() == 0.0 {
            return None;
        }

        Some(SampledDirection {
            wi,
            // Delta BSDF: weight is the transmittance tint (no cos/pdf terms).
            weight: self.transmittance.get(uv, p),
        })
    }

    fn emission(&self, _: &Vec3, _uv: &Vec2, _p: &Point3) -> Color3 {
        Color3::new(0.0, 0.0, 0.0)
    }

    fn have_emission(&self) -> bool {
        false
    }

    // Per assignment rule for delta materials in mixtures: return black/0 here.
    fn evaluate(&self, _wo: &Vec3, _wi: &Vec3, _uv: &Vec2, _p: &Point3) -> Color3 {
        Color3::new(0.0, 0.0, 0.0)
    }

    fn pdf(&self, _wo: &Vec3, _wi: &Vec3, _uv: &Vec2, _p: &Point3) -> f64 {
        0.0
    }

    fn have_delta(&self) -> bool {
        true
    }

    fn get_normal_map_value(&self, uv: &Vec2, p: &Point3) -> Vec3 {
        super::get_normal_map_value_helper(self.normal_map.as_ref(), uv, p)
    }

    fn have_normal_map(&self) -> bool {
        self.normal_map.is_some()
    }
}
