use std::{collections::HashMap, sync::Arc};

use tinyjson::JsonValue;

use crate::{
    aabb::{merge_aabb, AABB},
    materials::Material,
    transform::MyTransform,
    vec::Point3,
};

use super::{
    raymarch::RaymarchSettings,
    sdf_object::{json_to_sdf_object, parse_object_settings, SDFObject},
};

fn load_children(
    json: &HashMap<String, JsonValue>,
    key: &str,
    materials: &HashMap<String, Arc<dyn Material>>,
) -> Vec<Arc<dyn SDFObject>> {
    let list = json
        .get(key)
        .unwrap_or_else(|| panic!("SDF operator `{}` field missing", key));
    let list: &Vec<JsonValue> = list
        .get()
        .expect("SDF operator children must be an array of objects");

    list.iter()
        .map(|entry| {
            let data: &HashMap<String, JsonValue> =
                entry.get().expect("SDF operator child must be an object");
            json_to_sdf_object(data, materials)
        })
        .collect()
}

pub struct SdfUnion {
    children: Vec<Arc<dyn SDFObject>>,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
}

impl SdfUnion {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let children = load_children(json, "children", materials);
        if children.is_empty() {
            panic!("SDF union requires at least one child");
        }

        let bounds = children
            .iter()
            .skip(1)
            .fold(children[0].world_bounds(), |acc, child| {
                merge_aabb(&acc, &child.world_bounds())
            });

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("SDF union `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{}` for SDF union", name)),
            )
        });

        let settings = parse_object_settings(json);

        Self {
            children,
            bounds,
            material,
            settings,
        }
    }
}

impl SDFObject for SdfUnion {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        self.children
            .iter()
            .map(|child| child.signed_distance(world_p))
            .fold(f64::INFINITY, f64::min)
    }

    fn object_to_world(&self) -> &MyTransform {
        // Operators work directly in world space, so we forward the first child transform.
        self.children[0].object_to_world()
    }

    fn world_bounds(&self) -> AABB {
        self.bounds.clone()
    }

    fn material(&self) -> Option<Arc<dyn Material>> {
        if let Some(mat) = &self.material {
            return Some(Arc::clone(mat));
        }
        self.children.first().and_then(|child| child.material())
    }

    fn custom_settings(&self) -> Option<RaymarchSettings> {
        self.settings
    }
}

pub struct SdfIntersection {
    children: Vec<Arc<dyn SDFObject>>,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
}

impl SdfIntersection {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let children = load_children(json, "children", materials);
        if children.is_empty() {
            panic!("SDF intersection requires at least one child");
        }

        // Start from the first child's bounds as a simple approximation.
        let bounds = children[0].world_bounds();

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("SDF intersection `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{}` for SDF intersection", name)),
            )
        });

        let settings = parse_object_settings(json);

        Self {
            children,
            bounds,
            material,
            settings,
        }
    }
}

impl SDFObject for SdfIntersection {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        self.children
            .iter()
            .map(|child| child.signed_distance(world_p))
            .fold(f64::NEG_INFINITY, f64::max)
    }

    fn object_to_world(&self) -> &MyTransform {
        self.children[0].object_to_world()
    }

    fn world_bounds(&self) -> AABB {
        self.bounds.clone()
    }

    fn material(&self) -> Option<Arc<dyn Material>> {
        if let Some(mat) = &self.material {
            return Some(Arc::clone(mat));
        }
        self.children.first().and_then(|child| child.material())
    }

    fn custom_settings(&self) -> Option<RaymarchSettings> {
        self.settings
    }
}

pub struct SdfDifference {
    left: Arc<dyn SDFObject>,
    right: Arc<dyn SDFObject>,
    bounds: AABB,
    material: Option<Arc<dyn Material>>,
    settings: Option<RaymarchSettings>,
}

impl SdfDifference {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        materials: &HashMap<String, Arc<dyn Material>>,
    ) -> Self {
        let parts = load_children(json, "children", materials);
        if parts.len() != 2 {
            panic!("SDF difference expects exactly two children");
        }

        let bounds = parts[0].world_bounds();

        let material = json.get("material").map(|value| {
            let name: &str = value
                .get::<String>()
                .expect("SDF difference `material` must be a string");
            Arc::clone(
                materials
                    .get(name)
                    .unwrap_or_else(|| panic!("Unknown material `{}` for SDF difference", name)),
            )
        });

        let settings = parse_object_settings(json);

        Self {
            left: Arc::clone(&parts[0]),
            right: Arc::clone(&parts[1]),
            bounds,
            material,
            settings,
        }
    }
}

impl SDFObject for SdfDifference {
    fn signed_distance(&self, world_p: Point3) -> f64 {
        let a = self.left.signed_distance(world_p);
        let b = self.right.signed_distance(world_p);
        a.max(-b)
    }

    fn object_to_world(&self) -> &MyTransform {
        self.left.object_to_world()
    }

    fn world_bounds(&self) -> AABB {
        self.bounds.clone()
    }

    fn material(&self) -> Option<Arc<dyn Material>> {
        if let Some(mat) = &self.material {
            return Some(Arc::clone(mat));
        }
        self.left.material()
    }

    fn custom_settings(&self) -> Option<RaymarchSettings> {
        self.settings
    }
}
