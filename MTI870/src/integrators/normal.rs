use cgmath::Array;

use crate::{
    ray::Ray,
    samplers::Sampler,
    scene::Scene,
    vec::{Color3, Vec3},
};

use super::{render, Integrator, SamplerIntegrator};

pub struct NormalIntegrator;
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
        /* Couleur de la normale si une intersection :
        (n + 1.0) * 0.5

        Sinon retourner la couleur noire.
        */
        if let Some(hit) = scene.hit(ray) {
            let n = hit.n;
            Color3::new((n.x + 1.0) * 0.5, (n.y + 1.0) * 0.5, (n.z + 1.0) * 0.5)
        } else {
            Color3::from_value(0.0)
        }
    }
}
