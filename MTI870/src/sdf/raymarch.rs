use std::{collections::HashMap, sync::Arc};

use cgmath::InnerSpace;
use tinyjson::JsonValue;

use crate::{
    aabb::AABB,
    materials::Material,
    ray::Ray,
    vec::{Point3, Vec3},
};

use super::sdf_object::SDFObject;

/// User-tunable parameters that control ray marching behaviour.
#[derive(Debug, Clone, Copy)]
pub struct RaymarchSettings {
    /// Hard cap on the number of sphere-tracing steps.
    pub max_steps: u32,
    /// Threshold at which we consider the surface "hit".
    pub hit_epsilon: f64,
    /// Epsilon used when sampling SDF values for normal estimation and secondary rays.
    pub normal_epsilon: f64,
    /// Fraction applied to raw SDF step lengths to prevent overshooting narrow features.
    pub step_clamp: f64,
    /// Maximum distance to march along the ray (global scene bound or camera clip distance).
    pub max_travel_distance: f64,
}

impl Default for RaymarchSettings {
    fn default() -> Self {
        Self {
            max_steps: 128,
            hit_epsilon: 1.0e-4,
            normal_epsilon: 5.0e-4,
            step_clamp: 0.95,
            max_travel_distance: 1.0e5,
        }
    }
}

impl RaymarchSettings {
    /// Merge JSON overrides into the current settings.
    pub fn with_overrides(&self, json: &HashMap<String, JsonValue>) -> RaymarchSettings {
        let mut merged = *self;

        if let Some(v) = json.get("max_steps").and_then(|v| v.get::<f64>()) {
            merged.max_steps = v.max(1.0) as u32;
        }
        if let Some(v) = json.get("hit_epsilon").and_then(|v| v.get::<f64>()) {
            merged.hit_epsilon = v.max(1.0e-8);
        }
        if let Some(v) = json.get("normal_epsilon").and_then(|v| v.get::<f64>()) {
            merged.normal_epsilon = v.max(1.0e-8);
        }
        if let Some(v) = json.get("step_clamp").and_then(|v| v.get::<f64>()) {
            merged.step_clamp = v.clamp(1.0e-3, 1.0);
        }
        if let Some(v) = json.get("max_travel_distance").and_then(|v| v.get::<f64>()) {
            merged.max_travel_distance = v.max(0.0);
        }

        merged
    }
}

/// Detailed payload returned once a surface hit is confirmed.
#[derive(Clone)]
pub struct RaymarchHit {
    /// Ray parameter at the hit.
    pub t: f64,
    /// World-space impact point.
    pub position: Point3,
    /// Surface normal derived from SDF gradient estimation.
    pub normal: Vec3,
    /// Material selected for shading.
    pub material: Option<Arc<dyn Material>>,
    /// Iteration count taken to converge.
    pub steps: u32,
}

/// Status of the marching attempt.
#[derive(Debug, Clone, Copy)]
pub enum RaymarchStatus {
    /// No surface found within the configured travel distance.
    Miss,
    /// Surface detected under the hit epsilon.
    Hit,
    /// Step budget exhausted before convergence.
    MaxStepsExceeded,
    /// Ray escaped the object's bounding volume without intersecting.
    EscapedBounds,
}

/// Aggregated result returned by the marching routine.
#[derive(Clone)]
pub struct RaymarchResult {
    pub status: RaymarchStatus,
    pub hit: Option<RaymarchHit>,
}

impl RaymarchResult {
    /// Helper for miss cases.
    pub fn miss(status: RaymarchStatus) -> Self {
        Self { status, hit: None }
    }

    /// Helper for successful hits.
    pub fn success(hit: RaymarchHit) -> Self {
        Self {
            status: RaymarchStatus::Hit,
            hit: Some(hit),
        }
    }
}

/// Ray march an implicit surface using sphere tracing heuristics.
pub fn raymarch(ray: &Ray, sdf_obj: &dyn SDFObject, settings: RaymarchSettings) -> RaymarchResult {
    let bounds = sdf_obj.world_bounds();

    let (mut entry_t, mut exit_t) = match ray_bounds_interval(ray, &bounds) {
        Some(interval) => interval,
        None => return RaymarchResult::miss(RaymarchStatus::EscapedBounds),
    };

    let distance_cap = sdf_obj
        .max_raymarch_distance()
        .unwrap_or(settings.max_travel_distance)
        .min(settings.max_travel_distance);

    entry_t = entry_t.max(ray.tmin);
    exit_t = exit_t.min(ray.tmax).min(distance_cap);

    if exit_t <= entry_t {
        return RaymarchResult::miss(RaymarchStatus::EscapedBounds);
    }

    let mut t = entry_t;
    let mut steps = 0_u32;

    while steps < settings.max_steps && t <= exit_t {
        let position = ray.point_at(t);
        let distance = sdf_obj.signed_distance(position);

        if !distance.is_finite() {
            return RaymarchResult::miss(RaymarchStatus::MaxStepsExceeded);
        }

        if distance.abs() <= settings.hit_epsilon {
            let normal = compute_normal(position, sdf_obj, settings.normal_epsilon);
            let material = sdf_obj.material();
            return RaymarchResult::success(RaymarchHit {
                t,
                position,
                normal,
                material,
                steps,
            });
        }

        let mut step = distance * settings.step_clamp;
        if step <= 0.0 {
            step = settings.hit_epsilon;
        }

        const MIN_ADVANCE: f64 = 1.0e-7;
        step = step.max(MIN_ADVANCE);

        t += step;
        steps += 1;

        if t > exit_t {
            return RaymarchResult::miss(RaymarchStatus::Miss);
        }
    }

    if steps >= settings.max_steps {
        RaymarchResult::miss(RaymarchStatus::MaxStepsExceeded)
    } else {
        RaymarchResult::miss(RaymarchStatus::Miss)
    }
}

/// Estimate the normal direction using finite differences on the SDF.
pub fn compute_normal(world_p: Point3, sdf_obj: &dyn SDFObject, eps: f64) -> Vec3 {
    let epsilon = if eps > 0.0 { eps } else { 1.0e-4 };
    let offset_x = Vec3::new(epsilon, 0.0, 0.0);
    let offset_y = Vec3::new(0.0, epsilon, 0.0);
    let offset_z = Vec3::new(0.0, 0.0, epsilon);

    let dx =
        sdf_obj.signed_distance(world_p + offset_x) - sdf_obj.signed_distance(world_p - offset_x);
    let dy =
        sdf_obj.signed_distance(world_p + offset_y) - sdf_obj.signed_distance(world_p - offset_y);
    let dz =
        sdf_obj.signed_distance(world_p + offset_z) - sdf_obj.signed_distance(world_p - offset_z);

    let gradient = Vec3::new(dx, dy, dz);
    if gradient.magnitude2() <= std::f64::EPSILON {
        Vec3::unit_y()
    } else {
        gradient.normalize()
    }
}

/// Offset a surface hit point along its normal to combat self-shadowing.
pub fn apply_surface_bias(position: Point3, normal: Vec3, settings: &RaymarchSettings) -> Point3 {
    if normal.magnitude2() <= std::f64::EPSILON {
        position
    } else {
        position + normal.normalize() * settings.normal_epsilon
    }
}

fn ray_bounds_interval(ray: &Ray, bounds: &AABB) -> Option<(f64, f64)> {
    let mut t_min = ray.tmin;
    let mut t_max = ray.tmax;

    for axis in 0..3 {
        let origin = ray.o[axis];
        let direction = ray.d[axis];
        let min_bound = bounds.min[axis];
        let max_bound = bounds.max[axis];

        if direction.abs() < std::f64::EPSILON {
            if origin < min_bound || origin > max_bound {
                return None;
            }
            continue;
        }

        let inv_dir = 1.0 / direction;
        let mut t0 = (min_bound - origin) * inv_dir;
        let mut t1 = (max_bound - origin) * inv_dir;
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }

        t_min = t_min.max(t0);
        t_max = t_max.min(t1);

        if t_max < t_min {
            return None;
        }
    }

    Some((t_min, t_max))
}
