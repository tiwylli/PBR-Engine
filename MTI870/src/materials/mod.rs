use std::{collections::HashMap, sync::Arc};

use cgmath::{InnerSpace, Zero};
use tinyjson::JsonValue;

use crate::{
    samplers::Sampler,
    vec::{Color3, Vec2, Vec3},
};

pub struct SampledDirection {
    pub weight: Color3,
    pub wi: Vec3,
}

// pub trait Material: Send + Sync {
//     fn sample(&self, wo: &Vec3, s: &mut dyn Sampler) -> Option<SampledDirection>;
//     fn emission(&self, wo: &Vec3) -> Color3;
//     fn have_emission(&self) -> bool;
// }

pub trait Material: Send + Sync {
    //  !!!!!!!!!!!!!!!!!!! Attention !!!!!!!!!!!!!!!!!!!
    // cette méthode remplace votre méthode "sample" utilisant un "Sampler& s"
    //  !!!!!!!!!!!!!!!!!!! Attention !!!!!!!!!!!!!!!!!!!
    // En effet, nous allons utiliser un nombre aléatoire 2D (const Vec2& sample)
    // pour échantillonner une direction proportionnelle à "f_r(...) cos(theta)"
    // (si possible)
    fn sample(&self, wo: &Vec3, s: &Vec2) -> Option<SampledDirection>;
    // nouvelle méthode pour évaluer la valeur du matériau f_r(d_in, d_out)
    // multipliée par le cosinus -- Mettre en œuvre pour tâche 4
    fn evaluate(&self, wo: &Vec3, wi: &Vec3) -> Color3;
    // nouvelle méthode pour évaluer la valeur de la PDF p(wi | wo)
    // testée seulement dans la tâche 4
    fn pdf(&self, wo: &Vec3, wi: &Vec3) -> f64;
    // nouvelle méthode pour signifier si le matériau contient une composante discrète
    fn have_delta(&self) -> bool;
    // Méthode pour évaluer la couleur émise par le matériau
    fn emission(&self, wo: &Vec3) -> Color3;
    // Méthode pour signifier si le matériau émet de la lumière
    fn have_emission(&self) -> bool;
}

// pub fn random_in_unit_sphere(s: &mut dyn Sampler) -> Vec3 {
//     let mut p = Vec3::zero();
//     loop {
//         p.x = 2.0 * s.next() - 1.0;
//         p.y = 2.0 * s.next() - 1.0;
//         p.z = 2.0 * s.next() - 1.0;

//         if p.magnitude2() < 1.0 {
//             break;
//         }
//     }
//     p
// }
pub fn random_in_unit_sphere(s: f64) -> Vec3 {
    let mut p = Vec3::zero();
    loop {
        p.x = 2.0 * s - 1.0;
        p.y = 2.0 * s - 1.0;
        p.z = 2.0 * s - 1.0;

        if p.magnitude2() < 1.0 {
            break;
        }
    }
    p
}

pub mod blend;
pub mod car_paint;
pub mod dielectric;
pub mod diffuse;
pub mod diffuse_emitter;
pub mod diffuse_oren_nayar;
pub mod fresnel_blend;
pub mod metal;
pub mod phong;
pub mod subsurface;
pub mod translucent;
pub mod transparent;

pub fn json_to_material(json: &HashMap<String, JsonValue>) -> Arc<dyn Material> {
    if !json.contains_key("type") {
        panic!(
            "Need to specify 'type' variable to create the material.\n{:?}.",
            json
        );
    }

    let t: String = json["type"].clone().try_into().unwrap();
    match t.as_str() {
        "diffuse" => Arc::new(diffuse::Diffuse::from_json(json)),
        "metal" => Arc::new(metal::Metal::from_json(json)),
        "dielectric" => Arc::new(dielectric::Dielectric::from_json(json)),
        "diffuse_light" => Arc::new(diffuse_emitter::DiffuseEmit::from_json(json)),
        "blend" => Arc::new(blend::Blend::from_json(json)),
        "fresnel_blend" => Arc::new(fresnel_blend::FresnelBlend::from_json(json)),
        "phong" => Arc::new(phong::Phong::from_json(json)),
        "diffuse_oren_nayar" => Arc::new(diffuse_oren_nayar::OrenNayar::from_json(json)),
        "transparent" => Arc::new(transparent::Transparent::from_json(json)),
        "translucent" => Arc::new(translucent::Translucent::from_json(json)),
        "subsurface" => Arc::new(subsurface::Subsurface::from_json(json)),
        "car_paint" => Arc::new(car_paint::CarPaint::from_json(json)),
        _ => unimplemented!(),
    }
}
