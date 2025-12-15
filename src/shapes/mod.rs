use std::{collections::HashMap, sync::Arc};

use tinyjson::JsonValue;

use cgmath::{EuclideanSpace, InnerSpace, MetricSpace, Zero};

use crate::{
    Real,
    aabb::AABB,
    materials::Material,
    ray::Ray,
    transform::json_to_transform,
    vec::{Point3, Vec2, Vec3},
};

pub struct Intersection<'a> {
    /// Intersection distance
    pub t: f64,
    /// Intersection point
    pub p: Point3,
    /// Surface normal
    pub n: Vec3,
    /// Texture coordinates
    pub uv: Vec2,
    /// Material at the intersection point
    pub material: &'a dyn Material,
    /// forme intersectée
    pub shape: &'a dyn Shape,
}

pub trait Shape: Send + Sync {
    fn hit<'a>(&'a self, r: &Ray) -> Option<Intersection<'a>>;

    // Échantillonnage d'un point sur la source lumineuse depuis le shading point x
    #[must_use]
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
pub mod spot_light;
pub mod triangle;

/// Struct to distingush between simple shapes and collection of shapes
pub enum JsonShape {
    Shape(Box<dyn Shape>),
    Mesh(Option<mesh::Mesh>),
}

// Implementation for unit tests (e.g., devoir1)
impl JsonShape {
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn as_ref(&self) -> &dyn Shape {
        match self {
            Self::Shape(s) => s.as_ref(),
            Self::Mesh(_) => unimplemented!(),
        }
    }
}

pub fn json_to_shape(json: &HashMap<String, JsonValue>, material: Arc<dyn Material>) -> JsonShape {
    let transform = json_to_transform(json, "transform");

    assert!(
        json.contains_key("type"),
        "Need to specify 'type' variable to create the shape.\n{json:?}."
    );

    let t: String = json["type"].clone().try_into().unwrap();
    match t.as_str() {
        "sphere" => JsonShape::Shape(Box::new(sphere::Sphere::from_json(
            json, transform, material,
        ))),
        "quad" => JsonShape::Shape(Box::new(quad::Quad::from_json(json, transform, material))),
        "mesh" => JsonShape::Mesh(mesh::Mesh::from_json(json, &transform, material)),
        "triangle" => JsonShape::Shape(Box::new(triangle::Triangle::from_json(json, material))),
        "spotlight" => JsonShape::Shape(Box::new(spot_light::SpotLight::from_json(
            json, &transform, material,
        ))),
        _ => unimplemented!(),
    }
}

pub struct EmitterSample {
    pub y: Point3, // position sur la source lumineuse
    pub n: Vec3,   // Normale associée au point (si applicable, sinon n = normalize(x - p))
    pub pdf: Real,
}

impl Default for EmitterSample {
    fn default() -> Self {
        Self {
            y: Point3::origin(),
            n: Vec3::zero(),
            pdf: 0.0,
        }
    }
}

fn surfacial_to_solid_angle(pdf: Real, p: &Point3, y: &Point3, n: &Vec3) -> Real {
    let cos_theta = (p - y).normalize().dot(*n);
    let dist = p.distance(*y);
    pdf * dist * dist / cos_theta.abs()
}

#[must_use]
pub fn solid_angle_to_surfacial(pdf: Real, p: &Point3, y: &Point3, n: &Vec3) -> Real {
    let cos_theta = (p - y).normalize().dot(*n);
    let dist = p.distance(*y);
    pdf * cos_theta.abs() / (dist * dist)
}
