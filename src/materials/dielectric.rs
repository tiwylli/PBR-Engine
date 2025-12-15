use std::collections::HashMap;

use cgmath::{InnerSpace, Zero};
use tinyjson::JsonValue;

use crate::{
    json::json_to_f64,
    materials::roughness_to_exponent,
    samplers::{pdf_cosine_hemisphere_power, sample_cosine_hemisphere_power},
    texture::{Texture, json_to_texture},
    vec::{Color3, Frame, Point3, Vec2, Vec3},
};

use super::{Material, SampledDirection};

pub struct Dielectric {
    ks: Texture<Color3>,
    eta_ext: f64,
    eta_int: f64,
    exponent: Option<f64>,
    normal_map: Option<Texture<Vec3>>,
}

impl Dielectric {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let ks = json_to_texture(json, "ks", Vec3::new(1.0, 1.0, 1.0));
        let eta_ext = json_to_f64(json, "eta_ext", 1.0); // eta_i dans les diapositives, si le rayon arrive de l'air
        let eta_int = json_to_f64(json, "eta_int", 1.5); // eta_t dans les diapositives, si le rayon arrive de l'air
        let roughness = json_to_f64(json, "roughness", 0.0);
        let normal_map = super::json_to_normal_map(json);
        Self {
            ks,
            eta_ext,
            eta_int,
            exponent: roughness_to_exponent(roughness),
            normal_map,
        }
    }
}

impl Material for Dielectric {
    fn sample(&self, wo: &Vec3, uv: &Vec2, p: &Point3, s: &Vec2) -> Option<SampledDirection> {
        let under_surface = wo.z < 0.0;
        #[rustfmt::skip] let eta_t = if under_surface { self.eta_ext } else { self.eta_int };
        #[rustfmt::skip] let eta_i = if under_surface { self.eta_int } else { self.eta_ext };

        let cos_theta_i = wo.z.abs();
        let sin_theta_i = cos_theta_i.mul_add(-cos_theta_i, 1.0).sqrt();
        let sin_theta_t = sin_theta_i * eta_i / eta_t;

        let compute_wi = |reflected| {
            self.exponent.map_or(reflected, |exponent| {
                let frame = Frame::new(&reflected);
                frame.to_world(&sample_cosine_hemisphere_power(s, exponent))
            })
        };

        if sin_theta_t > 1.0 {
            // total internal reflection
            let wi = compute_wi(Vec3::new(-wo.x, -wo.y, wo.z));
            Some(SampledDirection {
                weight: self.ks.get(uv, p),
                wi,
            })
        } else {
            let f0 = ((eta_i - eta_t) / (eta_i + eta_t)).powi(2);
            let fr = (1.0 - f0).mul_add((1.0 - cos_theta_i).powi(5), f0);
            if s.x < fr {
                // specular reflection
                let wi = compute_wi(Vec3::new(-wo.x, -wo.y, wo.z));
                Some(SampledDirection {
                    weight: self.ks.get(uv, p),
                    wi,
                })
            } else {
                // refraction
                let reflected = -(eta_i / eta_t) * wo
                    + (eta_i / eta_t).mul_add(cos_theta_i, -sin_theta_t.asin().cos())
                        * wo.z.signum()
                        * Vec3::unit_z();
                let wi = compute_wi(reflected.normalize());
                Some(SampledDirection {
                    weight: self.ks.get(uv, p),
                    wi,
                })
            }
        }
    }

    fn evaluate(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        if self.have_delta() {
            Color3::zero()
        } else {
            self.ks.get(uv, p) * self.pdf(wo, wi, uv, p)
        }
    }

    fn pdf(&self, wo: &Vec3, wi: &Vec3, _uv: &Vec2, _p: &Point3) -> f64 {
        if self.have_delta() {
            0.0
        } else {
            let frame = Frame::new(&Vec3::new(-wo.x, -wo.y, wo.z));
            pdf_cosine_hemisphere_power(&frame.to_local(wi), self.exponent.unwrap())
        }
    }

    fn have_delta(&self) -> bool {
        self.exponent.is_none()
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
}
