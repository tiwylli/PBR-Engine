use std::collections::HashMap;

use cgmath::{Array, ElementWise};
use tinyjson::JsonValue;

use crate::{ray::Ray, samplers::Sampler, scene::Scene, sdf::RaymarchSettings, vec::Color3};

use super::{
    Integrator, SamplerIntegrator, render, sdf::collect_surface_hits, sdf_common::SurfaceContext,
};

#[derive(Default)]
pub struct NormalIntegrator {
    sdf_settings: RaymarchSettings,
}

impl NormalIntegrator {
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

impl Integrator for NormalIntegrator {
    fn render(
        &mut self,
        scene: &Scene,
        sampler: &mut dyn Sampler,
    ) -> crate::array2d::Array2d<Color3> {
        render(self, scene, sampler)
    }
}

impl SamplerIntegrator for NormalIntegrator {
    fn preprocess(&mut self, _: &Scene, _: &mut dyn Sampler) {}

    fn li(&self, ray: &Ray, scene: &Scene, _: &mut dyn Sampler) -> Color3 {
        collect_surface_hits(ray, scene, self.sdf_settings)
            .into_nearest()
            .map_or_else(
                || Color3::from_value(0.0),
                |hit| {
                    let surface = SurfaceContext::from_hit(hit);
                    (surface.normal.add_element_wise(1.0)) * 0.5
                },
            )
    }
}
