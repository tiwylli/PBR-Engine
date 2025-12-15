use std::{collections::HashMap, sync::Arc};

use crate::{
    aabb::AABB,
    materials::Material,
    ray::Ray,
    transform::json_to_transform,
    vec::{Point3, Vec2, Vec3},
    Real,
};
use cgmath::Zero;
use tinyjson::JsonValue;

pub struct Intersection<'a> {
    /// Intersection distance
    pub t: f64,
    /// Intersection point
    pub p: Point3,
    /// Surface normal
    pub n: Vec3,
    /// Material at the intersection point
    pub material: &'a dyn Material,
    /// Forme intersected
    pub shape: &'a dyn Shape,
}

pub trait Shape: Send + Sync {
    fn hit<'a>(&'a self, r: &Ray) -> Option<Intersection<'a>>;
    // Échantillonnage d'un point sur la source lumineuse depuis le shading point x
    fn sample_direct(&self, p: &Point3, sample: &Vec2) -> (EmitterSample, &dyn Shape);
    // Densité de probabilité en **angle solide** de x vers le point y
    fn pdf_direct(&self, shape: &dyn Shape, p: &Point3, y: &Point3, n: &Vec3) -> Real;
    // Retourne le matériau associé à la forme (utile pour identifier une lumière)
    /// Add shape to the group
    fn add_shape(&mut self, shape: Box<dyn Shape>);
    /// Build the acceleration structure
    fn build(&mut self);
    /// Compute the bouding box for the given shape
    fn aabb(&self) -> AABB;
    // Permet de connaitre le matériau (et savoir si source de lumière)
    fn material(&self) -> &dyn Material;
}

pub mod bvh;
pub mod mesh;
pub mod quad;
pub mod shape_group;
pub mod sphere;
pub mod triangle;

/// Struct to distingush between simple shapes and collection of shapes
pub enum JsonShape {
    Shape(Box<dyn Shape>),
    Mesh(Option<mesh::Mesh>),
}

// Implementation for unit tests (e.g., devoir1)
impl JsonShape {
    pub fn as_ref<'a>(&'a self) -> &'a dyn Shape {
        match self {
            JsonShape::Shape(s) => s.as_ref(),
            JsonShape::Mesh(_) => unimplemented!(),
        }
    }
}

pub fn json_to_shape(json: &HashMap<String, JsonValue>, material: Arc<dyn Material>) -> JsonShape {
    let transform = json_to_transform(json, "transform");

    if !json.contains_key("type") {
        panic!(
            "Need to specify 'type' variable to create the shape.\n{:?}.",
            json
        );
    }

    let t: String = json["type"].clone().try_into().unwrap();
    match t.as_str() {
        "sphere" => JsonShape::Shape(Box::new(sphere::Sphere::from_json(
            json, transform, material,
        ))),
        "quad" => JsonShape::Shape(Box::new(quad::Quad::from_json(json, transform, material))),
        "mesh" => JsonShape::Mesh(mesh::Mesh::from_json(json, transform, material)),
        "triangle" => JsonShape::Shape(Box::new(triangle::Triangle::from_json(json, material))),
        _ => unimplemented!(),
    }
}

pub struct EmitterSample {
    /// Point on the emitter surface
    pub y: Point3,
    /// Normal at the point on the emitter surface if available else n = normalized (x - p)
    pub n: Vec3,
    /// PDF of the sample in solid angle
    pub pdf: Real,
}
impl EmitterSample {
    pub fn new() -> Self {
        EmitterSample {
            y: Point3::new(0.0, 0.0, 0.0),
            n: Vec3::zero(),
            pdf: 0.0,
        }
    }
}
