#![allow(warnings)]
#![allow(clippy::all)]

use log::info;
use render::{image::image_save, samplers::independent::Independent, utils::generate_histogram};

use render::samplers::*;

fn main() {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    task2();
}

fn task2() {
    print!("=====================================\n");
    print!("Tache 2: Distributions simples\n");
    print!("=====================================\n");

    // Objet pour générer des nombres aléatoires
    let mut sampler = Box::new(Independent::new(1));

    // Spherical distribution
    {
        info!("Echantillonnage spherique");
        let (pdf, hist, diff) =
            generate_histogram(&pdf_spherical, &sample_spherical, 50, sampler.as_mut());
        image_save("sphere-hist.png", &hist).unwrap();
        image_save("sphere-pdf.png", &pdf).unwrap();
        image_save("sphere-diff.png", &diff).unwrap();
    }

    // Hemisphere distribution
    {
        info!("Echantillonnage hemisphere");
        let (pdf, hist, diff) =
            generate_histogram(&pdf_hemisphere, &sample_hemisphere, 50, sampler.as_mut());
        image_save("hemisphere-hist.png", &hist).unwrap();
        image_save("hemisphere-pdf.png", &pdf).unwrap();
        image_save("hemisphere-diff.png", &diff).unwrap();
    }

    // Cosine hemisphere distribution
    {
        info!("Echantillonnage hemisphere cosinus");
        let (pdf, hist, diff) = generate_histogram(
            &pdf_cosine_hemisphere,
            &sample_cosine_hemisphere,
            50,
            sampler.as_mut(),
        );
        image_save("hemisphere-cos-hist.png", &hist).unwrap();
        image_save("hemisphere-cos-pdf.png", &pdf).unwrap();
        image_save("hemisphere-cos-diff.png", &diff).unwrap();
    }

    // Cosine hemisphere power distribution
    {
        info!("Echantillonnage hemisphere cosinus puissance");
        let (pdf, hist, diff) = generate_histogram(
            &|dir| pdf_cosine_hemisphere_power(dir, 20.0),
            &|sample| sample_cosine_hemisphere_power(sample, 20.0),
            50,
            sampler.as_mut(),
        );
        image_save("hemisphere-cos-pow20-hist.png", &hist).unwrap();
        image_save("hemisphere-cos-pow20-pdf.png", &pdf).unwrap();
        image_save("hemisphere-cos-pow20-diff.png", &diff).unwrap();
    }

    // Cone distribution
    {
        info!("Echantillonnage cone");
        let (pdf, hist, diff) = generate_histogram(
            &|dir| pdf_cone(dir, std::f64::consts::PI * 0.4),
            &|sample| sample_cone(sample, std::f64::consts::PI * 0.4),
            50,
            sampler.as_mut(),
        );
        image_save("cone-hist.png", &hist).unwrap();
        image_save("cone-pdf.png", &pdf).unwrap();
        image_save("cone-diff.png", &diff).unwrap();
    }
}
