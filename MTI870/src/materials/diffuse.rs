use std::{collections::HashMap, f64::consts::PI};

use cgmath::{InnerSpace, Vector3, Zero};
use tinyjson::JsonValue;

use crate::{
    json::json_to_vec3,
    vec::{pdf_cosine_hemisphere, sample_cosine_hemisphere, Color3, Vec2, Vec3},
};

use super::{Material, SampledDirection};

pub struct Diffuse {
    albedo: Color3,
}

impl Diffuse {
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Diffuse {
        let albedo = json_to_vec3(json, "albedo", Vec3::new(0.8, 0.8, 0.8));
        Diffuse { albedo }
    }
}

impl Material for Diffuse {
    fn sample(&self, wo: &Vec3, sample: &Vec2) -> Option<SampledDirection> {
        if wo.z < 0.0 {
            return None;
        }

        let wi = sample_cosine_hemisphere(sample);

        //shouldnt happen
        if wi.z <= 0.0 {
            return None;
        }

        Some(SampledDirection {
            weight: self.albedo,
            wi,
        })
    }

    fn emission(&self, _: &Vec3) -> Color3 {
        Color3::zero()
    }

    fn have_emission(&self) -> bool {
        false
    }

    fn evaluate(&self, _wo: &Vec3, wi: &Vec3) -> Color3 {
        // f_r * cos(theta_i) = (albedo/PI) * cos
        let cos_theta = wi.z.max(0.0);
        (self.albedo / PI) * cos_theta
    }

    fn pdf(&self, _wo: &Vec3, wi: &Vec3) -> f64 {
        pdf_cosine_hemisphere(wi)
    }

    fn have_delta(&self) -> bool {
        false
    }
}
