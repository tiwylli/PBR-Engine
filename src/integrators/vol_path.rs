use std::collections::HashMap;

use cgmath::{Array, ElementWise, InnerSpace, Zero};
use tinyjson::JsonValue;

use crate::{
    array2d::Array2d,
    json::json_to_f64,
    medium::MediumSample,
    ray::Ray,
    samplers::Sampler,
    scene::Scene,
    vec::{Color3, Frame},
};

use super::{Integrator, SamplerIntegrator, render};

pub struct VolumetricPathIntegrator {
    max_depth: usize,
}

impl VolumetricPathIntegrator {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        Self {
            max_depth: json_to_f64(json, "max_depth", 16.0) as usize,
        }
    }
}

impl Integrator for VolumetricPathIntegrator {
    fn render(&mut self, scene: &Scene, sampler: &mut dyn Sampler) -> Array2d<Color3> {
        render(self, scene, sampler)
    }
}
impl SamplerIntegrator for VolumetricPathIntegrator {
    fn preprocess(&mut self, _: &Scene, _: &mut dyn Sampler) {}

    fn li(&self, ray: &Ray, scene: &Scene, sampler: &mut dyn Sampler) -> Color3 {
        let mut l = Color3::zero();
        let mut throughput = Color3::from_value(1.0);
        let mut ray = *ray;

        for i in 0..self.max_depth {
            let its_opt = scene.hit(&ray);
            let t_max = its_opt.as_ref().map_or(f64::MAX, |i| i.t);

            if let Some(medium) = &scene.medium {
                match medium.sample(t_max, sampler) {
                    MediumSample::Scatter { t, weight, .. } => {
                        let p = ray.point_at(t);
                        let wo = -ray.d;
                        let wi = medium
                            .phase_function()
                            .sample_p(&wo, sampler.next2d().into());
                        ray = Ray::new(&p, &wi.normalize());
                        throughput = throughput.mul_element_wise(weight);

                        if i > 3 {
                            let q = throughput.x.max(throughput.y).max(throughput.z);
                            if sampler.next() > q {
                                break;
                            }
                            throughput /= q;
                        }
                        continue;
                    }
                    MediumSample::None { tr } => {
                        throughput = throughput.mul_element_wise(tr);
                    }
                }
            }

            if let Some(its) = its_opt {
                if its.material.have_emission() {
                    let frame = Frame::new(&its.n);
                    l += throughput.mul_element_wise(its.material.emission(
                        &frame.to_local(&-ray.d),
                        &its.uv,
                        &its.p,
                    ));
                    break;
                }

                let frame = Frame::new(&its.n);
                let wo = -ray.d;
                let bsdf_sample_opt =
                    its.material
                        .sample(&frame.to_local(&wo), &its.uv, &its.p, &sampler.next2d());

                if let Some(bsdf_sample) = bsdf_sample_opt {
                    ray = Ray::new(&its.p, &frame.to_world(&bsdf_sample.wi).normalize());
                    throughput = throughput.mul_element_wise(bsdf_sample.weight);
                } else {
                    break;
                }
            } else {
                l += throughput.mul_element_wise(scene.background(ray.d));
                break;
            }

            if i > 3 {
                let q = throughput.x.max(throughput.y).max(throughput.z);
                if sampler.next() > q {
                    break;
                }
                throughput /= q;
            }
        }
        l
    }
}
