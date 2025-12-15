use std::{collections::HashMap, sync::Arc};

use cgmath::Zero;
use tinyjson::JsonValue;

use crate::{
    aabb::AABB,
    json::{json_to_string, json_to_vec3},
    materials::Material,
    transform::MyTransform,
    vec::{Point3, Vec3},
};

use super::{
    fbm_noise::SdfFbmNoise,
    fractal::{SdfJulia, SdfMandelbulb},
    mandelmorph::SdfMandelMorph,
    menger::SdfMenger,
    noise_sphere::SdfNoiseSphere,
    operators::{SdfDifference, SdfIntersection, SdfUnion},
    plane::SdfPlane,
    raymarch::RaymarchSettings,
    round_box::SdfRoundBox,
    sine_sphere::SdfSineSphere,
    sphere::SdfSphere,
    voronoi_noise::SdfVoronoiNoise,
};

/// Trait implemented by implicit surfaces that expose a signed distance function.
pub trait SDFObject: Send + Sync {
    /// Evaluate the signed distance at a world-space point.
    fn signed_distance(&self, world_p: Point3) -> f64;

    /// Transform mapping the object into world space (identity if already expressed in world space).
    fn object_to_world(&self) -> &MyTransform;

    /// Conservative world-space bounding box for early ray rejection.
    fn world_bounds(&self) -> AABB;

    /// Material handle used for shading when the surface is hit.
    ///
    /// Returning `None` allows debug shading or fallbacks in the integrator.
    fn material(&self) -> Option<Arc<dyn Material>>;

    /// Optional per-object override of global marching settings.
    fn custom_settings(&self) -> Option<RaymarchSettings> {
        None
    }

    /// Optional per-object clamp on travel distance (e.g., fractal bailout radius).
    fn max_raymarch_distance(&self) -> Option<f64> {
        None
    }
}

/// Parse a `bounds` object from JSON and convert it to an [`AABB`].
pub fn json_to_bounds(json: &HashMap<String, JsonValue>) -> Option<AABB> {
    let bounds_value = json.get("bounds")?;
    let bounds_obj: &HashMap<_, _> = bounds_value
        .get()
        .expect("SDF bounds must be an object with `min`/`max` fields");

    let min_vec = json_to_vec3(bounds_obj, "min", Vec3::zero());
    let max_vec = json_to_vec3(bounds_obj, "max", Vec3::zero());

    Some(AABB::from_points(
        Point3::new(min_vec.x, min_vec.y, min_vec.z),
        Point3::new(max_vec.x, max_vec.y, max_vec.z),
    ))
}

/// Convenience helper shared by SDF object loaders to resolve the optional
/// per-object marching settings.
pub(super) fn parse_object_settings(json: &HashMap<String, JsonValue>) -> Option<RaymarchSettings> {
    let settings_value = json.get("settings")?;
    let settings_obj: &HashMap<_, _> = settings_value
        .get()
        .expect("SDF `settings` must be an object with numeric overrides");
    Some(RaymarchSettings::default().with_overrides(settings_obj))
}

/// Create an [`Arc<dyn SDFObject>`] from the JSON representation found inside `Scene`.
pub fn json_to_sdf_object(
    json: &HashMap<String, JsonValue>,
    materials: &HashMap<String, Arc<dyn Material>>,
) -> Arc<dyn SDFObject> {
    if !json.contains_key("type") {
        panic!(
            "SDF object definition is missing a `type` field: {:?}",
            json
        );
    }

    let sdf_type = json_to_string(json, "type", "");
    match sdf_type.as_str() {
        "sdf_sphere" => Arc::new(SdfSphere::from_json(json, materials)),
        "sdf_plane" => Arc::new(SdfPlane::from_json(json, materials)),
        "sdf_mandelbulb" => Arc::new(SdfMandelbulb::from_json(json, materials)),
        "sdf_julia" => Arc::new(SdfJulia::from_json(json, materials)),
        "sdf_mandelmorph" => Arc::new(SdfMandelMorph::from_json(json, materials)),
        "sdf_menger" => Arc::new(SdfMenger::from_json(json, materials)),
        "sdf_noise_sphere" => Arc::new(SdfNoiseSphere::from_json(json, materials)),
        "sdf_voronoi_noise" => Arc::new(SdfVoronoiNoise::from_json(json, materials)),
        "sdf_fbm_noise" => Arc::new(SdfFbmNoise::from_json(json, materials)),
        "sdf_round_box" => Arc::new(SdfRoundBox::from_json(json, materials)),
        "sdf_sine_sphere" => Arc::new(SdfSineSphere::from_json(json, materials)),
        "sdf_union" => Arc::new(SdfUnion::from_json(json, materials)),
        "sdf_intersection" => Arc::new(SdfIntersection::from_json(json, materials)),
        "sdf_difference" => Arc::new(SdfDifference::from_json(json, materials)),
        _ => panic!("Unknown SDF object type `{}`", sdf_type),
    }
}
