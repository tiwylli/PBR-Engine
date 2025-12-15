use std::collections::HashMap;

use cgmath::Zero;
use tinyjson::JsonValue;

use crate::{
    array2d::Array2d,
    ray::Ray,
    samplers::Sampler,
    scene::Scene,
    sdf::RaymarchSettings,
    vec::{Color3, Vec3},
};

use super::{
    Integrator, SamplerIntegrator, render, sdf::collect_surface_hits, sdf_common::SurfaceContext,
};

#[derive(Default)]
pub struct AlbedoIntegrator {
    sdf_settings: RaymarchSettings,
}

impl AlbedoIntegrator {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let sdf_settings = json
            .get("sdf")
            .and_then(|v| v.get::<HashMap<String, JsonValue>>())
            .map(|cfg| RaymarchSettings::default().with_overrides(cfg))
            .unwrap_or_default();

        Self { sdf_settings }
    }
}

impl Integrator for AlbedoIntegrator {
    fn render(&mut self, scene: &Scene, sampler: &mut dyn Sampler) -> Array2d<Vec3> {
        render(self, scene, sampler)
    }
}

impl SamplerIntegrator for AlbedoIntegrator {
    fn preprocess(&mut self, _: &Scene, _: &mut dyn Sampler) {}

    fn li(&self, ray: &Ray, scene: &Scene, _: &mut dyn Sampler) -> Color3 {
        collect_surface_hits(ray, scene, self.sdf_settings)
            .into_nearest()
            .map_or_else(Color3::zero, |hit| {
                let surface = SurfaceContext::from_hit(hit);
                surface
                    .material()
                    .get_albedo(&surface.uv, &surface.position)
            })
    }
}
