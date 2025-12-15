use std::collections::HashMap;

use cgmath::{Array, ElementWise, InnerSpace};
use tinyjson::JsonValue;

use crate::{
    json::json_to_f64,
    ray::Ray,
    sdf::RaymarchSettings,
    vec::{Color3, Frame},
};

use super::{
    render,
    sdf::{collect_surface_hits, SurfaceHit},
    sdf_common::{
        direct_emitter_mis_surface, surface_hit_emission, surface_hit_has_emission, SurfaceContext,
    },
    Integrator, SamplerIntegrator,
};

pub struct PathMISIntegrator {
    max_depth: usize,
    sdf_settings: RaymarchSettings,
}

impl PathMISIntegrator {
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let max_depth = json_to_f64(json, "max_depth", 16.0) as usize;
        let sdf_settings = json
            .get("sdf")
            .and_then(|v| v.get::<HashMap<String, JsonValue>>())
            .map(|cfg| RaymarchSettings::default().with_overrides(cfg))
            .unwrap_or_default();

        Self {
            max_depth,
            sdf_settings,
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
        let mut acc_radiance = Color3::new(0.0, 0.0, 0.0);
        let mut throughput = Color3::from_value(1.0);
        let mut depth = 0usize;
        let mut r = ray.clone();
        let mut current_hit = collect_surface_hits(&r, scene, self.sdf_settings).into_nearest();

        let mut skip_next_emission = false;
        while depth < self.max_depth {
            let Some(hit) = current_hit.take() else {
                acc_radiance = acc_radiance + throughput.mul_element_wise(scene.background(r.d));
                break;
            };

            let surface = SurfaceContext::from_hit(hit);
            let frame = Frame::new(&surface.normal);
            let wo = frame.to_local(&-r.d);

            if !skip_next_emission {
                let le = surface.material().emission(&wo);
                acc_radiance = acc_radiance + throughput.mul_element_wise(le);
            }
            skip_next_emission = false;

            let mut direct = direct_emitter_mis_surface(
                &surface,
                scene,
                sampler,
                &frame,
                &wo,
                self.sdf_settings,
            );

            if let Some(sampled) = surface.material().sample(&wo, &sampler.next2d()) {
                let wi_world = frame.to_world(&sampled.wi);
                let origin = surface.spawn_origin(self.sdf_settings);
                let next_ray = Ray::new(&origin, &wi_world);
                let mut next_hit =
                    collect_surface_hits(&next_ray, scene, self.sdf_settings).into_nearest();

                if let Some(ref light_hit) = next_hit {
                    if surface_hit_has_emission(light_hit) {
                        let le = surface_hit_emission(light_hit, &(-next_ray.d));
                        let pdf_bsdf = surface.material().pdf(&wo, &sampled.wi);
                        let pdf_emitter = match light_hit {
                            SurfaceHit::Analytic(its_light) => scene.root.pdf_direct(
                                its_light.shape,
                                &surface.position,
                                &its_light.p,
                                &its_light.n,
                            ),
                            SurfaceHit::Implicit(_) => 0.0,
                        };

                        let mis_w = if !surface.material().have_delta() {
                            let denom = pdf_bsdf + pdf_emitter;
                            if denom > 0.0 {
                                pdf_bsdf / denom
                            } else {
                                0.0
                            }
                        } else {
                            1.0
                        };

                        direct = direct + sampled.weight.mul_element_wise(le) * mis_w;
                        skip_next_emission = true;
                    }
                }

                acc_radiance = acc_radiance + throughput.mul_element_wise(direct);

                throughput = throughput.mul_element_wise(sampled.weight);
                r = next_ray;
                current_hit = next_hit;
                depth += 1;
            } else {
                acc_radiance = acc_radiance + throughput.mul_element_wise(direct);
                break;
            }
        }

        acc_radiance
    }
}
