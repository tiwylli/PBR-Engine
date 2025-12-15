use std::collections::HashMap;

use cgmath::Zero;
use tinyjson::JsonValue;

use crate::{
    Real,
    json::{json_to_f64, json_to_vec3},
    materials::SampledDirection,
    texture::{Texture, json_to_texture},
    vec::{Color3, Point3, Vec2, Vec3},
};

use super::Material;

pub struct DiffuseEmit {
    radiance: Texture<Color3>,
}

impl DiffuseEmit {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let radiance = json_to_texture(json, "radiance", Vec3::new(1.0, 1.0, 1.0));
        Self { radiance }
    }
}

impl Material for DiffuseEmit {
    fn sample(&self, _wo: &Vec3, _uv: &Vec2, _p: &Point3, _s: &Vec2) -> Option<SampledDirection> {
        None
    }

    fn evaluate(&self, _wo: &Vec3, _wi: &Vec3, _uv: &Vec2, _p: &Point3) -> Color3 {
        panic!("Should not call evaluate on DiffuseEmit material");
    }

    fn pdf(&self, _wo: &Vec3, _wi: &Vec3, _uv: &Vec2, _p: &Point3) -> f64 {
        panic!("Should not call pdf on DiffuseEmit material");
    }

    fn have_delta(&self) -> bool {
        false
    }

    fn emission(&self, wo: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        if wo.z > 0.0 {
            self.radiance.get(uv, p)
        } else {
            Color3::new(0.0, 0.0, 0.0)
        }
    }

    fn have_emission(&self) -> bool {
        true
    }
}

pub struct DiffuseEmitSpotLight {
    radiance: Color3,
    focus: Real,
}

impl DiffuseEmitSpotLight {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let radiance = json_to_vec3(json, "radiance", Vec3::new(1.0, 1.0, 1.0));
        let focus = json_to_f64(json, "focus", 1.0);

        Self {
            radiance,
            focus: focus * focus,
        }
    }
}

impl Material for DiffuseEmitSpotLight {
    fn sample(&self, _wo: &Vec3, _uv: &Vec2, _p: &Point3, _s: &Vec2) -> Option<SampledDirection> {
        None
    }

    fn evaluate(&self, _wo: &Vec3, _wi: &Vec3, _uv: &Vec2, _p: &Point3) -> Color3 {
        panic!("Should not call evaluate on DiffuseEmitSpotLight material");
    }

    fn pdf(&self, _wo: &Vec3, _wi: &Vec3, _uv: &Vec2, _p: &Point3) -> f64 {
        panic!("Should not call pdf on DiffuseEmitSpotLight material");
    }

    fn have_delta(&self) -> bool {
        false
    }

    fn emission(&self, wo: &Vec3, _uv: &Vec2, _p: &Point3) -> Color3 {
        if wo.z < 0.0 {
            Color3::zero()
        } else {
            self.radiance * wo.z.powf(self.focus)
        }
    }

    fn have_emission(&self) -> bool {
        true
    }
}
