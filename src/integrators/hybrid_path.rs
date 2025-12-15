use std::collections::HashMap;

use cgmath::{Array, ElementWise, InnerSpace};
use tinyjson::JsonValue;

use crate::{
    json::json_to_f64,
    ray::Ray,
    sdf::{RaymarchSettings, apply_surface_bias},
    vec::{Color3, Frame, Vec2, Vec3, luminance},
};

use super::{
    Integrator, SamplerIntegrator, render,
    sdf::{SurfaceHit, collect_surface_hits},
};

pub struct HybridPathIntegrator {
    max_depth: usize,
    sdf_settings: RaymarchSettings,
}

impl HybridPathIntegrator {
    #[must_use]
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

impl Integrator for HybridPathIntegrator {
    fn render(
        &mut self,
        scene: &crate::scene::Scene,
        sampler: &mut dyn crate::samplers::Sampler,
    ) -> crate::array2d::Array2d<Color3> {
        render(self, scene, sampler)
    }
}
impl SamplerIntegrator for HybridPathIntegrator {
    fn preprocess(&mut self, _: &crate::scene::Scene, _: &mut dyn crate::samplers::Sampler) {}

    fn li(
        &self,
        ray: &crate::ray::Ray,
        scene: &crate::scene::Scene,
        sampler: &mut dyn crate::samplers::Sampler,
    ) -> crate::vec::Color3 {
        /* Copier votre code scene::trace(...)
        Adapter votre code pour éviter la récursion
        Vous pouvez accumuler la couleur des rebonds (du aux matériaux) dans une couleur. Par exemple:

        // Initalisation [...]
        Color3 throughput(1.0);

        // boucle de rendu rebondissant sur les surfaces
        while(...) { // Ou for loop. Condition sur la profondeur du chemin (depth < m_max_depth)
            // ...
            if(auto res = its.material->sample(...)) {
                throughput *= res->weight;
                // ...
            } else {
                // ...
                return throughput * its.material->emission(...);
            }
        }

        Cette variable vous permet ensuite de calculer la couleur totale.
        Exemple, si vous ne trouvez pas d'intersection:

        return throughput * scene.background(...);

        Ce code doit donner le meme resultat que votre code précédent.
        */
        // votrecodeici!("Devoir 2: Migrer et adapter votre code de scene::trace(...)");
        // return Color3::from_value(0.0);

        const RR_START_DEPTH: usize = 5;

        let mut accumulated_radiance = Color3::new(0.0, 0.0, 0.0);
        let mut throughput = Color3::from_value(1.0);
        let mut depth = 0;
        let mut r = *ray;

        // info!("{}", self.max_depth);

        while depth < self.max_depth {
            let selection = collect_surface_hits(&r, scene, self.sdf_settings);
            match selection.into_nearest() {
                Some(SurfaceHit::Analytic(its)) => {
                    let frame = Frame::new(&its.n);
                    let wo = frame.to_local(&-r.d);
                    let material = its.material;
                    let le = material.emission(&wo, &its.uv, &its.p);
                    accumulated_radiance += throughput.mul_element_wise(le);

                    if let Some(res) = material.sample(&wo, &its.uv, &its.p, &sampler.next2d()) {
                        throughput = throughput.mul_element_wise(res.weight);
                        let wi_world = frame.to_world(&res.wi);
                        r = Ray::new(&its.p, &wi_world);
                        depth += 1;
                        if depth >= RR_START_DEPTH {
                            let mut rr_prob = luminance(&throughput).max(0.0);
                            rr_prob = rr_prob.clamp(0.05, 0.95);
                            if sampler.next() > rr_prob {
                                break;
                            }
                            throughput /= rr_prob;
                        }
                    } else {
                        break;
                    }
                }
                Some(SurfaceHit::Implicit(sdf_hit)) => {
                    let normal = if sdf_hit.raymarch.normal.magnitude2() > 0.0 {
                        sdf_hit.raymarch.normal.normalize()
                    } else {
                        Vec3::new(0.0, 1.0, 0.0)
                    };
                    let frame = Frame::new(&normal);
                    let wo = frame.to_local(&-r.d);
                    let material = sdf_hit.material;
                    let dummy_uv = Vec2::new(0.0, 0.0);
                    let le = material.emission(&wo, &dummy_uv, &sdf_hit.raymarch.position);
                    accumulated_radiance += throughput.mul_element_wise(le);

                    if let Some(res) = material.sample(
                        &wo,
                        &dummy_uv,
                        &sdf_hit.raymarch.position,
                        &sampler.next2d(),
                    ) {
                        throughput = throughput.mul_element_wise(res.weight);
                        let wi_world = frame.to_world(&res.wi);
                        let origin = apply_surface_bias(
                            sdf_hit.raymarch.position,
                            normal,
                            &self.sdf_settings,
                        );
                        r = Ray::new(&origin, &wi_world);
                        depth += 1;
                        if depth >= RR_START_DEPTH {
                            let mut rr_prob = luminance(&throughput).max(0.0);
                            rr_prob = rr_prob.clamp(0.05, 0.95);
                            if sampler.next() > rr_prob {
                                break;
                            }
                            throughput /= rr_prob;
                        }
                    } else {
                        break;
                    }
                }
                None => {
                    accumulated_radiance += throughput.mul_element_wise(scene.background(r.d));
                    break;
                }
            }
        }
        accumulated_radiance
    }

    //     let black = Color3::new(0.0, 0.0, 0.0);
    //     if depth == 0 {
    //         return black;
    //     }
    //     if let Some(intersec) = self.root.hit(&r) {
    //         let f = Frame::new(&intersec.n);
    //         let le = intersec.material.emission(&f.to_local(&-r.d));
    //         if let Some(sd) = intersec.material.sample(&f.to_local(&-r.d), sampler) {
    //             return le
    //                 + sd.weight.mul_element_wise(self.trace(
    //                     Ray::new(&intersec.p, &f.to_world(&sd.wi)),
    //                     sampler,
    //                     depth - 1,
    //                 ));
    //         } else {
    //             return le;
    //         }
    //     } else {
    //         return self.background(r.d);
    //     }
    // }
}
