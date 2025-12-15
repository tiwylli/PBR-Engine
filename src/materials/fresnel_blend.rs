use super::{Material, SampledDirection};
// Nos imports de render (notre libraire)
use crate::fresnel;
use crate::materials::json_to_material;
use crate::texture::{Texture, json_to_texture_float};
use crate::vec::{Color3, Point3, Vec2, Vec3};
use std::{collections::HashMap, sync::Arc};
use tinyjson::JsonValue;

pub struct FresnelBlend {
    eta: Texture<f64>,
    mat_a: Arc<dyn Material>,
    mat_b: Arc<dyn Material>,
    normal_map: Option<Texture<Vec3>>,
}

impl FresnelBlend {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let eta = json_to_texture_float(json, "eta", 1.5);

        let mat_a = json_to_material(json["matA"].get().unwrap());
        let mat_b = json_to_material(json["matB"].get().unwrap());

        let normal_map = super::json_to_normal_map(json);

        Self {
            eta,
            mat_a,
            mat_b,
            normal_map,
        }
    }
}

impl Material for FresnelBlend {
    fn sample(&self, wo: &Vec3, uv: &Vec2, p: &Point3, sample: &Vec2) -> Option<SampledDirection> {
        // Calcul du facteur de Fresnel (on suppose eta_i = 1)
        let cos_theta_i = wo.z.abs();
        let fr = fresnel(cos_theta_i, 1.0, self.eta.get(uv, p));

        let mut rescaled = *sample;
        let picked_mat = if sample.x < fr {
            rescaled.x = sample.x / fr;
            &self.mat_a
        } else {
            rescaled.x = (sample.x - fr) / (1.0 - fr);
            &self.mat_b
        };
        #[allow(clippy::question_mark)]
        let Some(mut sd) = picked_mat.sample(wo, uv, p, &rescaled) else {
            return None;
        };
        if self.have_delta() {
            return Some(sd);
        }
        let pdf_a = self.mat_a.pdf(wo, &sd.wi, uv, p);
        let pdf_b = self.mat_b.pdf(wo, &sd.wi, uv, p);
        // let pdf_mix = sample.x * pdf_a + (1.0 - sample.x) * pdf_b;
        let pdf_mix = sample.x.mul_add(pdf_a, (1.0 - sample.x) * pdf_b);
        let f_a = self.mat_a.evaluate(wo, &sd.wi, uv, p);
        let f_b = self.mat_b.evaluate(wo, &sd.wi, uv, p);
        let f_mix = sample.x * f_a + (1.0 - sample.x) * f_b;

        sd.weight = f_mix / pdf_mix;
        Some(sd)
    }

    fn evaluate(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        if self.have_delta() {
            Color3::new(0.0, 0.0, 0.0)
        } else {
            let cos_theta_i = wo.z.abs();
            let fr = fresnel(cos_theta_i, 1.0, self.eta.get(uv, p));
            fr * self.mat_a.evaluate(wo, wi, uv, p)
                + (1.0 - fr) * self.mat_b.evaluate(wo, wi, uv, p)
        }
    }

    #[allow(clippy::suboptimal_flops)]
    fn pdf(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> f64 {
        if self.have_delta() {
            0.0
        } else {
            let cos_theta_i = wo.z.abs();
            let fr = fresnel(cos_theta_i, 1.0, self.eta.get(uv, p));
            fr * self.mat_a.pdf(wo, wi, uv, p) + (1.0 - fr) * self.mat_b.pdf(wo, wi, uv, p)
        }
    }

    fn have_delta(&self) -> bool {
        self.mat_a.have_delta() || self.mat_b.have_delta()
    }

    fn emission(&self, wo: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        let cos_theta_i = wo.z.abs();
        let fr = fresnel(cos_theta_i, 1.0, self.eta.get(uv, p));
        fr * self.mat_a.emission(wo, uv, p) + (1.0 - fr) * self.mat_b.emission(wo, uv, p)
    }

    fn have_emission(&self) -> bool {
        self.mat_a.have_emission() || self.mat_b.have_emission()
    }

    fn get_normal_map_value(&self, uv: &Vec2, p: &Point3) -> Vec3 {
        super::get_normal_map_value_helper(self.normal_map.as_ref(), uv, p)
    }

    fn have_normal_map(&self) -> bool {
        self.normal_map.is_some()
    }
}
