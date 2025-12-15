use std::{collections::HashMap, f64::consts::PI};

use cgmath::{InnerSpace, Zero};
use tinyjson::JsonValue;

use super::{Material, SampledDirection};
use crate::{
    json::json_to_vec3,
    vec::{luminance, pdf_cosine_hemisphere, sample_cosine_hemisphere, Color3, Vec2, Vec3},
};
// ChadGPT assisted in writing this material for missing knowledge and code architecture

/// Translucent (diffuse transmission) with optional diffuse reflection mix.
/// - Reflection: Lambert on the same side as wo
/// - Transmission: Lambert on the opposite side (no refraction), i.e. straight through but diffuse
pub struct Translucent {
    kd: Color3, // diffuse reflection albedo
    kt: Color3, // diffuse transmission albedo (tint)
    // probability to pick transmission vs reflection, from luminance
    prob_t: f64,
}

impl Translucent {
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Translucent {
        // aliases allowed: "albedo" -> kd (for convenience)
        let kd_default = Vec3::new(0.0, 0.0, 0.0);
        let mut kd = json_to_vec3(json, "kd", kd_default);
        if let Some(JsonValue::Array(_)) = json.get("albedo") {
            kd = json_to_vec3(json, "albedo", kd);
        }

        let kt = json_to_vec3(json, "kt", Vec3::new(1.0, 1.0, 1.0));

        let lk = luminance(&kd).max(0.0);
        let lt = luminance(&kt).max(0.0);
        let denom = (lk + lt).max(1e-12);
        let prob_t = (lt / denom).clamp(0.0, 1.0);

        Translucent { kd, kt, prob_t }
    }

    #[inline]
    fn same_hemisphere(woz: f64, wiz: f64) -> bool {
        (woz >= 0.0 && wiz >= 0.0) || (woz < 0.0 && wiz < 0.0)
    }

    #[inline]
    fn eval_reflection(&self, _wo: &Vec3, wi: &Vec3) -> Color3 {
        // f*cos = (kd/PI) * cos, only when wi is on same side (handled by caller)
        let cos_i = wi.z.abs().max(0.0);
        (self.kd / PI) * cos_i
    }

    #[inline]
    fn eval_transmission(&self, _wo: &Vec3, wi: &Vec3) -> Color3 {
        // f*cos = (kt/PI) * |cos| on the opposite side
        let cos_i = wi.z.abs().max(0.0);
        (self.kt / PI) * cos_i
    }

    #[inline]
    fn pdf_reflection(&self, _wo: &Vec3, wi: &Vec3) -> f64 {
        // cosine PDF on the reflection hemisphere
        // pdf_cosine_hemisphere expects z>=0. Mirror logic by using |z| since we work in local frame
        let mut v = *wi;
        v.z = v.z.abs();
        pdf_cosine_hemisphere(&v)
    }

    #[inline]
    fn pdf_transmission(&self, _wo: &Vec3, wi: &Vec3) -> f64 {
        // cosine PDF on the transmission hemisphere (opposite side)
        let mut v = *wi;
        v.z = v.z.abs();
        pdf_cosine_hemisphere(&v)
    }
}

impl Material for Translucent {
    fn sample(&self, wo: &Vec3, sample: &Vec2) -> Option<SampledDirection> {
        if wo.magnitude2() == 0.0 {
            return None;
        }

        // Which hemisphere is "reflection" for this hit?
        // If wo.z >= 0: reflection hemisphere is +Z, transmission is -Z.
        // If wo.z  < 0: reflection hemisphere is -Z, transmission is +Z.
        let refl_sign = if wo.z >= 0.0 { 1.0 } else { -1.0 };
        let tran_sign = -refl_sign;

        // Hierarchical pick: transmission with prob_t, reflection otherwise.
        let mut xi = *sample;
        let choose_t = self.prob_t > 0.0 && (self.prob_t >= 1.0 || xi.x < self.prob_t);
        let denom = if choose_t {
            self.prob_t.max(1e-12)
        } else {
            (1.0 - self.prob_t).max(1e-12)
        };
        xi.x = if choose_t {
            xi.x / denom
        } else {
            (xi.x - self.prob_t).max(0.0) / denom
        };

        // Sample cosine hemisphere, then flip to the proper side
        let mut v = sample_cosine_hemisphere(&xi); // z >= 0
        if choose_t {
            v.z *= tran_sign; // send to opposite side
        } else {
            v.z *= refl_sign; // same side as wo
        }
        let wi = v;

        // Mixture reweighting
        let pdf = self.pdf(wo, &wi);
        if pdf <= 0.0 {
            return None;
        }
        let fcos = self.evaluate(wo, &wi);
        Some(SampledDirection {
            wi,
            weight: fcos / pdf,
        })
    }

    fn emission(&self, _: &Vec3) -> Color3 {
        Color3::zero()
    }
    fn have_emission(&self) -> bool {
        false
    }

    fn evaluate(&self, wo: &Vec3, wi: &Vec3) -> Color3 {
        if wi.z == 0.0 {
            return Color3::zero();
        }
        // Reflection if wi on same side as wo; transmission otherwise
        if Self::same_hemisphere(wo.z, wi.z) {
            self.eval_reflection(wo, wi)
        } else {
            self.eval_transmission(wo, wi)
        }
    }

    fn pdf(&self, wo: &Vec3, wi: &Vec3) -> f64 {
        // Mixture PDF: p = p_t * p_T + (1-p_t) * p_R, where each side only supports its hemisphere
        let p_r = if Self::same_hemisphere(wo.z, wi.z) {
            self.pdf_reflection(wo, wi)
        } else {
            0.0
        };
        let p_t = if !Self::same_hemisphere(wo.z, wi.z) {
            self.pdf_transmission(wo, wi)
        } else {
            0.0
        };
        self.prob_t * p_t + (1.0 - self.prob_t) * p_r
    }

    fn have_delta(&self) -> bool {
        // Diffuse lobes -> non-delta
        false
    }
}
