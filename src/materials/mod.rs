use std::{collections::HashMap, sync::Arc};

use cgmath::{Array, InnerSpace, Zero};
use tinyjson::JsonValue;

use crate::{
    samplers::Sampler,
    texture::{Texture, json_to_texture},
    vec::{Color3, Point3, Vec2, Vec3},
};

pub struct SampledDirection {
    pub weight: Color3,
    pub wi: Vec3,
}

pub trait Material: Send + Sync {
    fn sample(&self, wo: &Vec3, uv: &Vec2, p: &Point3, s: &Vec2) -> Option<SampledDirection>;
    fn evaluate(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> Color3;
    fn pdf(&self, wo: &Vec3, wi: &Vec3, uv: &Vec2, p: &Point3) -> f64;
    fn have_delta(&self) -> bool;
    fn emission(&self, wo: &Vec3, uv: &Vec2, p: &Point3) -> Color3;
    fn have_emission(&self) -> bool;
    fn get_normal_map_value(&self, _uv: &Vec2, _p: &Point3) -> Vec3 {
        Vec3::unit_z()
    }
    fn have_normal_map(&self) -> bool {
        false
    }
    fn get_albedo(&self, _uv: &Vec2, _p: &Point3) -> Color3 {
        Color3::from_value(1.0)
    }
}

pub fn random_in_unit_sphere(s: &mut dyn Sampler) -> Vec3 {
    let mut p = Vec3::zero();
    loop {
        p.x = 2.0f64.mul_add(s.next(), -1.0);
        p.y = 2.0f64.mul_add(s.next(), -1.0);
        p.z = 2.0f64.mul_add(s.next(), -1.0);

        if p.magnitude2() < 1.0 {
            break p;
        }
    }
}

pub fn random_in_unit_disk(s: &mut dyn Sampler) -> Vec3 {
    let mut p = Vec3::zero();
    loop {
        p.x = 2.0f64.mul_add(s.next(), -1.0);
        p.y = 2.0f64.mul_add(s.next(), -1.0);

        if p.magnitude2() < 1.0 {
            break p;
        }
    }
}

pub mod anisotropic_metal;
pub mod blend;
pub mod car_paint;
pub mod dielectric;
pub mod diffuse;
pub mod diffuse_emitter;
pub mod diffuse_oren_nayar;
pub mod fresnel_blend;
pub mod metal;
pub mod phong;
pub mod principled_bsdf;
pub mod subsurface;
pub mod translucent;
pub mod transparent;

#[must_use]
pub fn json_to_material(json: &HashMap<String, JsonValue>) -> Arc<dyn Material> {
    assert!(
        json.contains_key("type"),
        "Need to specify 'type' variable to create the material.\n{json:?}."
    );

    let t: String = json["type"].clone().try_into().unwrap();
    match t.as_str() {
        "diffuse" => Arc::new(diffuse::Diffuse::from_json(json)),
        "metal" => Arc::new(metal::Metal::from_json(json)),
        "dielectric" => Arc::new(dielectric::Dielectric::from_json(json)),
        "diffuse_light" => Arc::new(diffuse_emitter::DiffuseEmit::from_json(json)),
        "diffuse_spotlight" => Arc::new(diffuse_emitter::DiffuseEmitSpotLight::from_json(json)),
        "blend" => Arc::new(blend::Blend::from_json(json)),
        "fresnel_blend" => Arc::new(fresnel_blend::FresnelBlend::from_json(json)),
        "phong" => Arc::new(phong::Phong::from_json(json)),
        "anisotropic-metal" => Arc::new(anisotropic_metal::AnisotropicMetal::from_json(json)),
        "car_paint" => Arc::new(car_paint::CarPaint::from_json(json)),
        "subsurface" => Arc::new(subsurface::Subsurface::from_json(json)),
        "translucent" => Arc::new(translucent::Translucent::from_json(json)),
        "transparent" => Arc::new(transparent::Transparent::from_json(json)),
        "diffuse_oren_nayar" => Arc::new(diffuse_oren_nayar::OrenNayar::from_json(json)),
        "principled_bsdf" => Arc::new(principled_bsdf::PrincipledBsdf::from_json(json)),
        _ => unimplemented!(),
    }
}

const fn roughness_to_exponent(roughness: f64) -> Option<f64> {
    if roughness == 0.0 {
        None
    } else {
        Some(2.0 / (roughness * roughness) - 2.0)
    }
}

fn json_to_normal_map(json: &HashMap<String, JsonValue>) -> Option<Texture<Vec3>> {
    if json.contains_key("normal_map") {
        Some(json_to_texture(json, "normal_map", Vec3::unit_z()))
    } else {
        None
    }
}

fn get_normal_map_value_helper(normal_map: Option<&Texture<Vec3>>, uv: &Vec2, p: &Point3) -> Vec3 {
    normal_map.as_ref().map_or_else(Vec3::unit_z, |normal_map| {
        2.0 * (normal_map.get(uv, p) - Vec3::from_value(0.5))
    })
}
