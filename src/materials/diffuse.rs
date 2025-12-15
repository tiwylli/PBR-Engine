use std::{collections::HashMap, f64};

use cgmath::{InnerSpace, Zero};
use tinyjson::JsonValue;

use crate::{
    samplers::{pdf_cosine_hemisphere, sample_cosine_hemisphere},
    texture::{Texture, json_to_texture},
    vec::{Color3, Point3, Vec2, Vec3},
};

use super::{Material, SampledDirection};

pub struct Diffuse {
    albedo: Texture<Color3>,
    normal_map: Option<Texture<Vec3>>,
}

impl Diffuse {
    #[must_use]
    pub const fn new(albedo: Texture<Color3>) -> Self {
        Self {
            albedo,
            normal_map: None,
        }
    }

    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let albedo = json_to_texture(json, "albedo", Vec3::new(0.8, 0.8, 0.8));
        let normal_map = super::json_to_normal_map(json);
        Self { albedo, normal_map }
    }
}

impl Material for Diffuse {
    fn sample(&self, wo: &Vec3, uv: &Vec2, p: &Point3, s: &Vec2) -> Option<SampledDirection> {
        if wo.z < 0.0 {
            return None;
        }

        let wi = sample_cosine_hemisphere(s);

        Some(SampledDirection {
            weight: self.albedo.get(uv, p),
            wi: wi.normalize(),
        })
    }

    fn evaluate(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        self.albedo.get(uv, p) * self.pdf(wo, wi, uv, p)
    }

    fn pdf(&self, _wo: &Vec3, wi: &Vec3, _uv: &Vec2, _p: &Point3) -> f64 {
        pdf_cosine_hemisphere(wi)
    }

    fn have_delta(&self) -> bool {
        false
    }

    fn emission(&self, _: &Vec3, _uv: &Vec2, _p: &Point3) -> Color3 {
        Color3::zero()
    }

    fn have_emission(&self) -> bool {
        false
    }

    fn get_normal_map_value(&self, uv: &Vec2, p: &Point3) -> Vec3 {
        super::get_normal_map_value_helper(self.normal_map.as_ref(), uv, p)
    }

    fn have_normal_map(&self) -> bool {
        self.normal_map.is_some()
    }

    fn get_albedo(&self, uv: &Vec2, p: &Point3) -> Color3 {
        self.albedo.get(uv, p)
    }
}
