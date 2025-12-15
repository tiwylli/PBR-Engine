use std::{collections::HashMap, sync::Arc};

use tinyjson::JsonValue;

use crate::{
    materials::{SampledDirection, json_to_material},
    texture::{Texture, json_to_texture_float},
    vec::{Color3, Point3, Vec2, Vec3},
};

use cgmath::Zero;

use super::Material;

pub struct Blend {
    prob: Texture<f64>,
    mat_a: Arc<dyn Material>,
    mat_b: Arc<dyn Material>,
    normal_map: Option<Texture<Vec3>>,
}

impl Blend {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let prob = json_to_texture_float(json, "alpha", 1.5);

        let mat_a = json_to_material(
            json.get("matA")
                .expect("Blend material should have a matA child material")
                .get()
                .unwrap(),
        );
        let mat_b = json_to_material(
            json.get("matB")
                .expect("Blend material should have a matB child material")
                .get()
                .unwrap(),
        );
        let normal_map = super::json_to_normal_map(json);
        Self {
            prob,
            mat_a,
            mat_b,
            normal_map,
        }
    }
}

impl Material for Blend {
    fn sample(&self, wo: &Vec3, uv: &Vec2, p: &Point3, s: &Vec2) -> Option<SampledDirection> {
        let prob = self.prob.get(uv, p);
        let mut s = *s;
        let res = if s.x < prob {
            s.x /= prob;
            self.mat_a.sample(wo, uv, p, &s)
        } else {
            s.x = (s.x - prob) / (1.0 - prob);
            self.mat_b.sample(wo, uv, p, &s)
        };

        // Correction: missing the mixture weighting
        #[allow(clippy::unnecessary_unwrap)]
        if res.is_some() && (!self.have_delta()) {
            let mut sampled = res.unwrap();
            let pdf_v = self.pdf(wo, &sampled.wi, uv, p);
            if pdf_v > 0.0 {
                sampled.weight = self.evaluate(wo, &sampled.wi, uv, p) / pdf_v;
                Some(sampled)
            } else {
                None
            }
        } else {
            res
        }
    }

    fn evaluate(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> Color3 {
        if self.have_delta() {
            Color3::zero()
        } else {
            let prob = self.prob.get(uv, p);
            prob * self.mat_a.evaluate(wo, wi, uv, p)
                + (1.0 - prob) * self.mat_b.evaluate(wo, wi, uv, p)
        }
    }

    #[allow(clippy::suboptimal_flops)]
    fn pdf(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> f64 {
        if self.have_delta() {
            0.0
        } else {
            let prob = self.prob.get(uv, p);
            prob * self.mat_a.pdf(wo, wi, uv, p) + (1.0 - prob) * self.mat_b.pdf(wo, wi, uv, p)
        }
    }

    fn have_delta(&self) -> bool {
        self.mat_a.have_delta() || self.mat_b.have_delta()
    }

    fn emission(&self, _wo: &Vec3, _uv: &Vec2, _p: &Point3) -> Color3 {
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
