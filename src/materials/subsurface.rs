use std::{collections::HashMap, f64::consts::PI};

use cgmath::prelude::ElementWise;
use cgmath::{InnerSpace, Zero};
use tinyjson::JsonValue;

use crate::texture::{Texture, json_to_texture};
use crate::vec::Point3;
use crate::{
    json::json_to_vec3,
    samplers::{pdf_cosine_hemisphere, sample_cosine_hemisphere},
    vec::{Color3, Vec2, Vec3},
};

use super::{Material, SampledDirection};
// ChadGPT assisted in writing this material for missing knowledge and code architecture
/// Fake Subsurface (thin SSS): diffuse transmission only (no refraction),
/// with Beer–Lambert attenuation through an approximate thickness.
pub struct Subsurface {
    /// Base transmitted color (tint) before absorption; often called "albedo" or "kt".
    kt: Texture<Color3>,
    /// Absorption coefficient per unit length (Beer: exp(-sigma_a * d)), RGB.
    sigma_a: Color3,
    /// Approximate physical thickness (scene units) used for path-length estimation.
    thickness: f64,
    normal_map: Option<Texture<Vec3>>,
}

impl Subsurface {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let kt = json_to_texture(json, "kt", Vec3::new(1.0, 1.0, 1.0));

        let sigma_a = json_to_vec3(json, "sigma_a", Vec3::new(0.0, 0.0, 0.0)); // 0 => no absorption
        let thickness = if let Some(JsonValue::Number(n)) = json.get("thickness") {
            (*n).max(0.0)
        } else {
            1.0
        };
        let normal_map = super::json_to_normal_map(json);

        Self {
            kt,
            sigma_a,
            thickness,
            normal_map,
        }
    }

    /// Beer–Lambert attenuation for transmission.
    /// We approximate the optical path by thickness / `cos(theta_i)`
    #[inline]
    fn attenuation(&self, wi: &Vec3) -> Color3 {
        let cos_i = wi.z.abs().max(1e-4);
        let path = self.thickness / cos_i;
        Vec3::new(
            (-self.sigma_a.x * path).exp(),
            (-self.sigma_a.y * path).exp(),
            (-self.sigma_a.z * path).exp(),
        )
    }
}

impl Material for Subsurface {
    fn sample(&self, wo: &Vec3, uv: &Vec2, p: &Point3, sample: &Vec2) -> Option<SampledDirection> {
        if wo.magnitude2() == 0.0 {
            return None;
        }

        // Transmission goes to the opposite hemisphere of wo.
        let flip_sign = if wo.z >= 0.0 { -1.0 } else { 1.0 };

        // Cosine sampling in +Z, then flip to transmission side.
        let mut v = sample_cosine_hemisphere(sample); // z >= 0
        v.z *= flip_sign;
        let wi = v;

        // Mixture reweighting compatible: weight = (f*cos)/pdf
        let pdf = self.pdf(wo, &wi, uv, p);
        if pdf <= 0.0 {
            return None;
        }
        let fcos = self.evaluate(wo, &wi, uv, p);
        Some(SampledDirection {
            wi,
            weight: fcos / pdf,
        })
    }

    fn emission(&self, _: &Vec3, _uv: &Vec2, _p: &Point3) -> Color3 {
        Color3::zero()
    }
    fn have_emission(&self) -> bool {
        false
    }

    fn evaluate(&self, _wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        // Only supports transmission (opposite side). If wi is on same side, 0.
        if wi.z == 0.0 {
            return Color3::zero();
        }
        // f*cos = (kt * attenuation / PI) * |cos(theta_i)|
        let att = self.attenuation(wi);
        let cos_i = wi.z.abs().max(0.0);
        (self.kt.get(uv, p).mul_element_wise(att)) * (cos_i / PI)
    }

    fn pdf(&self, _wo: &Vec3, wi: &Vec3, _uv: &Vec2, _p: &Point3) -> f64 {
        let mut v = *wi;
        v.z = v.z.abs();
        pdf_cosine_hemisphere(&v)
    }

    fn have_delta(&self) -> bool {
        // Diffuse transmission -> non-delta (works in blends without delta logic)
        false
    }

    fn get_normal_map_value(&self, uv: &Vec2, p: &Point3) -> Vec3 {
        super::get_normal_map_value_helper(self.normal_map.as_ref(), uv, p)
    }

    fn have_normal_map(&self) -> bool {
        self.normal_map.is_some()
    }
}
