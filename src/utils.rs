use std::fmt::Write;

use cgmath::{InnerSpace, Zero};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::{error, info, warn};

use crate::{
    array2d::Array2d,
    samplers::Sampler,
    vec::{Color3, Vec2, Vec2u, Vec3},
};

#[must_use]
pub fn spherical_coordinates_to_direction(phi: f64, theta: f64) -> Vec3 {
    let cos_theta = theta.cos();
    let sin_theta = theta.sin();
    let cos_phi = phi.cos();
    let sin_phi = phi.sin();
    Vec3::new(sin_theta * cos_phi, sin_theta * sin_phi, cos_theta)
}

#[must_use]
pub fn direction_to_spherical_coordinates(v: Vec3) -> (f64, f64) {
    (-v.y.atan2(-v.x) + std::f64::consts::PI, v.z.acos())
}

fn pixel_to_direction(p: Vec2, image_size: Vec2u) -> Vec3 {
    spherical_coordinates_to_direction(
        p.x * 2.0 * std::f64::consts::PI / f64::from(image_size.x),
        p.y * std::f64::consts::PI / f64::from(image_size.y),
    )
}

fn direction_to_pixel(d: Vec3, image_size: Vec2u) -> Vec2u {
    let sc = direction_to_spherical_coordinates(d);
    Vec2u::new(
        (sc.0 * f64::from(image_size.x) / (2.0 * std::f64::consts::PI)) as u32,
        (sc.1 * f64::from(image_size.y) / std::f64::consts::PI) as u32,
    )
}

// Based on https://www.shadertoy.com/view/WlfXRN
// Give a color (viridis) based on float value
#[allow(clippy::unreadable_literal)]
fn color(t: f64) -> Color3 {
    let c0 = Color3::new(0.2777273272234177, 0.005407344544966578, 0.3340998053353061);
    let c1 = Color3::new(0.1050930431085774, 1.404613529898575, 1.384590162594685);
    let c2 = Color3::new(-0.3308618287255563, 0.214847559468213, 0.09509516302823659);
    let c3 = Color3::new(-4.634230498983486, -5.799100973351585, -19.33244095627987);
    let c4 = Color3::new(6.228269936347081, 14.17993336680509, 56.69055260068105);
    let c5 = Color3::new(4.776384997670288, -13.74514537774601, -65.35303263337234);
    let c6 = Color3::new(-5.435455855934631, 4.645852612178535, 26.3124352495832);

    c0 + t * (c1 + t * (c2 + t * (c3 + t * (c4 + t * (c5 + t * c6)))))
}

type PdfFunction<'a> = dyn Fn(&Vec3) -> f64 + 'a;
type SampleFunction<'a> = dyn Fn(&Vec2) -> Vec3 + 'a;

pub fn generate_histogram<'a>(
    pdf_f: &PdfFunction<'a>,
    sample_f: &SampleFunction<'a>,
    nb_samples: u32,
    sampler: &mut dyn Sampler,
) -> (Array2d<Color3>, Array2d<Color3>, Array2d<Color3>) {
    // Size of the output image
    let image_size = Vec2u::new(512, 256);

    // Prepare progress bar
    let progress = ProgressBar::new(u64::from(image_size.y));
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

    // Check if there is numerical issues
    let mut nan_or_inf = false;

    // Compute PDF values
    let mut integral = 0.0;
    let mut pdf = Array2d::with_size(image_size.x, image_size.y, 0.0);
    for y in 0..image_size.y {
        progress.inc(1);
        for x in 0..image_size.x {
            let mut acc = 0.0;
            for _ in 0..nb_samples {
                let pos_img = Vec2::new(f64::from(x), f64::from(y)) + sampler.next2d();
                let dir = pixel_to_direction(pos_img, image_size);
                let sin_theta = dir.z.mul_add(-dir.z, 1.0).max(0.0).sqrt();
                let pixel_area = (std::f64::consts::PI / f64::from(image_size.y))
                    * (std::f64::consts::PI * 2.0 / f64::from(image_size.x))
                    * sin_theta;
                let pdf_val = pdf_f(&dir);
                if pdf_val.is_infinite() || pdf_val.is_nan() {
                    if !nan_or_inf {
                        warn!("PDF is NaN or Inf at ({x}, {y}) -- dir: {dir:?}");
                    }
                    nan_or_inf = true;
                    continue;
                }
                acc += pdf_val;
                integral += pdf_val * pixel_area;
            }
            *pdf.at_mut(x, y) = acc / f64::from(nb_samples);
        }
    }
    // Normalize integral
    integral /= f64::from(nb_samples);
    progress.is_finished();

    // Compute sample histogram
    let mut histogram = Array2d::with_size(image_size.x, image_size.y, 0.0);
    let normalisation =
        1.0 / (std::f64::consts::PI * (2.0 * std::f64::consts::PI) * f64::from(nb_samples));
    progress.reset();
    for _ in 0..image_size.y {
        progress.inc(1);
        for _ in 0..image_size.x {
            for _ in 0..nb_samples {
                let rnd = sampler.next2d();
                let dir = sample_f(&rnd);

                let dir_infinite =
                    dir.x.is_infinite() || dir.y.is_infinite() || dir.z.is_infinite();
                let dir_nan = dir.x.is_nan() || dir.y.is_nan() || dir.z.is_nan();
                if dir_infinite && !dir_nan {
                    if !nan_or_inf {
                        warn!("Echantillon invalide pour le nombre aleatoire: {rnd:?}");
                    }
                    nan_or_inf = true;
                    continue;
                }

                if dir.dot(dir) == 0.0 {
                    //warn!("Sampled direction is zero: {:?}", dir);
                    continue;
                }

                let pixel = direction_to_pixel(dir, image_size);
                if pixel.x >= image_size.x || pixel.y >= image_size.y {
                    continue;
                }

                let sin_theta = dir.z.mul_add(-dir.z, 1.0).max(0.0).sqrt();
                let weight = normalisation / sin_theta;
                *histogram.at_mut(pixel.x, pixel.y) += weight;
            }
        }
    }
    progress.is_finished();

    // Compute exposure with 95% median
    let mut pdf_1d = vec![0.0; (image_size.x * image_size.y) as usize];
    for y in 0..image_size.y {
        for x in 0..image_size.x {
            pdf_1d[(y * image_size.x + x) as usize] = *pdf.at(x, y);
        }
    }
    pdf_1d.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut exposure = pdf_1d[(pdf_1d.len() as f64 * 0.9995) as usize];
    if exposure == 0.0 {
        exposure = 1.0;
    }

    if nan_or_inf {
        error!("Certaines directions ou PDF donnent sont invalide.");
        error!("Verifiez que vous gerer tous les 'corner cases'. ");
    }

    // Compute final image
    let mut histogram_image = Array2d::with_size(image_size.x, image_size.y, Color3::zero());
    let mut pdf_image = Array2d::with_size(image_size.x, image_size.y, Color3::zero());
    let mut diff_image = Array2d::with_size(image_size.x, image_size.y, Color3::zero());
    let mut difference = 0.0;
    for y in 0..image_size.y {
        for x in 0..image_size.x {
            let pdf_val = *pdf.at(x, y);
            let histogram_val = *histogram.at(x, y);
            let diff = pdf_val - histogram_val;
            difference += diff;

            let pdf_color = color(pdf_val / exposure);
            let histogram_color = color(histogram_val / exposure);
            let diff_color = if diff < 0.0 {
                Color3::new(-diff / exposure, 0.0, 0.0)
            } else {
                Color3::new(0.0, diff / exposure, 0.0)
            };

            *pdf_image.at_mut(x, y) = pdf_color;
            *histogram_image.at_mut(x, y) = histogram_color;
            *diff_image.at_mut(x, y) = diff_color;
        }
    }

    info!("Integrale de la pdf (devrait etre proche de 1): {integral}");
    info!("99.95% percentile de la pdf: {exposure}");
    info!(
        "La difference entre l'histogramme et la pdf (devrait etre proche de 0): {}",
        difference / f64::from(image_size.x * image_size.y)
    );

    (pdf_image, histogram_image, diff_image)
}
