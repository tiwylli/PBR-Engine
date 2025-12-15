use std::collections::HashMap;

use cgmath::{Array, ElementWise, InnerSpace};
use log::info;
use tinyjson::JsonValue;

use crate::{
    json::json_to_f64,
    ray::Ray,
    shapes::Shape,
    vec::{Color3, Frame},
};

use super::{render, Integrator, SamplerIntegrator};

pub struct PathMISIntegrator {
    max_depth: usize,
}

impl PathMISIntegrator {
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        Self {
            max_depth: json_to_f64(json, "max_depth", 16.0) as usize,
        }
    }
}

impl Integrator for PathMISIntegrator {
    fn render(
        &mut self,
        scene: &crate::scene::Scene,
        sampler: &mut dyn crate::samplers::Sampler,
    ) -> crate::array2d::Array2d<Color3> {
        render(self, scene, sampler)
    }
}

impl SamplerIntegrator for PathMISIntegrator {
    fn preprocess(&mut self, _: &crate::scene::Scene, _: &mut dyn crate::samplers::Sampler) {}

    fn li(
        &self,
        ray: &crate::ray::Ray,
        scene: &crate::scene::Scene,
        sampler: &mut dyn crate::samplers::Sampler,
    ) -> crate::vec::Color3 {
        // Path tracer with MIS direct lighting (next-event estimation)
        let mut acc_radiance = Color3::new(0.0, 0.0, 0.0);
        let mut throughput = Color3::from_value(1.0);
        let mut depth = 0usize;
        let mut r = ray.clone();
        //ChadGPT Helped me do the bonus task of reusing the same BSDF sample for both direct and indirect lighting
        // It also helped me add comments to explain the code better and not lose myself in my reasoning
        let mut current_hit = scene.root.hit(&r);

        let mut skip_next_emission = false;
        while depth < self.max_depth {
            let Some(its) = current_hit.take() else {
                acc_radiance = acc_radiance + throughput.mul_element_wise(scene.background(r.d));
                break;
            };

            let f = Frame::new(&its.n);
            let wo = f.to_local(&-r.d);

            // Add emitted radiance at the hit (if any)
            if !skip_next_emission {
                let le = its.material.emission(&wo);
                acc_radiance = acc_radiance + throughput.mul_element_wise(le);
            }
            // reset skip flag (only skip emission for the immediate next hit)
            skip_next_emission = false;

            // Emitter sampling contribution
            let mut direct =
                crate::integrators::direct::direct_emitter_mis(&its, scene, sampler, &f, &wo);

            // Sample BSDF ONCE and reuse it for the BSDF direct-contribution and for
            // the indirect continuation.
            if let Some(sampled) = its.material.sample(&wo, &sampler.next2d()) {
                // Evaluate BSDF-sampling contribution (check if this direction hits an emitter)
                let wi_world = f.to_world(&sampled.wi);
                let next_ray = Ray::new(&its.p, &wi_world);
                let next_hit = scene.root.hit(&next_ray);

                if let Some(light_hit) = next_hit.as_ref() {
                    if light_hit.material.have_emission() {
                        let f_light = Frame::new(&light_hit.n);
                        let wo_light = f_light.to_local(&(-next_ray.d));
                        let le = light_hit.material.emission(&wo_light);

                        let pdf_bsdf = its.material.pdf(&wo, &sampled.wi);
                        let mut mis_w = 1.0;
                        if !its.material.have_delta() {
                            let pdf_emitter = scene.root.pdf_direct(
                                light_hit.shape,
                                &its.p,
                                &light_hit.p,
                                &light_hit.n,
                            );
                            let denom = pdf_bsdf + pdf_emitter;
                            if denom > 0.0 {
                                mis_w = pdf_bsdf / denom;
                            } else {
                                mis_w = 0.0;
                            }
                        }

                        direct = direct + sampled.weight.mul_element_wise(le) * mis_w;

                        // If the sampled direction hits an emitter, we will also march the path
                        // to that emitter in the next iteration. To avoid double-counting the
                        // emitter's radiance (already added it via the shadow-ray direct term),
                        // skip adding emission on the immediately following intersection.
                        skip_next_emission = true;
                    }
                }

                // accumulate direct contribution (explicit emitter + bsdf-based)
                acc_radiance = acc_radiance + throughput.mul_element_wise(direct);

                // Continue the path along the SAME sampled direction
                throughput = throughput.mul_element_wise(sampled.weight);
                r = next_ray;
                current_hit = next_hit;
                depth += 1;
                continue;
            } else {
                // No further scattering: add direct (from emitter sampling) then terminate
                acc_radiance = acc_radiance + throughput.mul_element_wise(direct);
                break;
            }
        }

        acc_radiance
    }
}
