use cgmath::Zero;

use crate::{modulo, ray::Ray, samplers::Sampler, scene::Scene, vec::Color3};

use super::{Integrator, SamplerIntegrator, render};

pub struct UvIntegrator;

impl Integrator for UvIntegrator {
    fn render(
        &mut self,
        scene: &Scene,
        sampler: &mut dyn Sampler,
    ) -> crate::array2d::Array2d<Color3> {
        render(self, scene, sampler)
    }
}

impl SamplerIntegrator for UvIntegrator {
    fn preprocess(&mut self, _: &Scene, _: &mut dyn Sampler) {}

    fn li(&self, ray: &Ray, scene: &Scene, _: &mut dyn Sampler) -> Color3 {
        scene.hit(ray).map_or_else(Color3::zero, |intersection| {
            let u = modulo(intersection.uv.x, 1.0);
            let v = modulo(intersection.uv.y, 1.0);
            Color3::new(u, v, 0.0)
        })
    }
}
