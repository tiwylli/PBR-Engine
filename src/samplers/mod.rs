use std::{collections::HashMap, f64};

use tinyjson::JsonValue;

use crate::{
    json::json_to_f64,
    vec::{Vec2, Vec3, spherical_to_directional},
};

pub trait Sampler: Send + Sync {
    fn next(&mut self) -> f64;
    fn next2d(&mut self) -> Vec2;
    fn clone_box(&mut self) -> Box<dyn Sampler>;

    fn nb_samples(&self) -> usize;
    fn set_nb_samples(&mut self, nspp: usize);
}

pub mod independent;

#[must_use]
pub fn json_to_sampler(json: &HashMap<String, JsonValue>) -> Box<dyn Sampler> {
    assert!(
        json.contains_key("type"),
        "Need to specify 'type' variable to create the sampler.\n{json:?}."
    );

    let nspp = json_to_f64(json, "samples", 1.0) as usize;

    let t: String = json["type"].clone().try_into().unwrap();
    match t.as_str() {
        "independent" => Box::new(independent::Independent::new(nspp)),
        _ => panic!("Unknow shape type: {t}"),
    }
}

#[must_use]
pub fn sample_spherical(sample: &Vec2) -> Vec3 {
    let theta = sample.x.mul_add(2.0, -1.0).acos();
    let phi = sample.y * f64::consts::PI * 2.0;
    spherical_to_directional(theta, phi)
}

#[must_use]
pub fn pdf_spherical(_dir: &Vec3) -> f64 {
    f64::consts::FRAC_1_PI / 4.0 // 1 / 4pi
}

#[must_use]
pub fn sample_hemisphere(sample: &Vec2) -> Vec3 {
    let theta = sample.x.acos();
    let phi = sample.y * f64::consts::PI * 2.0;
    spherical_to_directional(theta, phi)
}

#[must_use]
pub fn pdf_hemisphere(dir: &Vec3) -> f64 {
    if dir.z < 0.0 {
        0.0
    } else {
        f64::consts::FRAC_1_PI / 2.0 // 1 / 2pi
    }
}

#[must_use]
pub fn sample_cosine_hemisphere(sample: &Vec2) -> Vec3 {
    let theta = (1.0 - sample.x).sqrt().acos();
    let phi = sample.y * f64::consts::PI * 2.0;
    spherical_to_directional(theta, phi)
}

#[must_use]
pub fn pdf_cosine_hemisphere(dir: &Vec3) -> f64 {
    if dir.z < 0.0 {
        0.0
    } else {
        dir.z * f64::consts::FRAC_1_PI
    }
}

#[must_use]
pub fn sample_cosine_hemisphere_power(sample: &Vec2, power: f64) -> Vec3 {
    let theta = (1.0 - sample.x).powf(1.0 / (power + 1.0)).acos();
    let phi = sample.y * f64::consts::PI * 2.0;
    spherical_to_directional(theta, phi)
}

#[must_use]
pub fn pdf_cosine_hemisphere_power(dir: &Vec3, power: f64) -> f64 {
    if dir.z < 0.0 {
        0.0
    } else {
        dir.z.powf(power) * (power + 1.0) * 0.5 * f64::consts::FRAC_1_PI
    }
}

#[must_use]
pub fn sample_cone(sample: &Vec2, theta_max: f64) -> Vec3 {
    let theta = sample.x.mul_add(theta_max.cos() - 1.0, 1.0).acos();
    let phi = sample.y * 2.0 * f64::consts::PI;
    spherical_to_directional(theta, phi)
}

#[must_use]
pub fn sample_cone_cos_theta_max(sample: &Vec2, cos_theta_max: f64) -> Vec3 {
    let theta = sample.x.mul_add(cos_theta_max - 1.0, 1.0).acos();
    let phi = sample.y * 2.0 * f64::consts::PI;
    spherical_to_directional(theta, phi)
}

#[must_use]
pub fn pdf_cone(dir: &Vec3, theta_max: f64) -> f64 {
    if dir.z < theta_max.cos() {
        0.0
    } else {
        1.0 / (2.0 * f64::consts::PI * (1.0 - theta_max.cos()))
    }
}

#[must_use]
pub fn pdf_cone_cos_theta_max(dir: &Vec3, cos_theta_max: f64) -> f64 {
    if dir.z < cos_theta_max {
        0.0
    } else {
        1.0 / (2.0 * f64::consts::PI * (1.0 - cos_theta_max))
    }
}

#[must_use]
#[allow(clippy::suboptimal_flops)]
pub fn sample_anisotropic_hemisphere(sample: &Vec2, nu: f64, nv: f64, phi_offset: f64) -> Vec3 {
    let (eta1, rot) = if sample.x < 0.25 {
        (4.0 * sample.x, 0.0)
    } else if sample.x < 0.5 {
        (sample.x.mul_add(4.0, -1.0), f64::consts::FRAC_PI_2)
    } else if sample.x < 0.75 {
        (sample.x.mul_add(4.0, -2.0), f64::consts::PI)
    } else {
        (sample.x.mul_add(4.0, -3.0), f64::consts::FRAC_PI_2 * 3.0)
    };
    let phi = (((nu + 1.0) / (nv + 1.0)).sqrt() * (eta1 * f64::consts::FRAC_PI_2).tan()).atan()
        + rot
        + phi_offset;
    let theta = (1.0 - sample.y)
        .powf(1.0 / (nu * phi.cos().powi(2) + nv * phi.sin().powi(2) + 1.0))
        .acos();
    spherical_to_directional(theta, phi)
}

#[must_use]
pub fn sample_inverse_cosine_hemisphere_power(sample: &Vec2, power: f64) -> Vec3 {
    let theta = f64::consts::FRAC_PI_2 - (1.0 - sample.x).powf(1.0 / power).acos();
    let phi = sample.y * f64::consts::PI * 2.0;
    spherical_to_directional(theta, phi)
}

#[must_use]
pub fn pdf_inverse_cosine_hemisphere_power(dir: &Vec3, power: f64) -> f64 {
    if dir.z < 0.0 {
        0.0
    } else {
        // does not quite work
        dir.z.acos().sin().powf(power) * (power + 1.0) * 0.5 * f64::consts::FRAC_1_PI
    }
}

#[must_use]
#[allow(clippy::unreadable_literal)]
pub fn hash2(p: Vec2) -> f64 {
    let mut h = p.x.to_bits().wrapping_mul(0x6C8E9CF5) ^ p.y.to_bits().wrapping_mul(0xB5297A4D);

    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846CA68B);
    h ^= h >> 16;

    (h as f64) / (u64::MAX as f64)
}
