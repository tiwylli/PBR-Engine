use std::{collections::HashMap, f64::consts::PI};

use cgmath::{InnerSpace, Zero};
use tinyjson::JsonValue;

use super::{Material, SampledDirection};
use crate::{
    samplers::{pdf_cosine_hemisphere, sample_cosine_hemisphere},
    texture::{Texture, json_to_texture},
    vec::{Color3, Point3, Vec2, Vec3, luminance},
};
// ChadGPT assisted in writing this material for missing knowledge and code architecture

/// Translucent (diffuse transmission) with optional diffuse reflection mix.
/// - Reflection: Lambert on the same side as wo
/// - Transmission: Lambert on the opposite side (no refraction), i.e. straight through but diffuse
pub struct Translucent {
    kd: Texture<Color3>, // diffuse reflection albedo
    kt: Texture<Color3>, // diffuse transmission albedo (tint)
    normal_map: Option<Texture<Vec3>>,
}

impl Translucent {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let kd = json_to_texture(json, "kd", Vec3::new(0.0, 0.0, 0.0));
        let kt = json_to_texture(json, "kt", Vec3::new(1.0, 1.0, 1.0));
        let normal_map = super::json_to_normal_map(json);

        Self { kd, kt, normal_map }
    }

    fn get_prob_t(&self, uv: &Vec2, p: &Point3) -> f64 {
        let lk = luminance(&self.kd.get(uv, p)).max(0.0);
        let lt = luminance(&self.kt.get(uv, p)).max(0.0);
        let denom = (lk + lt).max(1e-12);
        (lt / denom).clamp(0.0, 1.0)
    }

    #[inline]
    #[allow(clippy::similar_names)]
    fn same_hemisphere(woz: f64, wiz: f64) -> bool {
        (woz >= 0.0 && wiz >= 0.0) || (woz < 0.0 && wiz < 0.0)
    }

    #[inline]
    fn eval_reflection(&self, _wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        // f*cos = (kd/PI) * cos, only when wi is on same side (handled by caller)
        let cos_i = wi.z.abs().max(0.0);
        (self.kd.get(uv, p) / PI) * cos_i
    }

    #[inline]
    fn eval_transmission(&self, _wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        // f*cos = (kt/PI) * |cos| on the opposite side
        let cos_i = wi.z.abs().max(0.0);
        (self.kt.get(uv, p) / PI) * cos_i
    }

    #[inline]
    fn pdf_reflection(_wo: &Vec3, wi: &Vec3) -> f64 {
        // cosine PDF on the reflection hemisphere
        // pdf_cosine_hemisphere expects z>=0. Mirror logic by using |z| since we work in local frame
        let mut v = *wi;
        v.z = v.z.abs();
        pdf_cosine_hemisphere(&v)
    }

    #[inline]
    fn pdf_transmission(_wo: &Vec3, wi: &Vec3) -> f64 {
        // cosine PDF on the transmission hemisphere (opposite side)
        let mut v = *wi;
        v.z = v.z.abs();
        pdf_cosine_hemisphere(&v)
    }
}

impl Material for Translucent {
    fn sample(&self, wo: &Vec3, uv: &Vec2, p: &Point3, sample: &Vec2) -> Option<SampledDirection> {
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
        let prob_t = self.get_prob_t(uv, p);
        let choose_t = prob_t > 0.0 && (prob_t >= 1.0 || xi.x < prob_t);
        let denom = if choose_t {
            prob_t.max(1e-12)
        } else {
            (1.0 - prob_t).max(1e-12)
        };
        xi.x = if choose_t {
            xi.x / denom
        } else {
            (xi.x - prob_t).max(0.0) / denom
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

    fn evaluate(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        if wi.z == 0.0 {
            return Color3::zero();
        }
        // Reflection if wi on same side as wo; transmission otherwise
        if Self::same_hemisphere(wo.z, wi.z) {
            self.eval_reflection(wo, wi, uv, p)
        } else {
            self.eval_transmission(wo, wi, uv, p)
        }
    }

    fn pdf(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> f64 {
        // Mixture PDF: p = p_t * p_T + (1-p_t) * p_R, where each side only supports its hemisphere
        let p_r = if Self::same_hemisphere(wo.z, wi.z) {
            Self::pdf_reflection(wo, wi)
        } else {
            0.0
        };

        let p_t = if Self::same_hemisphere(wo.z, wi.z) {
            0.0
        } else {
            Self::pdf_transmission(wo, wi)
        };

        let prob_t = self.get_prob_t(uv, p);
        prob_t.mul_add(p_t, (1.0 - prob_t) * p_r)
    }

    fn have_delta(&self) -> bool {
        // Diffuse lobes -> non-delta
        false
    }

    fn get_normal_map_value(&self, uv: &Vec2, p: &Point3) -> Vec3 {
        super::get_normal_map_value_helper(self.normal_map.as_ref(), uv, p)
    }

    fn have_normal_map(&self) -> bool {
        self.normal_map.is_some()
    }
}
