use crate::{
    samplers::Sampler,
    vec::{Color3, Vec3},
};
use cgmath::{Array, ElementWise, InnerSpace};
use serde::{Deserialize, Serialize};

pub trait Medium: Send + Sync {
    fn transmittance(&self, distance: f64) -> Color3;
    fn sample(&self, max_distance: f64, sampler: &mut dyn Sampler) -> MediumSample;
    fn phase_function(&self) -> &PhaseFunction;
    fn sigma_a(&self) -> Vec3;
    fn sigma_s(&self) -> Vec3;
    fn density(&self) -> f64;
}

#[derive(Debug, Clone, Copy)]
pub enum MediumSample {
    Scatter {
        t: f64,
        weight: Color3,
        tr: Color3,
        pdf: f64,
    },
    None {
        tr: Color3,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HenyeyGreenstein {
    pub g: f64,
}

impl HenyeyGreenstein {
    // PBRT's implementation
    #[must_use]
    pub fn phase_func(&self, w: &Vec3, wp: &Vec3) -> f64 {
        let cos_theta = w.dot(*wp);
        let denom = (2.0 * self.g).mul_add(cos_theta, self.g.mul_add(self.g, 1.0));
        self.g.mul_add(-self.g, 1.0) / (4.0 * std::f64::consts::PI * denom * denom.sqrt())
    }

    #[must_use]
    pub fn sample_p(&self, w: &Vec3, u: (f64, f64)) -> Vec3 {
        let cos_theta = if self.g.abs() < 1e-3 {
            2.0f64.mul_add(-u.0, 1.0)
        } else {
            let sqr_term = self.g.mul_add(-self.g, 1.0) / (2.0 * self.g).mul_add(u.0, 1.0 - self.g);
            (self.g.mul_add(self.g, 1.0) - sqr_term * sqr_term) / (2.0 * self.g)
        };

        let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
        let phi = 2.0 * std::f64::consts::PI * u.1;

        let v1 = if w.x.abs() > w.y.abs() {
            Vec3::new(-w.z, 0.0, w.x) / w.x.hypot(w.z)
        } else {
            Vec3::new(0.0, w.z, -w.y) / w.y.hypot(w.z)
        };
        let v2 = w.cross(v1);

        v1 * sin_theta * phi.cos() + v2 * sin_theta * phi.sin() + *w * cos_theta
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
pub enum PhaseFunction {
    HenyeyGreenstein(HenyeyGreenstein),
    Isotropic,
}

impl PhaseFunction {
    #[must_use]
    pub fn phase_func(&self, w: &Vec3, wp: &Vec3) -> f64 {
        match self {
            Self::HenyeyGreenstein(hg) => hg.phase_func(w, wp),
            Self::Isotropic => 1.0 / (4.0 * std::f64::consts::PI),
        }
    }

    #[must_use]
    pub fn sample_p(&self, w: &Vec3, u: (f64, f64)) -> Vec3 {
        match self {
            Self::HenyeyGreenstein(hg) => hg.sample_p(w, u),
            Self::Isotropic => {
                let z = 2.0f64.mul_add(-u.0, 1.0);
                let r = (1.0 - z * z).max(0.0).sqrt();
                let phi = 2.0 * std::f64::consts::PI * u.1;
                let x = r * phi.cos();
                let y = r * phi.sin();
                Vec3::new(x, y, z)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomogeneousMedium {
    pub sigma_a: Vec3,
    pub sigma_s: Vec3,
    pub density: f64,
    pub phase: PhaseFunction,
}

impl Medium for HomogeneousMedium {
    fn transmittance(&self, distance: f64) -> Color3 {
        let sigma_t = (self.sigma_a + self.sigma_s) * self.density;
        transmittance_from_sigma_t(sigma_t, distance.max(0.0))
    }

    fn sample(&self, max_distance: f64, sampler: &mut dyn Sampler) -> MediumSample {
        if max_distance <= 0.0 {
            return MediumSample::None {
                tr: Color3::from_value(1.0),
            };
        }

        let sigma_t = (self.sigma_a + self.sigma_s).mul_element_wise(self.density);
        let sigma_s = self.sigma_s * self.density;
        let channel = (sampler.next() * 3.0).floor().clamp(0.0, 2.0) as usize;
        let sigma_t_channel = sigma_t[channel];

        if sigma_t_channel <= 0.0 {
            return MediumSample::None {
                tr: transmittance_from_sigma_t(sigma_t, max_distance),
            };
        }

        let sampled_dist = -sampler.next().ln() / sigma_t_channel;
        if sampled_dist < max_distance {
            let tr = transmittance_from_sigma_t(sigma_t, sampled_dist);
            let pdf = (tr.x + tr.y + tr.z) * (1.0 / 3.0);
            if pdf <= 0.0 {
                return MediumSample::None { tr };
            }
            let weight = tr.mul_element_wise(sigma_s / pdf);
            MediumSample::Scatter {
                t: sampled_dist,
                weight,
                tr,
                pdf,
            }
        } else {
            MediumSample::None {
                tr: transmittance_from_sigma_t(sigma_t, max_distance),
            }
        }
    }

    fn phase_function(&self) -> &PhaseFunction {
        &self.phase
    }

    fn sigma_a(&self) -> Vec3 {
        self.sigma_a
    }

    fn sigma_s(&self) -> Vec3 {
        self.sigma_s
    }

    fn density(&self) -> f64 {
        self.density
    }
}

#[must_use]
pub fn transmittance_from_sigma_t(sigma_t: Vec3, distance: f64) -> Color3 {
    Color3::new(
        (-sigma_t.x * distance).exp(),
        (-sigma_t.y * distance).exp(),
        (-sigma_t.z * distance).exp(),
    )
}
