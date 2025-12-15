use std::{collections::HashMap, f64::consts::PI};

use cgmath::{InnerSpace, Zero};
use tinyjson::JsonValue;

use crate::{
    samplers::{pdf_cosine_hemisphere, sample_cosine_hemisphere},
    texture::{Texture, json_to_texture},
    vec::{Color3, Point3, Vec2, Vec3},
};

use super::{Material, SampledDirection};
// ChadGPT assisted in writing this material for missing knowledge and code architecture

pub struct OrenNayar {
    albedo: Texture<Color3>,
    a: f64, // coefficient A
    b: f64, // coefficient B
    normal_map: Option<Texture<Vec3>>,
}

impl OrenNayar {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let albedo = json_to_texture(json, "albedo", Vec3::new(0.8, 0.8, 0.8));

        // Lecture de la rugosité: "sigma_deg" (prioritaire) ou "sigma" (radians)
        let sigma_deg = match json.get("sigma_deg") {
            Some(JsonValue::Number(n)) => Some(*n),
            _ => None,
        };

        let mut sigma = sigma_deg.map_or_else(
            || {
                if let Some(JsonValue::Number(n)) = json.get("sigma") {
                    *n
                } else {
                    0.3 // ~17°
                }
            },
            f64::to_radians,
        );
        sigma = sigma.clamp(0.0, std::f64::consts::FRAC_PI_2); // [0, 90°]

        // Coefficients classiques Oren–Nayar
        let sigma2 = sigma * sigma;
        let a = 1.0 - (sigma2 / (2.0 * (sigma2 + 0.33)));
        let b = 0.45 * (sigma2 / (sigma2 + 0.09));
        let normal_map = super::json_to_normal_map(json);

        Self {
            albedo,
            a,
            b,
            normal_map,
        }
    }

    #[inline]
    fn on_factor(&self, wo: &Vec3, wi: &Vec3) -> f64 {
        // n = (0,0,1), on travaille dans l'espace local
        let cos_i = wi.z.max(0.0);
        let cos_o = wo.z.max(0.0);
        if cos_i <= 0.0 || cos_o <= 0.0 {
            return 0.0;
        }

        let sin_i = cos_i.mul_add(-cos_i, 1.0).max(0.0).sqrt();
        let sin_o = cos_o.mul_add(-cos_o, 1.0).max(0.0).sqrt();

        // cos(phi_i - phi_o) via projections dans le plan tangent
        let cos_dphi = if sin_i > 1e-7 && sin_o > 1e-7 {
            let wi_xy = Vec3::new(wi.x, wi.y, 0.0) / sin_i;
            let w_o_xy = Vec3::new(wo.x, wo.y, 0.0) / sin_o;
            wi_xy.dot(w_o_xy).clamp(-1.0, 1.0)
        } else {
            0.0
        };

        // alpha = max(theta_i, theta_o), beta = min(theta_i, theta_o)
        // => en termes de cosines : si cos_i > cos_o alors theta_i < theta_o
        // donc alpha = theta_o, beta = theta_i, et vice-versa.
        let (sin_alpha, tan_beta) = if cos_i > cos_o {
            // alpha = theta_o, beta = theta_i
            let tan_b = if cos_i > 1e-7 { sin_i / cos_i } else { 0.0 };
            (sin_o, tan_b)
        } else {
            // alpha = theta_i, beta = theta_o
            let tan_b = if cos_o > 1e-7 { sin_o / cos_o } else { 0.0 };
            (sin_i, tan_b)
        };

        // Terme Oren–Nayar: A + B * max(0, cos(dphi)) * sin(alpha) * tan(beta)
        // self.a + self.b * cos_dphi.max(0.0) * sin_alpha * tan_beta
        (self.b * cos_dphi.max(0.0) * sin_alpha).mul_add(tan_beta, self.a)
    }
}

impl Material for OrenNayar {
    fn sample(&self, wo: &Vec3, uv: &Vec2, p: &Point3, sample: &Vec2) -> Option<SampledDirection> {
        if wo.z < 0.0 {
            return None;
        }

        // Importance sampling cosinus (comme Diffuse)
        let wi = sample_cosine_hemisphere(sample);
        if wi.z <= 0.0 {
            return None;
        }

        // Ici evaluate() renvoie f*cos. Avec pdf = cos/PI,
        // le poids optimal vaut fcos / pdf = albedo * ON_factor
        let factor = self.on_factor(wo, &wi);
        Some(SampledDirection {
            wi,
            weight: self.albedo.get(uv, p) * factor,
        })
    }

    fn emission(&self, _: &Vec3, _uv: &Vec2, _p: &Point3) -> Color3 {
        Color3::zero()
    }

    fn have_emission(&self) -> bool {
        false
    }

    fn evaluate(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        // f_r * cos(theta_i) = (albedo/PI) * [A + B * cos(dphi)^+ * sin(alpha) * tan(beta)] * cos(theta_i)
        if wi.z <= 0.0 || wo.z <= 0.0 {
            return Color3::zero();
        }
        let factor = self.on_factor(wo, wi);
        let cos_i = wi.z.max(0.0);
        (self.albedo.get(uv, p) / PI) * factor * cos_i
    }

    fn pdf(&self, _wo: &Vec3, wi: &Vec3, _uv: &Vec2, _p: &Point3) -> f64 {
        // Même PDF que le Lambert (tirage cosinus)
        pdf_cosine_hemisphere(wi)
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
