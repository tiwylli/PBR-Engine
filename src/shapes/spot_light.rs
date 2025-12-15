use std::{collections::HashMap, sync::Arc};

use cgmath::{EuclideanSpace, InnerSpace, Zero};

use tinyjson::JsonValue;

use crate::{
    Real,
    aabb::AABB,
    json::json_to_vec3,
    materials::Material,
    ray::Ray,
    shapes::{EmitterSample, Intersection, Shape},
    transform::MyTransform,
    vec::{Point3, Vec2, Vec3},
};

pub struct SpotLight {
    position: Point3,
    direction: Vec3,
    material: Arc<dyn Material>,
}

impl SpotLight {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        transform: &MyTransform,
        material: Arc<dyn Material>,
    ) -> Self {
        let mut position = Point3::from_vec(json_to_vec3(json, "position", Vec3::zero()));
        let mut direction = json_to_vec3(json, "direction", -Vec3::unit_y()).normalize();

        position = transform.point(&position);
        direction = transform.vector(&direction);

        Self {
            position,
            direction,
            material,
        }
    }
}

impl Shape for SpotLight {
    fn hit<'a>(&'a self, _r: &Ray) -> Option<Intersection<'a>> {
        None
    }

    fn sample_direct(&self, p: &Point3, _sample: &Vec2) -> (EmitterSample, &dyn Shape) {
        let y = self.position;
        let n = self.direction;
        (
            EmitterSample {
                y,
                n,
                pdf: self.pdf_direct(self, p, &y, &n),
            },
            self,
        )
    }

    fn pdf_direct(&self, _shape: &dyn Shape, _p: &Point3, _y: &Point3, _n: &Vec3) -> Real {
        1.0
    }

    fn add_shape(&mut self, _shape: Box<dyn Shape>) {}

    fn build(&mut self) {}

    fn aabb(&self) -> AABB {
        AABB::from_points(self.position, self.position)
    }

    fn material(&self) -> &dyn Material {
        self.material.as_ref()
    }
}
