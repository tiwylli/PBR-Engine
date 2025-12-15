use std::{cell::RefCell, collections::HashMap};

use tinyjson::JsonValue;

#[macro_use]
extern crate quick_error;

#[macro_export]
macro_rules! function {
    () => {{
        const fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);

        // Find and cut the rest of the path
        match &name[..name.len() - 3].rfind(':') {
            Some(pos) => &name[pos + 1..name.len() - 3],
            None => &name[..name.len() - 3],
        }
    }};
}

#[macro_export]
macro_rules! votrecodeici {
    ($($arg:tt)+) => {
        static COUNTER: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
        let value = COUNTER.load(std::sync::atomic::Ordering::Relaxed);
        if value == 0 {
            COUNTER.store(10, std::sync::atomic::Ordering::Relaxed);
            log::error!("{}() not implemented in {}:{}\n\t msg: {}", function!(), file!(), line!(), format_args!($($arg)+))
        }
    };
}

pub type Real = f64;

pub mod constants {
    use std::f64;

    use crate::Real;
    pub const M_PI: Real = f64::consts::PI;
    pub const INV_PI: Real = f64::consts::FRAC_1_PI;
    pub const INV_TWOPI: Real = 0.159_154_943_091_895_35;
    pub const INV_FOURPI: Real = 0.079_577_471_545_947_67;
    pub const SQRT_TWO: Real = f64::consts::SQRT_2;
    pub const INV_SQRT_TWO: Real = f64::consts::FRAC_1_SQRT_2;
    pub const RAY_EPS: Real = 0.0001;
}

/// Convert radians to degrees
#[must_use]
pub fn rad2deg(value: Real) -> Real {
    value * (180.0 / constants::M_PI)
}

/// Convert degrees to radians
#[must_use]
pub fn deg2rad(value: Real) -> Real {
    value * (constants::M_PI / 180.0)
}

/// Compute the fresnel equation
/// `cos_theta_i`: cosinus of the angle between the normal and the incident ray
/// `eta_i`: index of refraction of the incident medium
/// `eta_t`: index of refraction of the transmitted medium
/// Return the fresnel equation
#[must_use]
pub fn fresnel(cos_theta_i: Real, eta_i: Real, eta_t: Real) -> Real {
    let (eta_i, eta_t, cos_theta_i) = if (eta_i - eta_t).abs() < 1e-6 {
        return 0.0;
    } else if cos_theta_i < 0.0 {
        (eta_t, eta_i, -cos_theta_i)
    } else {
        (eta_i, eta_t, cos_theta_i)
    };

    let eta = eta_i / eta_t;
    let sin_theta_t_sqr = eta * eta * cos_theta_i.mul_add(-cos_theta_i, 1.0);

    if sin_theta_t_sqr > 1.0 {
        return 1.0;
    }

    let cos_theta_t = (1.0 - sin_theta_t_sqr).sqrt();

    let rs = eta_i.mul_add(cos_theta_i, -(eta_t * cos_theta_t))
        / eta_i.mul_add(cos_theta_i, eta_t * cos_theta_t);
    let rp = eta_t.mul_add(cos_theta_i, -(eta_i * cos_theta_t))
        / eta_t.mul_add(cos_theta_i, eta_i * cos_theta_t);

    f64::midpoint(rs * rs, rp * rp)
}

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        /// InvalidType
        InvalidType(name: String) {
            display("Unknown reference (name: {:?})", name)
        }
        /// Attribute not found
        AttribNotFound(name: String, additional_info: String) {
            display("Impossible to found {} attribute when parsing {}", name, additional_info)
        }
        /// Uncovered case
        UncoveredCase(name: &'static str, json: HashMap<String, JsonValue>) {
            display("Impossible to construct {}, case is not covered {:?}", name, json)
        }
        /// Uncovered case
        UncoveredCaseJson(name: &'static str, json: JsonValue) {
            display("Impossible to construct {}, JSON object case non unhandled {:?}", name, json)
        }
        /// Wrong dimension
        WrongDimensionJson(name: &'static str, json: Vec<JsonValue>, dim_expected: usize) {
            display("Impossible to construct {}, Wrong dimension provided (expected: {}, got {}) {:?}", name, dim_expected, json.len(), json)
        }
        /// Fuse two json failed
        FailedPatchJson(json: JsonValue, target: JsonValue) {
            display("Impossible to patch this JSON part {:?} to this one {:?}", json, target)
        }
        /// Other error
        Other(err: Box<dyn std::error::Error>) {
            source(&**err)
        }
    }
}
pub type Result<T> = std::result::Result<T, Error>;

/// Modulo function, always return a positive number
/// a: number to modulo
/// b: modulo value
/// Return a modulo b
#[must_use]
pub fn modulo(a: Real, b: Real) -> Real {
    let r = a % b;
    if r < 0.0 { r + b } else { r }
}

// For statistics computations
thread_local! {
    pub static NUMBER_INTERSECTIONS: RefCell<usize> = const { RefCell::new(0) };
    pub static NUMBER_TRACED_RAYS: RefCell<usize> = const { RefCell::new(0) };
}

pub mod aabb;
pub mod array2d;
pub mod camera;
pub mod fileresolver;
pub mod image;
pub mod integrators;
pub mod json;
pub mod materials;
pub mod medium;
pub mod ray;
pub mod samplers;
pub mod scene;
pub mod sdf_shapes;
pub use sdf_shapes as sdf;
#[cfg(feature = "oidn")]
pub mod denoise;
pub mod shapes;
pub mod texture;
pub mod transform;
pub mod utils;
pub mod vec;
