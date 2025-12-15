use super::{Material, SampledDirection};
// Nos imports de render (notre libraire)
use crate::fresnel;
use crate::materials::json_to_material;
use crate::vec::{Color3, Vec2, Vec3};
use std::{collections::HashMap, sync::Arc};
use tinyjson::JsonValue;

pub struct FresnelBlend {
    eta: f64,
    mat_a: Arc<dyn Material>,
    mat_b: Arc<dyn Material>,
}

impl FresnelBlend {
    pub fn from_json(json: &HashMap<String, JsonValue>) -> FresnelBlend {
        let eta = if let Some(JsonValue::Number(e)) = json.get("eta") {
            *e as f64
        } else {
            1.5
        };

        let mat_a = json_to_material(json["matA"].get().unwrap());
        let mat_b = json_to_material(json["matB"].get().unwrap());

        FresnelBlend { eta, mat_a, mat_b }
    }
}

impl Material for FresnelBlend {
    fn sample(&self, wo: &Vec3, sample: &Vec2) -> Option<SampledDirection> {
        // Calcul du facteur de Fresnel (on suppose eta_i = 1)
        let cos_theta_i = wo.z.abs();
        let fr = fresnel(cos_theta_i, 1.0, self.eta);

        let mut rescaled = *sample;
        let picked_mat;
        if sample.x < fr {
            rescaled.x = sample.x / fr;
            picked_mat = &self.mat_a;
        } else {
            rescaled.x = (sample.x - fr) / (1.0 - fr);
            picked_mat = &self.mat_b;
        }
        let Some(mut sd) = picked_mat.sample(wo, &rescaled) else {
            return None;
        };
        if self.have_delta() {
            return Some(sd);
        }
        let pdf_a = self.mat_a.pdf(wo, &sd.wi);
        let pdf_b = self.mat_b.pdf(wo, &sd.wi);
        let pdf_mix = sample.x * pdf_a + (1.0 - sample.x) * pdf_b;

        let f_a = self.mat_a.evaluate(wo, &sd.wi);
        let f_b = self.mat_b.evaluate(wo, &sd.wi);
        let f_mix = sample.x * f_a + (1.0 - sample.x) * f_b;

        sd.weight = f_mix / pdf_mix;
        return Some(sd);
    }

    fn evaluate(&self, wo: &Vec3, wi: &Vec3) -> Color3 {
        if !self.have_delta() {
            let cos_theta_i = wo.z.abs();
            let fr = fresnel(cos_theta_i, 1.0, self.eta);
            fr * self.mat_a.evaluate(wo, wi) + (1.0 - fr) * self.mat_b.evaluate(wo, wi)
        } else {
            Color3::new(0.0, 0.0, 0.0)
        }
    }

    fn pdf(&self, wo: &Vec3, wi: &Vec3) -> f64 {
        if !self.have_delta() {
            let cos_theta_i = wo.z.abs();
            let fr = fresnel(cos_theta_i, 1.0, self.eta);
            fr * self.mat_a.pdf(wo, wi) + (1.0 - fr) * self.mat_b.pdf(wo, wi)
        } else {
            0.0
        }
    }

    fn have_delta(&self) -> bool {
        self.mat_a.have_delta() || self.mat_b.have_delta()
    }

    fn emission(&self, wo: &Vec3) -> Color3 {
        let cos_theta_i = wo.z.abs();
        let fr = fresnel(cos_theta_i, 1.0, self.eta);
        fr * self.mat_a.emission(wo) + (1.0 - fr) * self.mat_b.emission(wo)
    }

    fn have_emission(&self) -> bool {
        self.mat_a.have_emission() || self.mat_b.have_emission()
    }
}
