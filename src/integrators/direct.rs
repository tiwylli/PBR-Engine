use super::{Integrator, SamplerIntegrator, render};
use crate::{
    array2d::Array2d,
    json::json_to_string,
    materials::SampledDirection,
    ray::Ray,
    samplers::{Sampler, pdf_hemisphere, sample_hemisphere},
    scene::Scene,
    shapes::Intersection,
    vec::{Color3, Frame},
};
use cgmath::{ElementWise, InnerSpace, Zero};
use std::collections::HashMap;
use tinyjson::JsonValue;

enum DirectType {
    Bsdf,
    Naive,
    Emitter,
    Mis,
}

pub struct DirectIntegrator {
    direct_type: DirectType,
}

impl DirectIntegrator {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        let strategy = json_to_string(json, "strategy", "bsdf");
        let direct_type = match strategy.as_str() {
            "bsdf" => DirectType::Bsdf,
            "naive" => DirectType::Naive,
            "emitter" => DirectType::Emitter,
            "mis" => DirectType::Mis,
            _ => panic!("Unknown strategy {strategy}"),
        };
        Self { direct_type }
    }
}

impl Integrator for DirectIntegrator {
    fn render(&mut self, scene: &Scene, sampler: &mut dyn Sampler) -> Array2d<Color3> {
        render(self, scene, sampler)
    }
}

impl SamplerIntegrator for DirectIntegrator {
    fn preprocess(&mut self, _: &Scene, _: &mut dyn Sampler) {}

    fn li(&self, ray: &Ray, scene: &Scene, sampler: &mut dyn Sampler) -> Color3 {
        scene.hit(ray).map_or_else(
            || scene.background(ray.d),
            |intersection| {
                if intersection.material.have_emission() {
                    let frame = Frame::new(&intersection.n);
                    intersection.material.emission(
                        &frame.to_local(&-ray.d),
                        &intersection.uv,
                        &intersection.p,
                    )
                } else {
                    match self.direct_type {
                        DirectType::Bsdf => eval_bsdf(ray, &intersection, scene, sampler),
                        DirectType::Naive => eval_naive(ray, &intersection, scene, sampler),
                        DirectType::Emitter => eval_emitter(ray, &intersection, scene, sampler),
                        DirectType::Mis => eval_mis(ray, &intersection, scene, sampler),
                    }
                }
            },
        )
    }
}

fn light_contribution(light_ray: Ray, scene: &Scene) -> Color3 {
    scene.hit(&light_ray).map_or_else(
        || scene.background(light_ray.d),
        |intersection| {
            let frame = Frame::new(&intersection.n);
            let dir_world = -light_ray.d;
            let dir_local = frame.to_local(&dir_world);
            intersection
                .material
                .emission(&dir_local, &intersection.uv, &intersection.p)
        },
    )
}

fn eval_bsdf(
    ray: &Ray,
    intersection: &Intersection,
    scene: &Scene,
    sampler: &mut dyn Sampler,
) -> Color3 {
    let frame = Frame::new(&intersection.n);
    let dir_world = -ray.d;
    let dir_local = frame.to_local(&dir_world);

    intersection
        .material
        .sample(
            &dir_local,
            &intersection.uv,
            &intersection.p,
            &sampler.next2d(),
        )
        .map_or_else(Color3::zero, |sampled_direction| {
            let light_ray = Ray::new(&intersection.p, &frame.to_world(&sampled_direction.wi));
            let light_contribution = light_contribution(light_ray, scene);
            sampled_direction
                .weight
                .mul_element_wise(light_contribution)
        })
}

fn eval_naive(
    ray: &Ray,
    intersection: &Intersection,
    scene: &Scene,
    sampler: &mut dyn Sampler,
) -> Color3 {
    let frame = Frame::new(&intersection.n);
    let dir_world = -ray.d;
    let dir_local = frame.to_local(&dir_world);

    let sample = if intersection.material.have_delta() {
        intersection.material.sample(
            &dir_local,
            &intersection.uv,
            &intersection.p,
            &sampler.next2d(),
        )
    } else {
        let wi = sample_hemisphere(&sampler.next2d());
        Some(SampledDirection {
            wi,
            weight: intersection.material.evaluate(
                &dir_local,
                &wi,
                &intersection.uv,
                &intersection.p,
            ) / pdf_hemisphere(&wi),
        })
    };
    sample.map_or_else(Color3::zero, |sampled_direction| {
        let light_ray = Ray::new(&intersection.p, &frame.to_world(&sampled_direction.wi));
        let light_contribution = light_contribution(light_ray, scene);
        sampled_direction
            .weight
            .mul_element_wise(light_contribution)
    })
}

fn eval_emitter(
    ray: &Ray,
    intersection: &Intersection,
    scene: &Scene,
    sampler: &mut dyn Sampler,
) -> Color3 {
    let frame = Frame::new(&intersection.n);
    let dir_world = -ray.d;
    let dir_local = frame.to_local(&dir_world);

    if intersection.material.have_delta() {
        eval_bsdf(ray, intersection, scene, sampler)
    } else {
        let (es, shape) = scene.sample_direct(&intersection.p, &sampler.next2d());

        if scene.visible(&intersection.p, &es.y) {
            let wi_world = (es.y - intersection.p).normalize();
            let wi_local = frame.to_local(&wi_world);
            let frame_light = Frame::new(&es.n);
            shape
                .material()
                .emission(
                    &frame_light.to_local(&-wi_world),
                    &intersection.uv,
                    &intersection.p,
                )
                .mul_element_wise(intersection.material.evaluate(
                    &dir_local,
                    &wi_local,
                    &intersection.uv,
                    &intersection.p,
                ))
                / es.pdf
        } else {
            Color3::zero()
        }
    }
}

pub fn eval_mis(
    ray: &Ray,
    intersection: &Intersection,
    scene: &Scene,
    sampler: &mut dyn Sampler,
) -> Color3 {
    let frame = Frame::new(&intersection.n);
    let dir_world = -ray.d;
    let dir_local = frame.to_local(&dir_world);

    let mut contribution = Color3::zero();

    // emitter sampling
    {
        let (es, shape) = scene.sample_direct(&intersection.p, &sampler.next2d());
        if scene.visible(&intersection.p, &es.y) && !intersection.material.have_delta() {
            let wi_world = (es.y - intersection.p).normalize();
            let wi_local = frame.to_local(&wi_world);
            let pdf_bsdf =
                intersection
                    .material
                    .pdf(&dir_local, &wi_local, &intersection.uv, &intersection.p);
            let mis_w = es.pdf / (pdf_bsdf + es.pdf);

            let frame_light = Frame::new(&es.n);
            contribution += mis_w
                * shape
                    .material()
                    .emission(
                        &frame_light.to_local(&-wi_world),
                        &intersection.uv,
                        &intersection.p,
                    )
                    .mul_element_wise(intersection.material.evaluate(
                        &dir_local,
                        &wi_local,
                        &intersection.uv,
                        &intersection.p,
                    ))
                / es.pdf;
        }
    }
    // bsdf sampling
    if let Some(sampled_direction) = intersection.material.sample(
        &dir_local,
        &intersection.uv,
        &intersection.p,
        &sampler.next2d(),
    ) {
        // cast new ray to try and find a light source
        let light_ray = Ray::new(&intersection.p, &frame.to_world(&sampled_direction.wi));
        if let Some(light_intersection) = scene.hit(&light_ray)
            && light_intersection.material.have_emission()
        {
            let mis_w = if intersection.material.have_delta() {
                1.0
            } else {
                let pdf_emitter = light_intersection.shape.pdf_direct(
                    light_intersection.shape,
                    &intersection.p,
                    &light_intersection.p,
                    &light_intersection.n,
                );
                let pdf_bsdf = intersection.material.pdf(
                    &dir_local,
                    &sampled_direction.wi,
                    &intersection.uv,
                    &intersection.p,
                );
                pdf_bsdf / (pdf_bsdf + pdf_emitter)
            };

            let frame_light = Frame::new(&light_intersection.n);

            let light_contribution = light_intersection.material.emission(
                &frame_light.to_local(&-light_ray.d),
                &intersection.uv,
                &intersection.p,
            );

            contribution += mis_w
                * sampled_direction
                    .weight
                    .mul_element_wise(light_contribution);
        }
    }

    contribution
}
