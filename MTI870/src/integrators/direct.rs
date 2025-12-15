use super::{render, Integrator, SamplerIntegrator};
use crate::{
    json::json_to_string,
    ray::Ray,
    samplers::Sampler,
    scene::Scene,
    shapes::Shape,
    vec::{pdf_hemisphere, sample_hemisphere, Color3, Frame},
};
use cgmath::{Array, ElementWise, InnerSpace};
use log::info;
use std::collections::HashMap;
use tinyjson::JsonValue;

// Helper: compute the emitter-sampling contribution with MIS (returns Color3)
pub fn direct_emitter_mis(
    its: &crate::shapes::Intersection,
    scene: &Scene,
    sampler: &mut dyn Sampler,
    f: &Frame,
    wo: &cgmath::Vector3<f64>,
) -> Color3 {
    if !scene.has_analytic_emitters() {
        return Color3::from_value(0.0);
    }

    let mut contrib = Color3::from_value(0.0);
    if !its.material.have_delta() {
        let (ps, shape) = scene.root.sample_direct(&its.p, &sampler.next2d());
        if ps.pdf > 0.0 && scene.visible(&its.p, &ps.y) {
            let dir = (ps.y - its.p).normalize();
            let wi_local = f.to_local(&dir);

            // Evaluate BSDF (returns f*cos)
            let fbsdf_cos = its.material.evaluate(wo, &wi_local);

            // Emitted radiance from the emitter towards the shading point
            let f_light = Frame::new(&ps.n);
            let wo_light = f_light.to_local(&(-dir));
            let le = shape.material().emission(&wo_light);

            let pdf_emitter = ps.pdf as f64;
            let pdf_bsdf = its.material.pdf(wo, &wi_local);

            let denom = pdf_bsdf + pdf_emitter;
            if denom > 0.0 {
                let mis_w = pdf_emitter / denom;
                contrib = contrib + (fbsdf_cos / pdf_emitter).mul_element_wise(le) * mis_w;
            }
        }
    }
    contrib
}

enum DirectType {
    EBSDF,    // Utilisation bsdf.sample(...) -- Tâche 1
    ENaive,   // Tâche 4
    EEmitter, // Devoir 3 (prochain devoir)
    EMIS,     // Devoir 3 (prochain devoir)
}

pub struct DirectIntegrator {
    direct_type: DirectType,
}

impl DirectIntegrator {
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let strategy = json_to_string(json, "strategy", "bsdf");
        let direct_type = match strategy.as_str() {
            "bsdf" => DirectType::EBSDF,
            "naive" => DirectType::ENaive,
            "emitter" => DirectType::EEmitter,
            "mis" => DirectType::EMIS,
            _ => panic!("Unknown strategy {}", strategy),
        };
        Self { direct_type }
    }
}

impl Integrator for DirectIntegrator {
    fn render(
        &mut self,
        scene: &crate::scene::Scene,
        sampler: &mut dyn crate::samplers::Sampler,
    ) -> crate::array2d::Array2d<Color3> {
        render(self, scene, sampler)
    }
}

impl SamplerIntegrator for DirectIntegrator {
    fn preprocess(&mut self, _: &Scene, _: &mut dyn Sampler) {}

    fn li(&self, ray: &Ray, scene: &Scene, sampler: &mut dyn Sampler) -> Color3 {
        // votre code ici :
        // - intersection dans la scène pour trouver le premier point d'intersection
        // - vérifier si l'intersection est valide, sinon couleur de fond
        // - vérifier si ce point d'intersection est sur une source de lumière
        //   si oui, retourner la couleur de la source de lumière
        // - sinon, calculer l'éclairage direct (voir les différentes stratégies)

        let Some(its) = scene.root.hit(ray) else {
            return scene.background(ray.d);
        };

        let f = Frame::new(&its.n);
        let wo = f.to_local(&(-ray.d));

        if its.material.have_emission() {
            return its.material.emission(&wo);
        }

        match self.direct_type {
            DirectType::EBSDF => {
                if let Some(bsdf_sample) = its.material.sample(&wo, &sampler.next2d()) {
                    let wi_world = f.to_world(&bsdf_sample.wi);
                    let shadow_ray = Ray::new(&its.p, &wi_world);

                    if let Some(light_hit) = scene.root.hit(&shadow_ray) {
                        if light_hit.material.have_emission() {
                            let f_light = Frame::new(&light_hit.n);
                            let wo_light = f_light.to_local(&(-shadow_ray.d));
                            let le = light_hit.material.emission(&wo_light);
                            bsdf_sample.weight.mul_element_wise(le)
                        } else {
                            Color3::from_value(0.0)
                        }
                    } else {
                        bsdf_sample
                            .weight
                            .mul_element_wise(scene.background(-wi_world))
                    }
                    // If we didn't hit an emitter, there is no direct contribution along this sample.
                    // Color3::from_value(0.0)
                } else {
                    Color3::from_value(0.0)
                }
            }
            DirectType::ENaive => {
                // If the BSDF is specular (delta), behave like Task 1
                if its.material.have_delta() {
                    if let Some(bsdf_sample) = its.material.sample(&wo, &sampler.next2d()) {
                        let wi_world = f.to_world(&bsdf_sample.wi);
                        let shadow_ray = Ray::new(&its.p, &wi_world);

                        if let Some(light_hit) = scene.root.hit(&shadow_ray) {
                            if light_hit.material.have_emission() {
                                let f_light = Frame::new(&light_hit.n);
                                let wo_light = f_light.to_local(&(-shadow_ray.d));
                                let le = light_hit.material.emission(&wo_light);
                                return bsdf_sample.weight.mul_element_wise(le);
                            } else {
                                return Color3::from_value(0.0);
                            }
                        } else {
                            return bsdf_sample
                                .weight
                                .mul_element_wise(scene.background(-wi_world));
                        }
                    }
                    return Color3::from_value(0.0);
                } else {
                    // Otherwise: sample a UNIFORM hemisphere direction (naive)
                    let xi = sampler.next2d();
                    let wi_local = sample_hemisphere(&xi);
                    let pdf = pdf_hemisphere(&wi_local);
                    if pdf <= 0.0 {
                        return Color3::from_value(0.0);
                    }

                    // Shadow ray to check visibility of the sampled direction
                    let wi_world = f.to_world(&wi_local);
                    let shadow_ray = Ray::new(&its.p, &wi_world);

                    if let Some(light_hit) = scene.root.hit(&shadow_ray) {
                        if light_hit.material.have_emission() {
                            let bsdf_cos = its.material.evaluate(&wo, &wi_local);

                            let f_light = Frame::new(&light_hit.n);
                            let wo_light = f_light.to_local(&(-shadow_ray.d));
                            let le = light_hit.material.emission(&wo_light);

                            return (bsdf_cos / pdf).mul_element_wise(le);
                        }
                    } else {
                        let bsdf_cos = its.material.evaluate(&wo, &wi_local);
                        let le = scene.background(-wi_world);
                        return (bsdf_cos / pdf).mul_element_wise(le);
                    }

                    Color3::from_value(0.0)
                }
            }

            DirectType::EEmitter => {
                if !scene.has_analytic_emitters() {
                    return Color3::from_value(0.0);
                }

                // If the BSDF is specular (delta), behave like Task 1 (BSDF sampling)
                if its.material.have_delta() {
                    if let Some(bsdf_sample) = its.material.sample(&wo, &sampler.next2d()) {
                        let wi_world = f.to_world(&bsdf_sample.wi);
                        let shadow_ray = Ray::new(&its.p, &wi_world);

                        if let Some(light_hit) = scene.root.hit(&shadow_ray) {
                            if light_hit.material.have_emission() {
                                let f_light = Frame::new(&light_hit.n);
                                let wo_light = f_light.to_local(&(-shadow_ray.d));
                                let le = light_hit.material.emission(&wo_light);
                                return bsdf_sample.weight.mul_element_wise(le);
                            }
                        }
                    }
                    return Color3::from_value(0.0);
                }

                // Otherwise: explicit emitter sampling
                let (ps, shape) = scene.root.sample_direct(&its.p, &sampler.next2d());
                // ps.pdf is the PDF in solid angle (including selection of emitter)
                if ps.pdf <= 0.0 {
                    return Color3::from_value(0.0);
                }

                // Check visibility between shading point and sampled emitter point
                if !scene.visible(&its.p, &ps.y) {
                    return Color3::from_value(0.0);
                }

                // Evaluate BSDF (evaluate returns f * cos(theta))
                let dir = (ps.y - its.p).normalize();
                let wi_local = f.to_local(&dir);
                let fbsdf_cos = its.material.evaluate(&wo, &wi_local);

                // Evaluate emitted radiance at emitter towards the shading point
                let f_light = Frame::new(&ps.n);
                let wo_light = f_light.to_local(&(-dir));
                let le = shape.material().emission(&wo_light);

                // Contribution = fbsdf_cos * Le / pdf_emitter
                (fbsdf_cos / (ps.pdf as f64)).mul_element_wise(le)
            }
            DirectType::EMIS => {
                // Compute emitter-sampling contribution (factored helper)
                let mut contrib = direct_emitter_mis(&its, scene, sampler, &f, &wo);

                // BSDF sampling
                if let Some(bsdf_sample) = its.material.sample(&wo, &sampler.next2d()) {
                    let wi_world = f.to_world(&bsdf_sample.wi);
                    let shadow_ray = Ray::new(&its.p, &wi_world);

                    if let Some(light_hit) = scene.root.hit(&shadow_ray) {
                        if light_hit.material.have_emission() {
                            // Emitted radiance at the hit point (on emitter)
                            let f_light = Frame::new(&light_hit.n);
                            let wo_light = f_light.to_local(&(-shadow_ray.d));
                            let le = light_hit.material.emission(&wo_light);

                            // MIS weight
                            let pdf_bsdf = its.material.pdf(&wo, &bsdf_sample.wi);
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

                            contrib = contrib + bsdf_sample.weight.mul_element_wise(le) * mis_w;
                        }
                    }
                }

                contrib
            }
        }
    }
}
