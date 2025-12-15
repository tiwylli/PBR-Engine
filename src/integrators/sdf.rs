use std::sync::Arc;

use crate::{
    materials::Material,
    ray::Ray,
    scene::Scene,
    sdf::{RaymarchHit, RaymarchSettings, RaymarchStatus, SDFObject, raymarch},
    shapes::Intersection,
};

/// Combined representation of whichever surface (analytic or SDF) was selected for shading.
pub enum SurfaceHit<'scene> {
    Analytic(Intersection<'scene>),
    Implicit(SDFSurfaceHit),
}

/// Metadata captured when an SDF surface wins the depth test.
#[derive(Clone)]
pub struct SDFSurfaceHit {
    /// Handle back to the SDF object for gradient/bounds queries.
    pub sdf: Arc<dyn SDFObject>,
    /// Raw result returned by the marching routine.
    pub raymarch: RaymarchHit,
    /// Material bound to the implicit surface.
    pub material: Arc<dyn Material>,
}

/// Iterate over all registered SDF objects and return the closest successful hit, if any.
#[must_use]
pub fn gather_sdf_hit(
    ray: &Ray,
    scene: &Scene,
    settings: RaymarchSettings,
) -> Option<SDFSurfaceHit> {
    let mut best_hit: Option<SDFSurfaceHit> = None;
    if scene.sdf_objects.is_empty() {
        return None;
    }

    let mut test_candidate = |sdf: &Arc<dyn SDFObject>| {
        let per_object_settings = sdf.custom_settings().unwrap_or(settings);
        let result = raymarch(ray, sdf.as_ref(), per_object_settings);
        if matches!(result.status, RaymarchStatus::Hit)
            && let Some(hit) = result.hit
            && let Some(material) = hit.material.clone()
        {
            let candidate = SDFSurfaceHit {
                sdf: Arc::clone(sdf),
                raymarch: hit,
                material,
            };
            let replace = match &best_hit {
                Some(current_best) => candidate.raymarch.t < current_best.raymarch.t,
                None => true,
            };
            if replace {
                best_hit = Some(candidate);
            }
        }
    };

    if let Some(bvh) = &scene.sdf_bvh {
        bvh.traverse(ray, &mut test_candidate);
    } else {
        for sdf in &scene.sdf_objects {
            test_candidate(sdf);
        }
    }
    best_hit
}

/// Evaluate analytic and implicit geometry and return both candidates for the caller to arbitrate.
#[must_use]
pub fn collect_surface_hits<'scene>(
    ray: &Ray,
    scene: &'scene Scene,
    settings: RaymarchSettings,
) -> SurfaceSelection<'scene> {
    let analytic_hit = scene.root.hit(ray);
    let sdf_hit = gather_sdf_hit(ray, scene, settings);

    SurfaceSelection {
        analytic_hit,
        sdf_hit,
    }
}

/// Convenience struct bundling the competing surface candidates for downstream selection logic.
pub struct SurfaceSelection<'scene> {
    pub analytic_hit: Option<Intersection<'scene>>,
    pub sdf_hit: Option<SDFSurfaceHit>,
}

impl<'scene> SurfaceSelection<'scene> {
    /// Helper for discriminating between misses and hits.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.analytic_hit.is_none() && self.sdf_hit.is_none()
    }

    /// Consume the selection and return whichever surface is closest along the ray.
    #[must_use]
    pub fn into_nearest(self) -> Option<SurfaceHit<'scene>> {
        match (self.analytic_hit, self.sdf_hit) {
            (Some(analytic), Some(sdf)) => {
                if sdf.raymarch.t < analytic.t {
                    Some(SurfaceHit::Implicit(sdf))
                } else {
                    Some(SurfaceHit::Analytic(analytic))
                }
            }
            (Some(analytic), None) => Some(SurfaceHit::Analytic(analytic)),
            (None, Some(sdf)) => Some(SurfaceHit::Implicit(sdf)),
            (None, None) => None,
        }
    }

    /// Provide read-only access to the analytic hit when present.
    #[must_use]
    pub const fn analytic(&self) -> Option<&Intersection<'scene>> {
        self.analytic_hit.as_ref()
    }

    /// Provide read-only access to the implicit hit when present.
    #[must_use]
    pub const fn sdf(&self) -> Option<&SDFSurfaceHit> {
        self.sdf_hit.as_ref()
    }
}
