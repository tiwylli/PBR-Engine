use std::collections::HashMap;

use cgmath::{Array, ElementWise, InnerSpace};
use tinyjson::JsonValue;

use crate::{
    constants::RAY_EPS,
    json::json_to_string,
    ray::Ray,
    samplers::Sampler,
    scene::Scene,
    sdf::RaymarchSettings,
    shapes::Shape,
    vec::{pdf_hemisphere, sample_hemisphere, Color3, Frame, Vec3},
};

use super::{
    render,
    sdf::{collect_surface_hits, SurfaceHit},
    sdf_common::{
        direct_emitter_mis_surface, surface_hit_emission, surface_hit_has_emission, SurfaceContext,
    },
    Integrator, SamplerIntegrator,
};

enum DirectType {
    EBSDF,
    ENaive,
    EEmitter,
    EMIS,
}

pub struct SDFDirectIntegrator {
    direct_type: DirectType,
    sdf_settings: RaymarchSettings,
}

impl SDFDirectIntegrator {
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let strategy = json_to_string(json, "strategy", "bsdf");
        let direct_type = match strategy.as_str() {
            "bsdf" => DirectType::EBSDF,
            "naive" => DirectType::ENaive,
            "emitter" => DirectType::EEmitter,
            "mis" => DirectType::EMIS,
            _ => panic!("Unknown strategy {}", strategy),
        };

        let sdf_settings = json
            .get("sdf")
            .and_then(|v| v.get::<HashMap<String, JsonValue>>())
            .map(|cfg| RaymarchSettings::default().with_overrides(cfg))
            .unwrap_or_default();

        Self {
            direct_type,
            sdf_settings,
        }
    }

    fn sample_bsdf(
        &self,
        surface: &SurfaceContext,
        scene: &Scene,
        sampler: &mut dyn Sampler,
        frame: &Frame,
        wo: &Vec3,
    ) -> Color3 {
        if let Some(bsdf_sample) = surface.material().sample(wo, &sampler.next2d()) {
            let wi_world = frame.to_world(&bsdf_sample.wi);
            let origin = surface.spawn_origin(self.sdf_settings);
            let shadow_ray = Ray::new(&origin, &wi_world);

            let hit = collect_surface_hits(&shadow_ray, scene, self.sdf_settings).into_nearest();
            if let Some(light_hit) = hit {
                if surface_hit_has_emission(&light_hit) {
                    let le = surface_hit_emission(&light_hit, &(-shadow_ray.d));
                    return bsdf_sample.weight.mul_element_wise(le);
                }
            } else {
                return bsdf_sample
                    .weight
                    .mul_element_wise(scene.background(-wi_world));
            }
        }
        Color3::from_value(0.0)
    }

    fn explicit_emitter(
        &self,
        surface: &SurfaceContext,
        scene: &Scene,
        sampler: &mut dyn Sampler,
        frame: &Frame,
        wo: &Vec3,
    ) -> Color3 {
        if !scene.has_analytic_emitters() {
            return Color3::from_value(0.0);
        }

        let (ps, shape) = scene
            .root
            .sample_direct(&surface.position, &sampler.next2d());
        if ps.pdf <= 0.0 {
            return Color3::from_value(0.0);
        }

        let origin = surface.spawn_origin(self.sdf_settings);
        let to_light = ps.y - origin;
        let dist = to_light.magnitude();
        if dist <= RAY_EPS * 2.0 {
            return Color3::from_value(0.0);
        }

        let dir_world = to_light / dist;
        let max_t = dist - RAY_EPS * 2.0;
        let shadow_ray = Ray::new(&origin, &dir_world).with_distance_max(max_t);
        let occluder = collect_surface_hits(&shadow_ray, scene, self.sdf_settings).into_nearest();
        if occluder.is_some() {
            return Color3::from_value(0.0);
        }

        let wi_local = frame.to_local(&dir_world);
        let fbsdf_cos = surface.material().evaluate(wo, &wi_local);

        let f_light = Frame::new(&ps.n);
        let wo_light = f_light.to_local(&(-dir_world));
        let le = shape.material().emission(&wo_light);

        (fbsdf_cos / (ps.pdf as f64)).mul_element_wise(le)
    }
}

impl Integrator for SDFDirectIntegrator {
    fn render(
        &mut self,
        scene: &Scene,
        sampler: &mut dyn Sampler,
    ) -> crate::array2d::Array2d<Color3> {
        render(self, scene, sampler)
    }
}

impl SamplerIntegrator for SDFDirectIntegrator {
    fn preprocess(&mut self, _: &Scene, _: &mut dyn Sampler) {}

    fn li(&self, ray: &Ray, scene: &Scene, sampler: &mut dyn Sampler) -> Color3 {
        let selection = collect_surface_hits(ray, scene, self.sdf_settings);
        let Some(hit) = selection.into_nearest() else {
            return scene.background(ray.d);
        };

        let surface = SurfaceContext::from_hit(hit);
        let frame = Frame::new(&surface.normal);
        let wo = frame.to_local(&(-ray.d));

        if surface.material().have_emission() {
            return surface.material().emission(&wo);
        }

        match self.direct_type {
            DirectType::EBSDF => self.sample_bsdf(&surface, scene, sampler, &frame, &wo),
            DirectType::ENaive => {
                if surface.material().have_delta() {
                    return self.sample_bsdf(&surface, scene, sampler, &frame, &wo);
                }

                let xi = sampler.next2d();
                let wi_local = sample_hemisphere(&xi);
                let pdf = pdf_hemisphere(&wi_local);
                if pdf <= 0.0 {
                    return Color3::from_value(0.0);
                }

                let wi_world = frame.to_world(&wi_local);
                let origin = surface.spawn_origin(self.sdf_settings);
                let shadow_ray = Ray::new(&origin, &wi_world);
                let hit =
                    collect_surface_hits(&shadow_ray, scene, self.sdf_settings).into_nearest();

                let bsdf_cos = surface.material().evaluate(&wo, &wi_local);
                if let Some(light_hit) = hit {
                    if surface_hit_has_emission(&light_hit) {
                        let le = surface_hit_emission(&light_hit, &(-wi_world));
                        return (bsdf_cos / pdf).mul_element_wise(le);
                    }
                } else {
                    let le = scene.background(-wi_world);
                    return (bsdf_cos / pdf).mul_element_wise(le);
                }

                Color3::from_value(0.0)
            }
            DirectType::EEmitter => {
                if !scene.has_analytic_emitters() && !surface.material().have_delta() {
                    return Color3::from_value(0.0);
                }

                if surface.material().have_delta() {
                    self.sample_bsdf(&surface, scene, sampler, &frame, &wo)
                } else {
                    self.explicit_emitter(&surface, scene, sampler, &frame, &wo)
                }
            }
            DirectType::EMIS => {
                let mut contrib = Color3::from_value(0.0);
                if !surface.material().have_delta() {
                    contrib = contrib
                        + direct_emitter_mis_surface(
                            &surface,
                            scene,
                            sampler,
                            &frame,
                            &wo,
                            self.sdf_settings,
                        );
                }

                if let Some(bsdf_sample) = surface.material().sample(&wo, &sampler.next2d()) {
                    let wi_world = frame.to_world(&bsdf_sample.wi);
                    let origin = surface.spawn_origin(self.sdf_settings);
                    let next_ray = Ray::new(&origin, &wi_world);
                    let hit =
                        collect_surface_hits(&next_ray, scene, self.sdf_settings).into_nearest();

                    if let Some(light_hit) = hit {
                        if surface_hit_has_emission(&light_hit) {
                            let le = surface_hit_emission(&light_hit, &(-next_ray.d));
                            if surface.material().have_delta() {
                                return bsdf_sample.weight.mul_element_wise(le);
                            }

                            let pdf_bsdf = surface.material().pdf(&wo, &bsdf_sample.wi);
                            let pdf_emitter = match light_hit {
                                SurfaceHit::Analytic(its_light) => scene.root.pdf_direct(
                                    its_light.shape,
                                    &surface.position,
                                    &its_light.p,
                                    &its_light.n,
                                ),
                                SurfaceHit::Implicit(_) => 0.0,
                            };

                            let mut mis_w = 1.0;
                            let denom = pdf_bsdf + pdf_emitter;
                            if denom > 0.0 {
                                mis_w = pdf_bsdf / denom;
                            }

                            contrib = contrib + bsdf_sample.weight.mul_element_wise(le) * mis_w;
                            return contrib;
                        }
                    } else {
                        contrib = contrib
                            + bsdf_sample
                                .weight
                                .mul_element_wise(scene.background(-wi_world));
                        return contrib;
                    }

                    contrib
                } else {
                    contrib
                }
            }
        }
    }
}
