use std::{collections::HashMap, fmt::Write};

use cgmath::Zero;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::info;
use rayon::prelude::*;
use tinyjson::JsonValue;

use crate::{
    array2d::Array2d,
    ray::Ray,
    samplers::Sampler,
    scene::Scene,
    vec::{Color3, Vec2, Vec2u},
    NUMBER_INTERSECTIONS, NUMBER_TRACED_RAYS,
};

use self::path::PathIntegrator;

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

    const BLOCKSIZE: u32 = 32;
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
            write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
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
                let mut avg = Color3::zero();
                for _ in 0..task.sampler.nb_samples() {
                    let pos_img = Vec2::new(x as f64, y as f64) + task.sampler.next2d();
                    let ray = scene.camera.generate_ray(&pos_img, task.sampler.as_mut());
                    let value = int.li(&ray, scene, task.sampler.as_mut());
                    avg += value;
                }

                *task.im.at_mut(lx, ly) = avg / sampler.nb_samples() as f64;
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
    info!(" - #intersections: {}", total_its);
    info!(" - #rays(traced) : {}", total_rays);
    info!(
        " - #intersections/#ray(traced): {}",
        total_its as f64 / total_rays as f64
    );

    im
}

pub mod direct;
pub mod hybrid_direct;
pub mod normal;
pub mod path;
pub mod path_mis;
pub mod sdf;
mod sdf_common;
pub mod sdf_direct;
pub mod sdf_path_mis;

pub fn json_to_integrator(json: &HashMap<String, JsonValue>) -> Box<dyn Integrator> {
    if !json.contains_key("type") {
        panic!(
            "Need to specify 'type' variable to create the material.\n{:?}.",
            json
        );
    }

    let t: String = json["type"].clone().try_into().unwrap();
    match t.as_str() {
        "normal" => Box::new(normal::NormalIntegrator {}),
        "path" => Box::new(PathIntegrator::from_json(json)),
        "direct" => Box::new(direct::DirectIntegrator::from_json(json)),
        "hybrid_direct" => Box::new(hybrid_direct::HybridDirectIntegrator::from_json(json)),
        "path_mis" => Box::new(path_mis::PathMISIntegrator::from_json(json)),
        "sdf_direct" => Box::new(sdf_direct::SDFDirectIntegrator::from_json(json)),
        "sdf_path_mis" => Box::new(sdf_path_mis::PathMISIntegrator::from_json(json)),
        _ => panic!("Unknown integrator type: {}", t),
    }
}
