use std::sync::Arc;

use cgmath::{Array, ElementWise, InnerSpace, Zero};

use crate::{
    constants::RAY_EPS,
    materials::Material,
    ray::Ray,
    samplers::Sampler,
    scene::Scene,
    sdf::{RaymarchSettings, apply_surface_bias},
    vec::{Color3, Frame, Point3, Vec2, Vec3},
};

use super::sdf::{SurfaceHit, collect_surface_hits};

pub struct SurfaceContext<'scene> {
    pub position: Point3,
    pub normal: Vec3,
    pub uv: Vec2,
    material: MaterialSlot<'scene>,
    is_sdf: bool,
}

impl<'scene> SurfaceContext<'scene> {
    #[must_use]
    pub fn from_hit(hit: SurfaceHit<'scene>) -> Self {
        match hit {
            SurfaceHit::Analytic(its) => SurfaceContext {
                position: its.p,
                normal: its.n.normalize(),
                uv: its.uv,
                material: MaterialSlot::Borrowed(its.material),
                is_sdf: false,
            },
            SurfaceHit::Implicit(sdf_hit) => {
                let mut n = sdf_hit.raymarch.normal;
                if n.magnitude2() <= f64::EPSILON {
                    n = Vec3::new(0.0, 1.0, 0.0);
                } else {
                    n = n.normalize();
                }

                SurfaceContext {
                    position: sdf_hit.raymarch.position,
                    normal: n,
                    uv: Vec2::zero(),
                    material: MaterialSlot::Owned(sdf_hit.material.clone()),
                    is_sdf: true,
                }
            }
        }
    }

    #[must_use]
    pub fn material(&self) -> &dyn Material {
        self.material.get()
    }
    #[must_use]
    pub fn spawn_origin(&self, settings: RaymarchSettings) -> Point3 {
        if self.is_sdf {
            apply_surface_bias(self.position, self.normal, &settings)
        } else {
            self.position
        }
    }
}

enum MaterialSlot<'scene> {
    Borrowed(&'scene dyn Material),
    Owned(Arc<dyn Material>),
}

impl MaterialSlot<'_> {
    fn get(&self) -> &dyn Material {
        match self {
            MaterialSlot::Borrowed(m) => *m,
            MaterialSlot::Owned(m) => m.as_ref(),
        }
    }
}
#[must_use]
pub fn surface_hit_has_emission(hit: &SurfaceHit) -> bool {
    match hit {
        SurfaceHit::Analytic(its) => its.material.have_emission(),
        SurfaceHit::Implicit(sdf_hit) => sdf_hit.material.have_emission(),
    }
}

#[must_use]
pub fn surface_hit_emission(hit: &SurfaceHit, incoming_world: &Vec3) -> Color3 {
    match hit {
        SurfaceHit::Analytic(its) => {
            let frame = Frame::new(&its.n);
            let wo = frame.to_local(incoming_world);
            its.material.emission(&wo, &its.uv, &its.p)
        }
        SurfaceHit::Implicit(sdf_hit) => {
            let mut n = sdf_hit.raymarch.normal;
            if n.magnitude2() <= f64::EPSILON {
                n = Vec3::new(0.0, 1.0, 0.0);
            } else {
                n = n.normalize();
            }
            let frame = Frame::new(&n);
            let wo = frame.to_local(incoming_world);
            let dummy_uv = Vec2::zero();
            sdf_hit
                .material
                .emission(&wo, &dummy_uv, &sdf_hit.raymarch.position)
        }
    }
}

#[must_use]
pub fn direct_emitter_mis_surface(
    surface: &SurfaceContext<'_>,
    scene: &Scene,
    sampler: &mut dyn Sampler,
    frame: &Frame,
    wo: &Vec3,
    settings: RaymarchSettings,
) -> Color3 {
    if !scene.has_analytic_emitters() {
        return Color3::from_value(0.0);
    }

    if surface.material().have_delta() {
        return Color3::from_value(0.0);
    }

    let (ps, shape) = scene
        .root
        .sample_direct(&surface.position, &sampler.next2d());
    if ps.pdf <= 0.0 {
        return Color3::from_value(0.0);
    }

    let origin = surface.spawn_origin(settings);
    let to_light = ps.y - origin;
    let dist = to_light.magnitude();
    if dist <= RAY_EPS * 2.0 {
        return Color3::from_value(0.0);
    }
    let dir_world = to_light / dist;
    let max_t = RAY_EPS.mul_add(-2.0, dist);

    let shadow_ray = Ray::new(&origin, &dir_world).with_distance_max(max_t);
    let occluder = collect_surface_hits(&shadow_ray, scene, settings).into_nearest();
    if occluder.is_some() {
        return Color3::from_value(0.0);
    }

    let wi_local = frame.to_local(&dir_world);
    let fbsdf_cos = surface
        .material()
        .evaluate(wo, &wi_local, &surface.uv, &surface.position);

    let f_light = Frame::new(&ps.n);
    let wo_light = f_light.to_local(&(-dir_world));
    let le = shape
        .material()
        .emission(&wo_light, &surface.uv, &surface.position);

    let pdf_emitter = ps.pdf;
    let pdf_bsdf = surface
        .material()
        .pdf(wo, &wi_local, &surface.uv, &surface.position);
    let denom = pdf_bsdf + pdf_emitter;
    if denom <= 0.0 {
        return Color3::from_value(0.0);
    }

    let mis_w = pdf_emitter / denom;
    (fbsdf_cos / pdf_emitter).mul_element_wise(le) * mis_w
}
