use std::collections::HashMap;

use cgmath::{Array, ElementWise, InnerSpace, Zero};
use tinyjson::JsonValue;

use crate::{
    array2d::Array2d,
    json::json_to_f64,
    ray::Ray,
    samplers::Sampler,
    scene::Scene,
    vec::{Color3, Frame},
};

use super::{Integrator, SamplerIntegrator, render};

pub struct PathIntegrator {
    max_depth: usize,
}

impl PathIntegrator {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        Self {
            max_depth: json_to_f64(json, "max_depth", 16.0) as usize,
        }
    }
}

impl Integrator for PathIntegrator {
    fn render(&mut self, scene: &Scene, sampler: &mut dyn Sampler) -> Array2d<Color3> {
        render(self, scene, sampler)
    }
}
impl SamplerIntegrator for PathIntegrator {
    fn preprocess(&mut self, _: &Scene, _: &mut dyn Sampler) {}

    fn li(&self, ray: &Ray, scene: &Scene, sampler: &mut dyn Sampler) -> Color3 {
        let mut throughput = Color3::from_value(1.0);
        let mut ray = *ray;
        for _ in 0..self.max_depth {
            if let Some(intersection) = scene.hit(&ray) {
                let frame = Frame::new(&intersection.n);
                let dir_world = -ray.d;
                let dir_local = frame.to_local(&dir_world);

                if let Some(sampled_direction) = intersection.material.sample(
                    &dir_local,
                    &intersection.uv,
                    &intersection.p,
                    &sampler.next2d(),
                ) {
                    throughput = throughput.mul_element_wise(sampled_direction.weight);
                    ray = Ray::new(
                        &intersection.p,
                        &frame.to_world(&sampled_direction.wi).normalize(),
                    );
                } else {
                    return throughput.mul_element_wise(intersection.material.emission(
                        &dir_local,
                        &intersection.uv,
                        &intersection.p,
                    ));
                }
            } else {
                return throughput.mul_element_wise(scene.background(ray.d));
            }
        }
        Color3::zero()
    }
}
