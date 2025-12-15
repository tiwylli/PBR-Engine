#![allow(warnings)]

use std::{collections::HashMap, f64};

use cgmath::{ElementWise, Zero};
use tinyjson::JsonValue;

use crate::{
    json::{json_to_bool, json_to_f64, json_to_vec3},
    materials::SampledDirection,
    samplers::sample_anisotropic_hemisphere,
    vec::{Color3, Point3, Vec2, Vec3},
};

use super::Material;

pub struct AnisotropicMetal {
    ks: Color3,
    nu: f64,
    nv: f64,
    use_fresnel: bool,
}

impl AnisotropicMetal {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let ks = json_to_vec3(json, "ks", Vec3::new(1.0, 1.0, 1.0));
        let nu = json_to_f64(json, "nu", 1.0);
        let nv = json_to_f64(json, "nv", 1.0);
        let use_fresnel = json_to_bool(json, "use_fresnel", false);
        Self {
            ks,
            nu,
            nv,
            use_fresnel,
        }
    }
}

#[allow(clippy::option_if_let_else)]
impl Material for AnisotropicMetal {
    fn sample(&self, wo: &Vec3, uv: &Vec2, p: &Point3, s: &Vec2) -> Option<SampledDirection> {
        if wo.z < 0.0 {
            return None;
        }

        let weight = if self.use_fresnel {
            self.ks - self.ks.sub_element_wise(1.0) * (1.0 - wo.z).powi(5)
        } else {
            self.ks
        };

        let phi_offset = wo.y.atan2(wo.x);
        let wi = sample_anisotropic_hemisphere(s, self.nu, self.nv, phi_offset);
        if wi.z > 0.0 {
            Some(SampledDirection { weight, wi })
        } else {
            None
        }
    }

    fn evaluate(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        if self.have_delta() || wi.z < 0.0 {
            Color3::zero()
        } else {
            self.ks * self.pdf(wo, wi, uv, p)
        }
    }

    fn pdf(&self, _wo: &Vec3, _wi: &Vec3, uv: &Vec2, p: &Point3) -> f64 {
        // if self.have_delta() || wi.z < 0.0 {
        //     0.0
        // } else {
        //     let frame = Frame::new(&Vec3::new(-wo.x, -wo.y, wo.z));
        //     pdf_cosine_hemisphere_power(&frame.to_local(wi), self.exponent.unwrap())
        // }
        todo!()
    }

    fn have_delta(&self) -> bool {
        false
    }

    fn emission(&self, _: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        Color3::zero()
    }

    fn have_emission(&self) -> bool {
        false
    }
}
