use std::collections::HashMap;

use cgmath::{Array, ElementWise, InnerSpace, Zero};
use tinyjson::JsonValue;

use crate::{
    array2d::Array2d,
    json::json_to_f64,
    ray::Ray,
    samplers::Sampler,
    scene::Scene,
    vec::{Color3, Frame, Vec3},
};

use super::{Integrator, SamplerIntegrator, render};

pub struct PathMisIntegrator {
    max_depth: usize,
}

impl PathMisIntegrator {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        Self {
            max_depth: json_to_f64(json, "max_depth", 16.0) as usize,
        }
    }
}

impl Integrator for PathMisIntegrator {
    fn render(&mut self, scene: &Scene, sampler: &mut dyn Sampler) -> Array2d<Color3> {
        render(self, scene, sampler)
    }
}
impl SamplerIntegrator for PathMisIntegrator {
    fn preprocess(&mut self, _: &Scene, _: &mut dyn Sampler) {}

    fn li(&self, ray: &Ray, scene: &Scene, sampler: &mut dyn Sampler) -> Color3 {
        let mut throughput = Color3::zero();
        let mut weight = Vec3::from_value(1.0);
        let mut ray = *ray;
        let mut scene_hit = scene.hit(&ray);
        let mut is_primary = true;
        for _ in 0..self.max_depth {
            if let Some(intersection) = scene_hit {
                let frame = Frame::new(&intersection.n);
                let dir_world = -ray.d;
                let dir_local = frame.to_local(&dir_world);

                if intersection.material.have_emission() {
                    if is_primary {
                        return intersection.material.emission(
                            &dir_local,
                            &intersection.uv,
                            &intersection.p,
                        );
                    }
                    return throughput;
                }

                let sampled_direction = intersection.material.sample(
                    &dir_local,
                    &intersection.uv,
                    &intersection.p,
                    &sampler.next2d(),
                );

                // start direct
                let mut direct = Color3::zero();

                // emitter sampling
                {
                    let (es, shape) = scene.sample_direct(&intersection.p, &sampler.next2d());
                    if scene.visible(&intersection.p, &es.y) && !intersection.material.have_delta()
                    {
                        let wi_world = (es.y - intersection.p).normalize();
                        let wi_local = frame.to_local(&wi_world);
                        let pdf_bsdf = intersection.material.pdf(
                            &dir_local,
                            &wi_local,
                            &intersection.uv,
                            &intersection.p,
                        );
                        let mis_w = es.pdf / (pdf_bsdf + es.pdf);

                        let frame_light = Frame::new(&es.n);
                        direct += mis_w
                            * shape
                                .material()
                                .emission(
                                    &frame_light.to_local(&-wi_world),
                                    &intersection.uv,
                                    &intersection.p,
                                )
                                .mul_element_wise(intersection.material.evaluate(
                                    &dir_local,
                                    &wi_local,
                                    &intersection.uv,
                                    &intersection.p,
                                ))
                            / es.pdf;
                    }
                }

                if sampled_direction.is_none() {
                    return throughput + weight.mul_element_wise(direct);
                }

                let sampled_direction = sampled_direction.unwrap();

                // bsdf sampling
                // cast new ray to try and find a light source
                ray = Ray::new(&intersection.p, &frame.to_world(&sampled_direction.wi));
                scene_hit = scene.hit(&ray);

                if let Some(light_intersection) = &scene_hit
                    && light_intersection.material.have_emission()
                {
                    let mis_w = if intersection.material.have_delta() {
                        1.0
                    } else {
                        let pdf_emitter = light_intersection.shape.pdf_direct(
                            light_intersection.shape,
                            &intersection.p,
                            &light_intersection.p,
                            &light_intersection.n,
                        );
                        let pdf_bsdf = intersection.material.pdf(
                            &dir_local,
                            &sampled_direction.wi,
                            &intersection.uv,
                            &intersection.p,
                        );
                        pdf_bsdf / (pdf_bsdf + pdf_emitter)
                    };

                    let frame_light = Frame::new(&light_intersection.n);

                    let light_contribution = light_intersection.material.emission(
                        &frame_light.to_local(&-ray.d),
                        &intersection.uv,
                        &intersection.p,
                    );

                    direct += mis_w
                        * sampled_direction
                            .weight
                            .mul_element_wise(light_contribution);
                }
                // end direct

                throughput += weight.mul_element_wise(direct);

                weight = weight.mul_element_wise(sampled_direction.weight);
                is_primary = false;
            } else {
                return throughput + weight.mul_element_wise(scene.background(ray.d));
            }
        }
        Color3::zero()
    }
}
