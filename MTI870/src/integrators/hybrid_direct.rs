use std::collections::HashMap;

use cgmath::{Array, ElementWise, InnerSpace};
use tinyjson::JsonValue;

use crate::{
    json::json_to_string,
    ray::Ray,
    samplers::Sampler,
    scene::Scene,
    sdf::RaymarchSettings,
    shapes::Shape,
    vec::{luminance, pdf_hemisphere, sample_hemisphere, Color3, Frame, Vec3},
};

use super::{
    render,
    sdf::collect_surface_hits,
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

/// Direct-lighting integrator that works with both analytic shapes and SDF objects.
pub struct HybridDirectIntegrator {
    direct_type: DirectType,
    sdf_settings: RaymarchSettings,
}

const RR_MIN_PROB: f64 = 0.05;
const RR_MAX_PROB: f64 = 1.0;

impl HybridDirectIntegrator {
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

    fn russian_roulette(&self, weight: &mut Color3, sampler: &mut dyn Sampler) -> bool {
        let lumin = luminance(weight).max(0.0);
        if lumin == 0.0 {
            return false;
        }
        let prob = lumin.clamp(RR_MIN_PROB, RR_MAX_PROB);
        if sampler.next() > prob {
            false
        } else {
            *weight = *weight / prob;
            true
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
            let mut weight = bsdf_sample.weight;
            if !self.russian_roulette(&mut weight, sampler) {
                return Color3::from_value(0.0);
            }
            let wi_world = frame.to_world(&bsdf_sample.wi);
            let origin = surface.spawn_origin(self.sdf_settings);
            let shadow_ray = Ray::new(&origin, &wi_world);

            let hit = collect_surface_hits(&shadow_ray, scene, self.sdf_settings).into_nearest();
            if let Some(light_hit) = hit {
                if surface_hit_has_emission(&light_hit) {
                    let le = surface_hit_emission(&light_hit, &(-shadow_ray.d));
                    return weight.mul_element_wise(le);
                }
            } else {
                return weight.mul_element_wise(scene.background(-wi_world));
            }
        }

        Color3::from_value(0.0)
    }

    fn hemisphere_naive(
        &self,
        surface: &SurfaceContext,
        scene: &Scene,
        sampler: &mut dyn Sampler,
        frame: &Frame,
        wo: &Vec3,
    ) -> Color3 {
        if surface.material().have_delta() {
            return self.sample_bsdf(surface, scene, sampler, frame, wo);
        }

        let xi = sampler.next2d();
        let wi_local = sample_hemisphere(&xi);
        let pdf = pdf_hemisphere(&wi_local);
        if pdf <= 0.0 {
            return Color3::from_value(0.0);
        }

        let bsdf_cos = surface.material().evaluate(wo, &wi_local);
        let mut weight = bsdf_cos / pdf;
        if !self.russian_roulette(&mut weight, sampler) {
            return Color3::from_value(0.0);
        }

        let wi_world = frame.to_world(&wi_local);
        let origin = surface.spawn_origin(self.sdf_settings);
        let shadow_ray = Ray::new(&origin, &wi_world);
        let hit = collect_surface_hits(&shadow_ray, scene, self.sdf_settings).into_nearest();

        if let Some(light_hit) = hit {
            if surface_hit_has_emission(&light_hit) {
                let le = surface_hit_emission(&light_hit, &(-wi_world));
                return weight.mul_element_wise(le);
            }
        } else {
            let le = scene.background(-wi_world);
            return weight.mul_element_wise(le);
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
        if dist <= crate::constants::RAY_EPS * 2.0 {
            return Color3::from_value(0.0);
        }

        let dir_world = to_light / dist;
        let max_t = dist - crate::constants::RAY_EPS * 2.0;
        let shadow_ray = Ray::new(&origin, &dir_world).with_distance_max(max_t);
        if collect_surface_hits(&shadow_ray, scene, self.sdf_settings)
            .into_nearest()
            .is_some()
        {
            return Color3::from_value(0.0);
        }

        let wi_local = frame.to_local(&dir_world);
        let fbsdf_cos = surface.material().evaluate(wo, &wi_local);
        let mut weight = fbsdf_cos / (ps.pdf as f64);
        if !self.russian_roulette(&mut weight, sampler) {
            return Color3::from_value(0.0);
        }

        let f_light = Frame::new(&ps.n);
        let wo_light = f_light.to_local(&(-dir_world));
        let le = shape.material().emission(&wo_light);

        weight.mul_element_wise(le)
    }
}

impl Integrator for HybridDirectIntegrator {
    fn render(
        &mut self,
        scene: &Scene,
        sampler: &mut dyn Sampler,
    ) -> crate::array2d::Array2d<Color3> {
        render(self, scene, sampler)
    }
}

impl SamplerIntegrator for HybridDirectIntegrator {
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
            DirectType::ENaive => self.hemisphere_naive(&surface, scene, sampler, &frame, &wo),
            DirectType::EEmitter => {
                if surface.material().have_delta() {
                    self.sample_bsdf(&surface, scene, sampler, &frame, &wo)
                } else {
                    self.explicit_emitter(&surface, scene, sampler, &frame, &wo)
                }
            }
            DirectType::EMIS => {
                let bsdf_term = self.sample_bsdf(&surface, scene, sampler, &frame, &wo);
                let emitter_term = direct_emitter_mis_surface(
                    &surface,
                    scene,
                    sampler,
                    &frame,
                    &wo,
                    self.sdf_settings,
                );
                bsdf_term + emitter_term
            }
        }
    }
}
