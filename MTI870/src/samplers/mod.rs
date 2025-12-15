use std::collections::HashMap;

use tinyjson::JsonValue;

use crate::{json::json_to_f64, vec::Vec2};

pub trait Sampler: Send + Sync {
    fn next(&mut self) -> f64;
    fn next2d(&mut self) -> Vec2;
    fn clone_box(&mut self) -> Box<dyn Sampler>;

    fn nb_samples(&self) -> usize;
    fn set_nb_samples(&mut self, nspp: usize);
}

pub mod independent;

pub fn json_to_sampler(json: &HashMap<String, JsonValue>) -> Box<dyn Sampler> {
    if !json.contains_key("type") {
        panic!(
            "Need to specify 'type' variable to create the sampler.\n{:?}.",
            json
        );
    }

    let nspp = json_to_f64(json, "samples", 1.0) as usize;

    let t: String = json["type"].clone().try_into().unwrap();
    match t.as_str() {
        "independent" => Box::new(independent::Independent::new(nspp)),
        _ => panic!("Unknow shape type: {}", t),
    }
}
