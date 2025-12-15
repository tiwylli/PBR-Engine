use std::collections::HashMap;
use tinyjson::JsonValue;

use crate::{
    json::json_to_vec3,
    vec::{Color3, Vec2, Vec3},
};

use super::Material;

pub struct DiffuseEmit {
    radiance: Color3,
}

impl DiffuseEmit {
    pub fn from_json(json: &HashMap<String, JsonValue>) -> DiffuseEmit {
        let radiance = json_to_vec3(json, "radiance", Vec3::new(1.0, 1.0, 1.0));
        DiffuseEmit { radiance }
    }
}

impl Material for DiffuseEmit {
    fn sample(&self, _: &Vec3, _: &Vec2) -> Option<super::SampledDirection> {
        None
    }

    fn emission(&self, wo: &Vec3) -> Color3 {
        if wo.z > 0.0 {
            self.radiance
        } else {
            Color3::new(0.0, 0.0, 0.0)
        }
    }

    fn have_emission(&self) -> bool {
        true
    }

    fn evaluate(&self, wo: &Vec3, wi: &Vec3) -> Color3 {
        Color3::new(0.0, 0.0, 0.0)
    }

    fn pdf(&self, wo: &Vec3, wi: &Vec3) -> f64 {
        0.0
    }

    fn have_delta(&self) -> bool {
        false
    }
}
