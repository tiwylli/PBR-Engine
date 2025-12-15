use std::{cell::RefCell, collections::HashMap};

use tinyjson::JsonValue;

#[macro_use]
extern crate quick_error;

#[macro_export]
macro_rules! function {
    () => {{
        fn f() {}
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
    use crate::Real;
    pub const M_PI: Real = 3.141_592_653_589_793;
    pub const INV_PI: Real = 0.318_309_886_183_790_7;
    pub const INV_TWOPI: Real = 0.159_154_943_091_895_35;
    pub const INV_FOURPI: Real = 0.079_577_471_545_947_67;
    pub const SQRT_TWO: Real = 1.414_213_562_373_095_1;
    pub const INV_SQRT_TWO: Real = 0.707_106_781_186_547_6;
    pub const RAY_EPS: Real = 0.0001;
}

/// Convert radians to degrees
pub fn rad2deg(value: Real) -> Real {
    value * (180.0 / constants::M_PI)
}

/// Convert degrees to radians
pub fn deg2rad(value: Real) -> Real {
    value * (constants::M_PI / 180.0)
}

/// Compute the fresnel equation
/// cos_theta_i: cosinus of the angle between the normal and the incident ray
/// eta_i: index of refraction of the incident medium
/// eta_t: index of refraction of the transmitted medium
/// Return the fresnel equation
pub fn fresnel(cos_theta_i: Real, eta_i: Real, eta_t: Real) -> Real {
    let (eta_i, eta_t, cos_theta_i) = if eta_i == eta_t {
        return 0.0;
    } else if cos_theta_i < 0.0 {
        (eta_t, eta_i, -cos_theta_i)
    } else {
        (eta_i, eta_t, cos_theta_i)
    };

    let eta = eta_i / eta_t;
    let sin_theta_t_sqr = eta * eta * (1.0 - cos_theta_i * cos_theta_i);

    if sin_theta_t_sqr > 1.0 {
        return 1.0;
    }

    let cos_theta_t = (1.0 - sin_theta_t_sqr).sqrt();

    let rs =
        (eta_i * cos_theta_i - eta_t * cos_theta_t) / (eta_i * cos_theta_i + eta_t * cos_theta_t);
    let rp =
        (eta_t * cos_theta_i - eta_i * cos_theta_t) / (eta_t * cos_theta_i + eta_i * cos_theta_t);

    (rs * rs + rp * rp) / 2.0
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
pub fn modulo(a: Real, b: Real) -> Real {
    let r = a % b;
    if r < 0.0 {
        r + b
    } else {
        r
    }
}

// For statistics computations
thread_local! {
    pub static NUMBER_INTERSECTIONS: RefCell<usize> = RefCell::new(0);
    pub static NUMBER_TRACED_RAYS: RefCell<usize> = RefCell::new(0);
}


pub mod array2d;
pub mod camera;
pub mod fileresolver;
pub mod image;
pub mod integrators;
pub mod json;
pub mod materials;
pub mod ray;
pub mod samplers;
pub mod scene;
pub mod sdf;
pub mod shapes;
pub mod transform;
pub mod utils;
pub mod vec;
pub mod aabb;
pub mod texture;
