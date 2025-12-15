use std::f64::consts::PI;

use cgmath::Zero;
use log::info;
use render::{
    function,
    image::image_save,
    samplers::independent::Independent,
    utils::generate_histogram,
    vec::{Vec2, Vec3},
    votrecodeici,
};

fn main() {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    task2();
}

fn sample_spherical(sample: &Vec2) -> Vec3 {
    let cos_theta = 1.0 - 2.0 * sample.x;
    let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
    let phi = 2.0 * PI * sample.y;
    let x = sin_theta * phi.cos();
    let y = sin_theta * phi.sin();
    Vec3::new(x, y, cos_theta)
}

fn pdf_spherical(_dir: &Vec3) -> f64 {
    1.0 / (4.0 * PI)
}

fn sample_hemisphere(sample: &Vec2) -> Vec3 {
    let theta = (1.0 - sample.x).acos();
    let phi = 2.0 * PI * sample.y;
    let x = theta.sin() * phi.cos();
    let y = theta.sin() * phi.sin();
    let z = theta.cos();
    Vec3::new(x, y, z)
}

fn pdf_hemisphere(dir: &Vec3) -> f64 {
    if dir.z > 0.0 {
        1.0 / (2.0 * PI)
    } else {
        0.0
    }
}

fn sample_cosine_hemisphere(sample: &Vec2) -> Vec3 {
    let r = sample.x.sqrt();
    let theta = 2.0 * PI * sample.y;
    let x = r * theta.cos();
    let y = r * theta.sin();
    let z = (1.0 - x * x - y * y).sqrt();
    Vec3::new(x, y, z)
}

fn pdf_cosine_hemisphere(dir: &Vec3) -> f64 {
    if dir.z > 0.0 {
        dir.z / PI
    } else {
        0.0
    }
}

fn sample_cosine_hemisphere_power(sample: &Vec2, power: f64) -> Vec3 {
    let phi = 2.0 * PI * sample.x;

    let inv = 1.0 / (power + 1.0);
    let cos_theta = (1.0 - sample.y).powf(inv);

    let sin_theta_sq = (1.0 - cos_theta * cos_theta).max(0.0);
    let sin_theta = sin_theta_sq.sqrt();
    let sin_phi = phi.sin();
    let cos_phi = phi.cos();

    Vec3::new(cos_phi * sin_theta, sin_phi * sin_theta, cos_theta)
}

fn pdf_cosine_hemisphere_power(dir: &Vec3, power: f64) -> f64 {
    let cos_theta = dir.z;
    if cos_theta <= 0.0 {
        0.0
    } else {
        (power + 1.0) * cos_theta.powf(power) / (2.0 * PI)
    }
}

fn sample_cone(sample: &Vec2, theta_max: f64) -> Vec3 {
    let cos_theta_max = theta_max.cos();

    let phi = 2.0 * PI * sample.x;
    let cos_theta = 1.0 - sample.y * (1.0 - cos_theta_max);
    let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
    let sin_phi = phi.sin();
    let cos_phi = phi.cos();
    Vec3::new(cos_phi * sin_theta, sin_phi * sin_theta, cos_theta)
}

fn pdf_cone(dir: &Vec3, theta_max: f64) -> f64 {
    let cos_theta_max = theta_max.cos();
    let cos_theta = dir.z;

    if cos_theta < cos_theta_max {
        0.0
    } else {
        1.0 / (2.0 * PI * (1.0 - cos_theta_max))
    }
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
