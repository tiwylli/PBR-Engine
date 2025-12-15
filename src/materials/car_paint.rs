use std::{collections::HashMap, f64::consts::PI};

use cgmath::{InnerSpace, Zero};
use tinyjson::JsonValue;

use super::{Material, SampledDirection};
use crate::{
    samplers::{pdf_cosine_hemisphere, sample_cosine_hemisphere},
    texture::{Texture, json_to_texture},
    vec::{Color3, Point3, Vec2, Vec3},
};
//ChadGPT assisted in writing this material for missing knowledge and code architecture
pub struct CarPaint {
    // Base diffuse pigment
    base_color: Texture<Color3>,
    // GGX clearcoat roughness alpha in [0,1]
    clearcoat_alpha: f64,
    // Discrete sampling weight for the clearcoat lobe in [0,1]
    clearcoat_weight: f64,
    // Fresnel F0 for the clearcoat (dielectric clear layer ~0.04 by default)
    clearcoat_f0: f64,
    normal_map: Option<Texture<Vec3>>,
}

impl CarPaint {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let base_color = json_to_texture(json, "base_color", Vec3::new(0.8, 0.1, 0.1));
        let clearcoat_roughness =
            if let Some(JsonValue::Number(n)) = json.get("clearcoat_roughness") {
                (*n).clamp(0.0, 1.0)
            } else {
                0.1
            };
        let clearcoat_weight = if let Some(JsonValue::Number(n)) = json.get("clearcoat_weight") {
            (*n).clamp(0.0, 1.0)
        } else {
            0.5
        };
        let clearcoat_f0 = if let Some(JsonValue::Number(n)) = json.get("clearcoat_f0") {
            (*n).clamp(0.0, 1.0)
        } else {
            0.04
        };
        let normal_map = super::json_to_normal_map(json);

        Self {
            base_color,
            clearcoat_alpha: (clearcoat_roughness * clearcoat_roughness).max(1e-4), // square roughness
            clearcoat_weight,
            clearcoat_f0,
            normal_map,
        }
    }

    // ---------- GGX helpers (isotropic, N=(0,0,1)) ----------

    #[inline]
    fn ggx_d(&self, ndoth: f64) -> f64 {
        // Trowbridge-Reitz GGX normal distribution
        let a2 = self.clearcoat_alpha * self.clearcoat_alpha;
        let c = ndoth.max(0.0);
        // replace (c * c * (a2 - 1.0) + 1.0) with mul_add
        a2 / (PI * ((a2 - 1.0).mul_add(c * c, 1.0)).powi(2))
    }

    #[inline]
    fn smith_g1(&self, ndotv: f64) -> f64 {
        // Schlick-GGX G1
        let a = self.clearcoat_alpha;
        let c = ndotv.max(0.0);
        //let k = (a * a + 1.0).sqrt(); // optional variant; use exact Smith:
        // exact Smith for GGX:
        let a2 = a * a;
        // let denom = c + (a2 + (1.0 - a2) * c * c).sqrt();
        let denom = c + ((1.0 - a2) * c).mul_add(c, a2).sqrt();
        (2.0 * c / denom).min(1.0)
    }

    #[inline]
    fn smith_g(&self, ndot_v: f64, ndot_l: f64) -> f64 {
        self.smith_g1(ndot_v) * self.smith_g1(ndot_l)
    }

    #[inline]
    fn fresnel_schlick(&self, cos_theta: f64) -> f64 {
        // scalar Fresnel for dielectric clearcoat
        let f0 = self.clearcoat_f0;
        // f0 + (1.0 - f0) * (1.0 - cos_theta.max(0.0)).powi(5)
        (1.0 - f0).mul_add((1.0 - cos_theta.max(0.0)).powi(5), f0)
    }

    #[inline]
    fn sample_ggx_normal(&self, xi: &Vec2) -> Vec3 {
        // Sample half-vector h around +Z using GGX NDF
        // phi = 2πu, tan^2θ = a^2 * v / (1 - v)
        let phi = 2.0 * PI * xi.y;
        let a = self.clearcoat_alpha;
        let v = xi.x.clamp(1e-12, 1.0 - 1e-12);
        let t2 = a * a * v / (1.0 - v);
        let cos_theta = 1.0 / (1.0 + t2).sqrt();
        let sin_theta = cos_theta.mul_add(-cos_theta, 1.0).max(0.0).sqrt();

        let (sin_phi, cos_phi) = phi.sin_cos();
        Vec3::new(cos_phi * sin_theta, sin_phi * sin_theta, cos_theta)
    }

    #[inline]
    fn pdf_ggx_reflection(&self, wo: &Vec3, wi: &Vec3) -> f64 {
        // p(wi) = D(h) * (n·h) / (4 |wo·h|)
        let h = (*wo + *wi).normalize(); // reflection half-vector
        let ndoth = h.z.max(0.0);
        let wodoth = wo.dot(h).abs().max(1e-12);
        self.ggx_d(ndoth) * ndoth / (4.0 * wodoth)
    }

    #[inline]
    fn eval_clearcoat_fcos(&self, wo: &Vec3, wi: &Vec3) -> Color3 {
        // Microfacet reflection: f = (D * F * G) / (4 n·wo n·wi)
        // Return f * cos(theta_i) = (D * F * G / (4 n·wo)) * (n·wi)
        let ndotw_o = wo.z.max(0.0);
        let ndotw_i = wi.z.max(0.0);
        if ndotw_o <= 0.0 || ndotw_i <= 0.0 {
            return Color3::zero();
        }
        let h = (*wo + *wi).normalize();
        let ndoth = h.z.max(0.0);
        let vodoth = wo.dot(h).max(0.0);

        let d = self.ggx_d(ndoth);
        let g = self.smith_g(ndotw_o, ndotw_i);
        let f = self.fresnel_schlick(vodoth);

        // scalar clearcoat (colorless), return as RGB
        let spec = (d * g * f / (4.0 * ndotw_o)) * ndotw_i;
        Vec3::new(spec, spec, spec)
    }

    #[inline]
    fn eval_base_fcos(&self, _wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        // Lambert base: f*cos = (base / π) * cos
        let cos_i = wi.z.max(0.0);
        (self.base_color.get(uv, p) / PI) * cos_i
    }

    #[inline]
    const fn mixture_prob(&self) -> f64 {
        // Discrete sampling prob. for clearcoat lobe
        self.clearcoat_weight.clamp(0.0, 1.0)
    }
}

impl Material for CarPaint {
    fn sample(&self, wo: &Vec3, uv: &Vec2, p: &Point3, sample: &Vec2) -> Option<SampledDirection> {
        if wo.z <= 0.0 {
            return None; // backface (consistent with your other materials)
        }

        let mut xi = *sample;
        let p_spec = self.mixture_prob();
        let choose_spec = p_spec > 0.0 && (p_spec >= 1.0 || xi.x < p_spec);

        // Rescale hierarchical RNG
        let denom = if choose_spec {
            p_spec.max(1e-12)
        } else {
            (1.0 - p_spec).max(1e-12)
        };
        xi.x = if choose_spec {
            xi.x / denom
        } else {
            (xi.x - p_spec).max(0.0) / denom
        };

        let wi = if choose_spec {
            // GGX clearcoat: sample half-vector h, then reflect wo about h
            let h = self.sample_ggx_normal(&xi);
            let wodoth = wo.dot(h);
            let cand = (h * (2.0 * wodoth)) - *wo;
            if cand.z <= 0.0 {
                return None;
            }
            cand.normalize()
        } else {
            // Base diffuse: cosine hemisphere
            sample_cosine_hemisphere(&xi)
        };

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
        if wo.z <= 0.0 || wi.z <= 0.0 {
            return Color3::zero();
        }

        // Layered energy sharing:
        // outgoing is: clearcoat reflection + base seen through clear layer
        // Use Schlick Fresnel at viewing half-vector to modulate energy split.
        let h = (*wo + *wi).normalize();
        let f_coat = self.fresnel_schlick(wo.dot(h).max(0.0)); // scalar

        let coat = self.eval_clearcoat_fcos(wo, wi); // f*cos (RGB, scalar spec replicated)
        let base = self.eval_base_fcos(wo, wi, uv, p) * (1.0 - f_coat); // attenuate base by (1 - F)

        coat + base
    }

    fn pdf(&self, wo: &Vec3, wi: &Vec3, _uv: &Vec2, _p: &Point3) -> f64 {
        if wo.z <= 0.0 || wi.z <= 0.0 {
            return 0.0;
        }
        let p_spec = self.mixture_prob();
        let pdf_spec = self.pdf_ggx_reflection(wo, wi);
        let pdf_diff = pdf_cosine_hemisphere(wi);
        // p_spec * pdf_spec + (1.0 - p_spec) * pdf_diff
        p_spec.mul_add(pdf_spec, (1.0 - p_spec) * pdf_diff)
    }

    fn have_delta(&self) -> bool {
        false
    }

    fn get_normal_map_value(&self, uv: &Vec2, p: &Point3) -> Vec3 {
        super::get_normal_map_value_helper(self.normal_map.as_ref(), uv, p)
    }

    fn have_normal_map(&self) -> bool {
        self.normal_map.is_some()
    }
}
