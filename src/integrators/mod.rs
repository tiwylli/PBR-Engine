use std::{collections::HashMap, fmt::Write};

use cgmath::{Array, Zero};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::info;
use rayon::prelude::*;
use tinyjson::JsonValue;

use crate::{
    NUMBER_INTERSECTIONS, NUMBER_TRACED_RAYS,
    array2d::Array2d,
    integrators::direct::DirectIntegrator,
    ray::Ray,
    samplers::Sampler,
    scene::Scene,
    vec::{Color3, Vec2, Vec2u},
};

use self::hybrid_direct::HybridDirectIntegrator;
use self::hybrid_path::HybridPathIntegrator;
use self::hybrid_path_mis::HybridPathMisIntegrator;
use self::path::PathIntegrator;
use self::path_mis::PathMisIntegrator;
use self::sdf_direct::SDFDirectIntegrator;
use self::uv::UvIntegrator;
use self::vol_path::VolumetricPathIntegrator;

/// Abstract trait for integrating
pub trait Integrator {
    /// Generate an image by integrating the incoming light
    fn render(&mut self, scene: &Scene, sampler: &mut dyn Sampler) -> Array2d<Color3>;
}

/// Abstract trait for an integrator that integrates one pixel at a time
pub trait SamplerIntegrator {
    /// Method to perform a preprocessing step
    fn preprocess(&mut self, scene: &Scene, sampler: &mut dyn Sampler);

    /// Method that estimates the incoming light for a given ray
    fn li(&self, ray: &Ray, scene: &Scene, sampler: &mut dyn Sampler) -> Color3;
}

/// Default implementation of render for sampler integrator
fn render<T: SamplerIntegrator + Send + Sync>(
    int: &mut T,
    scene: &Scene,
    sampler: &mut dyn Sampler,
) -> Array2d<Color3> {
    const BLOCKSIZE: u32 = 32;

    pub struct RenderBlock {
        pub pos: Vec2u,
        pub size: Vec2u,
        pub im: Array2d<Color3>,
        pub sampler: Box<dyn Sampler>,
        // Statistics (after rendering done)
        pub nb_its: usize,
        pub nb_rays: usize,
    }

    info!("Integrator preprocess...");
    int.preprocess(scene, sampler);

    let mut tasks = vec![];
    for x in (0..scene.camera.resolution.x).step_by(BLOCKSIZE as usize) {
        for y in (0..scene.camera.resolution.y).step_by(BLOCKSIZE as usize) {
            let size = Vec2u::new(
                (scene.camera.resolution.x - x).min(BLOCKSIZE),
                (scene.camera.resolution.y - y).min(BLOCKSIZE),
            );
            let im = Array2d::with_size(size.x, size.y, Color3::zero());
            tasks.push(RenderBlock {
                pos: Vec2u::new(x, y),
                size,
                im,
                sampler: sampler.clone_box(),
                nb_its: 0,
                nb_rays: 0,
            });
        }
    }

    let progress = ProgressBar::new(tasks.len() as u64);
    progress.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar}] {pos:>7}/{len:7} ({eta})",
        )
        .unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| {
            write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap();
        })
        .progress_chars("#>-"),
    );

    info!("Rendering... ");
    tasks.par_iter_mut().for_each(|task| {
        NUMBER_INTERSECTIONS.with(|f| *f.borrow_mut() = 0);
        NUMBER_TRACED_RAYS.with(|f| *f.borrow_mut() = 0);

        // Rendering the local block
        for lx in 0..task.size.x {
            for ly in 0..task.size.y {
                // Compute the absolute pixel position on the image
                let x = lx + task.pos.x;
                let y = ly + task.pos.y;

                // Monte Carlo computations
                let mut sum = Color3::zero();
                let mut count = 0.0;
                for _ in 0..task.sampler.nb_samples() {
                    let pos_img = Vec2::new(f64::from(x), f64::from(y)) + task.sampler.next2d();
                    let ray = scene.camera.generate_ray(&pos_img, task.sampler.as_mut());
                    let value = int.li(&ray, scene, task.sampler.as_mut());
                    if !scene.ignore_nans || value.is_finite() {
                        sum += value;
                        count += 1.0;
                    }
                }

                *task.im.at_mut(lx, ly) = sum / count;
            }
        }

        NUMBER_INTERSECTIONS.with(|f| task.nb_its = *f.borrow());
        NUMBER_TRACED_RAYS.with(|f| task.nb_rays = *f.borrow());

        progress.inc(1);
    });

    //  Assemble the final image and compute stats
    let mut im = Array2d::with_size(
        scene.camera.resolution.x,
        scene.camera.resolution.y,
        Color3::zero(),
    );

    let mut total_its = 0;
    let mut total_rays = 0;
    for task in tasks {
        total_its += task.nb_its;
        total_rays += task.nb_rays;

        for x in 0..task.size.x {
            for y in 0..task.size.y {
                *im.at_mut(x + task.pos.x, y + task.pos.y) = *task.im.at(x, y);
            }
        }
    }

    info!("Stats: ");
    info!(" - #intersections: {total_its}");
    info!(" - #rays(traced) : {total_rays}");
    info!(
        " - #intersections/#ray(traced): {}",
        total_its as f64 / total_rays as f64
    );

    im
}

pub mod albedo;
pub mod direct;
pub mod hybrid_direct;
pub mod hybrid_path;
pub mod hybrid_path_mis;
pub mod hybrid_vol_path_mis;
pub mod normal;
pub mod path;
pub mod path_mis;
pub mod sdf;
pub mod sdf_common;
pub mod sdf_direct;
pub mod uv;
pub mod vol_path;

#[must_use]
pub fn json_to_integrator(json: &HashMap<String, JsonValue>) -> Box<dyn Integrator> {
    assert!(
        json.contains_key("type"),
        "Need to specify 'type' variable to create the integrator.\n{json:?}."
    );

    let t: String = json["type"].clone().try_into().unwrap();
    match t.as_str() {
        "normal" => Box::new(normal::NormalIntegrator::from_json(json)),
        "albedo" => Box::new(albedo::AlbedoIntegrator::from_json(json)),
        "uv" => Box::new(UvIntegrator {}),
        "path" => Box::new(PathIntegrator::from_json(json)),
        "volpath" => Box::new(VolumetricPathIntegrator::from_json(json)),
        "path_mis" => Box::new(PathMisIntegrator::from_json(json)),
        "direct" => Box::new(DirectIntegrator::from_json(json)),
        "hybrid_direct" => Box::new(HybridDirectIntegrator::from_json(json)),
        "hybrid_path" => Box::new(HybridPathIntegrator::from_json(json)),
        "hybrid_path_mis" | "sdf_path_mis" => Box::new(HybridPathMisIntegrator::from_json(json)),
        "hybrid_vol_path_mis" | "volpath_mis" | "sdf_volpath_mis" => Box::new(
            hybrid_vol_path_mis::HybridVolPathMisIntegrator::from_json(json),
        ),
        "sdf_direct" => Box::new(SDFDirectIntegrator::from_json(json)),
        _ => panic!("Unknown integrator type: {t}"),
    }
}
