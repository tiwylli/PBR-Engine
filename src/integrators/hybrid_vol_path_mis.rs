use std::collections::HashMap;

use cgmath::{Array, ElementWise, InnerSpace, Zero};
use tinyjson::JsonValue;

use crate::{
    constants::RAY_EPS,
    json::json_to_f64,
    medium::{Medium, MediumSample},
    ray::Ray,
    sdf::RaymarchSettings,
    vec::{Color3, Frame, Point3, Vec2, Vec3},
};

use super::{
    Integrator, SamplerIntegrator, render,
    sdf::{SurfaceHit, collect_surface_hits},
    sdf_common::SurfaceContext,
};

pub struct HybridVolPathMisIntegrator {
    max_depth: usize,
    sdf_settings: RaymarchSettings,
}

impl HybridVolPathMisIntegrator {
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

impl Integrator for HybridVolPathMisIntegrator {
    fn render(
        &mut self,
        scene: &crate::scene::Scene,
        sampler: &mut dyn crate::samplers::Sampler,
    ) -> crate::array2d::Array2d<Color3> {
        render(self, scene, sampler)
    }
}

impl SamplerIntegrator for HybridVolPathMisIntegrator {
    fn preprocess(&mut self, _: &crate::scene::Scene, _: &mut dyn crate::samplers::Sampler) {}

    fn li(
        &self,
        ray: &crate::ray::Ray,
        scene: &crate::scene::Scene,
        sampler: &mut dyn crate::samplers::Sampler,
    ) -> crate::vec::Color3 {
        let mut acc_radiance = Color3::zero();
        let mut throughput = Color3::from_value(1.0);
        let mut depth = 0usize;
        let mut r = *ray;

        let mut prev_pdf: Option<f64> = None;
        let mut prev_delta = false;
        let mut prev_position = ray.o;

        while depth < self.max_depth {
            let selection = collect_surface_hits(&r, scene, self.sdf_settings);
            let hit_opt = selection.into_nearest();
            let t_surface = hit_opt
                .as_ref()
                .map_or(f64::INFINITY, surface_distance)
                .min(r.tmax);

            let mut handled_medium = false;
            if let Some(medium) = scene.medium.as_deref() {
                match medium.sample(t_surface, sampler) {
                    MediumSample::Scatter { t, weight, .. } => {
                        let p = r.point_at(t);
                        let wo = -r.d;
                        throughput = throughput.mul_element_wise(weight);

                        let direct =
                            direct_medium_nee(scene, sampler, &p, &wo, medium, self.sdf_settings);
                        acc_radiance += throughput.mul_element_wise(direct);

                        let wi = medium
                            .phase_function()
                            .sample_p(&wo, sampler.next2d().into())
                            .normalize();
                        let pdf_dir = medium.phase_function().phase_func(&wo, &wi);

                        prev_pdf = Some(pdf_dir);
                        prev_delta = false;
                        prev_position = p;

                        r = Ray::new(&p, &wi);
                        depth += 1;
                        handled_medium = true;

                        if russian_roulette(&mut throughput, sampler, depth) {
                            break;
                        }
                    }
                    MediumSample::None { tr } => {
                        throughput = throughput.mul_element_wise(tr);
                    }
                }
            }

            if handled_medium {
                continue;
            }

            let Some(hit) = hit_opt else {
                acc_radiance += throughput.mul_element_wise(scene.background(r.d));
                break;
            };

            let emission_weight =
                emission_mis_weight(prev_pdf, prev_delta, &hit, scene, &prev_position);
            let surface = SurfaceContext::from_hit(hit);
            let frame = Frame::new(&surface.normal);
            let wo = frame.to_local(&-r.d);

            if surface.material().have_emission() {
                let le = surface
                    .material()
                    .emission(&wo, &surface.uv, &surface.position);
                acc_radiance += throughput.mul_element_wise(le) * emission_weight;
                break;
            }

            let direct = direct_emitter_mis_surface_medium(
                &surface,
                scene,
                sampler,
                &frame,
                &wo,
                self.sdf_settings,
                scene.medium.as_deref(),
            );
            acc_radiance += throughput.mul_element_wise(direct);

            if let Some(sampled) =
                surface
                    .material()
                    .sample(&wo, &surface.uv, &surface.position, &sampler.next2d())
            {
                let wi_world = frame.to_world(&sampled.wi).normalize();
                let origin = surface.spawn_origin(self.sdf_settings);

                prev_pdf = Some(surface.material().pdf(
                    &wo,
                    &sampled.wi,
                    &surface.uv,
                    &surface.position,
                ));
                prev_delta = surface.material().have_delta();
                prev_position = origin;

                throughput = throughput.mul_element_wise(sampled.weight);
                r = Ray::new(&origin, &wi_world);
                depth += 1;

                if russian_roulette(&mut throughput, sampler, depth) {
                    break;
                }
            } else {
                break;
            }
        }

        acc_radiance
    }
}

const fn surface_distance(hit: &SurfaceHit<'_>) -> f64 {
    match hit {
        SurfaceHit::Analytic(its) => its.t,
        SurfaceHit::Implicit(sdf_hit) => sdf_hit.raymarch.t,
    }
}

fn emission_mis_weight(
    prev_pdf: Option<f64>,
    prev_delta: bool,
    hit: &SurfaceHit<'_>,
    scene: &crate::scene::Scene,
    prev_position: &Point3,
) -> f64 {
    if prev_delta {
        return 1.0;
    }

    let Some(pdf_bsdf) = prev_pdf else {
        return 1.0;
    };

    let pdf_emitter = match hit {
        SurfaceHit::Analytic(its) => {
            scene
                .root
                .pdf_direct(its.shape, prev_position, &its.p, &its.n)
        }
        SurfaceHit::Implicit(_) => 0.0,
    };

    let denom = pdf_bsdf + pdf_emitter;
    if denom > 0.0 { pdf_bsdf / denom } else { 1.0 }
}

fn direct_emitter_mis_surface_medium(
    surface: &SurfaceContext<'_>,
    scene: &crate::scene::Scene,
    sampler: &mut dyn crate::samplers::Sampler,
    frame: &Frame,
    wo: &Vec3,
    settings: RaymarchSettings,
    medium: Option<&dyn Medium>,
) -> Color3 {
    if !scene.has_analytic_emitters() {
        return Color3::from_value(0.0);
    }

    if surface.material().have_delta() {
        return Color3::from_value(0.0);
    }

    let (ps, shape) = scene
        .root
        .sample_direct(&surface.position, &sampler.next2d());
    if ps.pdf <= 0.0 {
        return Color3::from_value(0.0);
    }

    let origin = surface.spawn_origin(settings);
    let to_light = ps.y - origin;
    let dist = to_light.magnitude();
    if dist <= RAY_EPS * 2.0 {
        return Color3::from_value(0.0);
    }
    let dir_world = to_light / dist;
    let max_t = RAY_EPS.mul_add(-2.0, dist);

    let shadow_ray = Ray::new(&origin, &dir_world).with_distance_max(max_t);
    let occluder = collect_surface_hits(&shadow_ray, scene, settings).into_nearest();
    if occluder.is_some() {
        return Color3::from_value(0.0);
    }

    let wi_local = frame.to_local(&dir_world);
    let fbsdf_cos = surface
        .material()
        .evaluate(wo, &wi_local, &surface.uv, &surface.position);

    let f_light = Frame::new(&ps.n);
    let wo_light = f_light.to_local(&(-dir_world));
    let le = shape
        .material()
        .emission(&wo_light, &surface.uv, &surface.position);

    let pdf_emitter = ps.pdf;
    let pdf_bsdf = surface
        .material()
        .pdf(wo, &wi_local, &surface.uv, &surface.position);
    let denom = pdf_bsdf + pdf_emitter;
    if denom <= 0.0 {
        return Color3::from_value(0.0);
    }

    let mis_w = pdf_emitter / denom;
    let tr = medium.map_or_else(|| Color3::from_value(1.0), |m| m.transmittance(dist));
    tr.mul_element_wise((fbsdf_cos / pdf_emitter).mul_element_wise(le)) * mis_w
}

fn direct_medium_nee(
    scene: &crate::scene::Scene,
    sampler: &mut dyn crate::samplers::Sampler,
    p: &Point3,
    wo: &Vec3,
    medium: &dyn Medium,
    settings: RaymarchSettings,
) -> Color3 {
    if !scene.has_analytic_emitters() {
        return Color3::from_value(0.0);
    }

    let (ps, shape) = scene.root.sample_direct(p, &sampler.next2d());
    if ps.pdf <= 0.0 {
        return Color3::from_value(0.0);
    }

    let to_light = ps.y - *p;
    let dist = to_light.magnitude();
    if dist <= RAY_EPS * 2.0 {
        return Color3::from_value(0.0);
    }

    let dir = to_light / dist;
    let shadow_ray = Ray::new(p, &dir).with_distance_max(RAY_EPS.mul_add(-2.0, dist));
    if collect_surface_hits(&shadow_ray, scene, settings)
        .into_nearest()
        .is_some()
    {
        return Color3::from_value(0.0);
    }

    let tr = medium.transmittance(dist);
    let phase = medium.phase_function();
    let pdf_light = ps.pdf;
    let pdf_phase = phase.phase_func(wo, &dir);
    let denom = pdf_light + pdf_phase;
    if denom <= 0.0 {
        return Color3::from_value(0.0);
    }

    let frame_light = Frame::new(&ps.n);
    let le = shape
        .material()
        .emission(&frame_light.to_local(&-dir), &Vec2::zero(), p);
    let mis_w = pdf_light / denom;

    tr.mul_element_wise(le) * (pdf_phase / pdf_light) * mis_w
}

fn russian_roulette(
    throughput: &mut Color3,
    sampler: &mut dyn crate::samplers::Sampler,
    depth: usize,
) -> bool {
    if depth < 3 {
        return false;
    }

    let q = throughput
        .x
        .max(throughput.y)
        .max(throughput.z)
        .clamp(0.0, 0.99);
    if q <= 0.0 {
        return true;
    }

    if sampler.next() > q {
        return true;
    }

    *throughput /= q;
    false
}
