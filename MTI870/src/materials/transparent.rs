use std::collections::HashMap;

use cgmath::InnerSpace;
use tinyjson::JsonValue;

use crate::{
    json::json_to_vec3,
    vec::{Color3, Vec2, Vec3},
};

use super::{Material, SampledDirection};
// ChadGPT assisted in writing this material for missing knowledge and code architecture

/// Perfect transparent material (no refraction): straight-through transmission.
/// Delta distribution: evaluate() = 0, pdf() = 0; only sample() returns a direction.
pub struct Transparent {
    transmittance: Color3, // tint / attenuation (e.g., [1,1,1] for clear)
}

impl Transparent {
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Transparent {
        // Field name "color" or "transmittance" are both accepted; default is white.
        let mut t = json_to_vec3(json, "transmittance", Vec3::new(1.0, 1.0, 1.0));
        // Backward-compat: allow "color" as alias
        if let Some(JsonValue::Array(_)) = json.get("color") {
            t = json_to_vec3(json, "color", t);
        }
        Transparent { transmittance: t }
    }
}

impl Material for Transparent {
    fn sample(&self, wo: &Vec3, _sample: &Vec2) -> Option<SampledDirection> {
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
            weight: self.transmittance,
        })
    }

    fn emission(&self, _: &Vec3) -> Color3 {
        Color3::new(0.0, 0.0, 0.0)
    }

    fn have_emission(&self) -> bool {
        false
    }

    // Per assignment rule for delta materials in mixtures: return black/0 here.
    fn evaluate(&self, _wo: &Vec3, _wi: &Vec3) -> Color3 {
        Color3::new(0.0, 0.0, 0.0)
    }

    fn pdf(&self, _wo: &Vec3, _wi: &Vec3) -> f64 {
        0.0
    }

    fn have_delta(&self) -> bool {
        true
    }
}
