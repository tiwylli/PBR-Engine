use super::{Material, SampledDirection};
use crate::materials::json_to_material;
use crate::vec::{Color3, Vec2, Vec3};
use cgmath::Zero;
use std::collections::HashMap;
use std::sync::Arc;
use tinyjson::JsonValue;

pub struct Blend {
    alpha: f64, // Peut être une texture (eventuellement) ou une constante
    mat_a: Arc<dyn Material>,
    mat_b: Arc<dyn Material>,
}

impl Blend {
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Blend {
        let alpha = if let Some(JsonValue::Number(a)) = json.get("alpha") {
            *a as f64
        } else {
            0.5 // valeur par défaut
        };

        let mat_a = json_to_material(json["matA"].get().unwrap());
        let mat_b = json_to_material(json["matB"].get().unwrap());

        Blend {
            alpha,
            mat_a,
            mat_b,
        }
    }
}

impl Material for Blend {
    fn sample(&self, wo: &Vec3, sample: &Vec2) -> Option<SampledDirection> {
        let mut rescaled = *sample;
        let picked_mat;
        if sample.x < self.alpha {
            rescaled.x = sample.x / self.alpha;
            picked_mat = &self.mat_a;
        } else {
            rescaled.x = (sample.x - self.alpha) / (1.0 - self.alpha);
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
        //TODO mixture seulement si pas de delta???
        if !self.have_delta() {
            self.alpha * self.mat_a.evaluate(wo, wi)
                + (1.0 - self.alpha) * self.mat_b.evaluate(wo, wi)
        } else {
            // Color3::new(0.0, 0.0, 0.0)
            Color3::zero()
        }
    }

    fn pdf(&self, wo: &Vec3, wi: &Vec3) -> f64 {
        //same as evaluate
        if !self.have_delta() {
            self.alpha * self.mat_a.pdf(wo, wi) + (1.0 - self.alpha) * self.mat_b.pdf(wo, wi)
        } else {
            0.0
        }
    }

    fn have_delta(&self) -> bool {
        self.mat_a.have_delta() || self.mat_b.have_delta()
    }

    fn emission(&self, wo: &Vec3) -> Color3 {
        self.alpha * self.mat_a.emission(wo) + (1.0 - self.alpha) * self.mat_b.emission(wo)
    }

    fn have_emission(&self) -> bool {
        self.mat_a.have_emission() || self.mat_b.have_emission()
    }
}
