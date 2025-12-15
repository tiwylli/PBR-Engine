use std::collections::HashMap;

use cgmath::{Array, ElementWise};
use tinyjson::JsonValue;

use crate::{
    array2d::Array2d,
    fileresolver::FILE_RESOLVER,
    image::image_load,
    json::{json_to_bool, json_to_f64, json_to_vec2, json_to_vec3},
    modulo,
    transform::{json_to_transform, MyTransform},
    vec::{Color3, Point3, Vec2},
};

pub enum Texture<T: Clone> {
    Constant(T),
    TextureMap {
        values: Array2d<T>,
        scale: f64,
        uv_scale: Vec2,
        uv_offset: Vec2,
    },
    Checkerboard2d {
        color1: T,
        color2: T,
        uv_scale: Vec2,
        uv_offset: Vec2,
    },
    Checkerboard3d {
        color1: T,
        color2: T,
        transform: MyTransform,
    },
}

impl<T: Clone + std::ops::Mul<f64, Output = T> + std::ops::Add<T, Output = T> + Copy> Texture<T> {
    pub fn get(&self, uv: &Vec2, p: &Point3) -> T {
        match self {
            Texture::Constant(c) => *c,
            Texture::TextureMap {
                values,
                scale,
                uv_scale,
                uv_offset,
            } => {
                let uv = uv.mul_element_wise(*uv_scale) + *uv_offset;
                let uv = Vec2::new(modulo(uv.x, 1.0), modulo(uv.y, 1.0));

                // Compute bilinear interpolation
                let x = uv.x * (values.size_x() as f64 - 1.0) - 0.5;
                let y = uv.y * (values.size_y() as f64 - 1.0) - 0.5;
                let x0 = x.floor() as u32;
                let x1 = x.ceil() as u32;
                let y0 = y.floor() as u32;
                let y1 = y.ceil() as u32;
                let x = x - x0 as f64;
                let y = y - y0 as f64;
                let v00 = *values.at(x0, y0);
                let v01 = *values.at(x0, y1);
                let v10 = *values.at(x1, y0);
                let v11 = *values.at(x1, y1);
                let v0 = v00 * (1.0 - x) + v10 * x;
                let v1 = v01 * (1.0 - x) + v11 * x;
                let v = v0 * (1.0 - y) + v1 * y;

                v * (*scale)
            }
            Texture::Checkerboard2d {
                color1,
                color2,
                uv_scale,
                uv_offset,
            } => {
                let uv = uv.mul_element_wise(*uv_scale) + *uv_offset;
                if (uv.x as i32 + uv.y as i32) % 2 == 0 {
                    *color1
                } else {
                    *color2
                }
            }
            Texture::Checkerboard3d {
                color1,
                color2,
                transform,
            } => {
                let p = transform.point(p);
                if (p.x as i32 + p.y as i32 + p.z as i32) % 2 == 0 {
                    *color1
                } else {
                    *color2
                }
            }
        }
    }
}

pub fn json_to_texture(
    json: &HashMap<String, JsonValue>,
    name: &str,
    default: Color3,
) -> Texture<Color3> {
    if !json.contains_key(name) {
        return Texture::Constant(default);
    }

    match &json[name] {
        JsonValue::String(s) => {
            let s = FILE_RESOLVER
                .lock()
                .unwrap()
                .resolve(&std::path::Path::new(&s));
            let mut values = image_load(s.to_str().unwrap(), true).unwrap();
            values.flip_vertically();
            Texture::TextureMap {
                values,
                scale: 1.0,
                uv_scale: Vec2::new(1.0, 1.0),
                uv_offset: Vec2::new(0.0, 0.0),
            }
        }
        JsonValue::Object(o) => {
            let type_object: &String = o["type"].get().unwrap();
            match type_object.as_str() {
                "constant" => {
                    let values: Vec<f64> = o["value"]
                        .get::<Vec<JsonValue>>()
                        .unwrap()
                        .iter()
                        .map(|v| *v.get().unwrap())
                        .collect();
                    Texture::Constant(Color3::new(values[0], values[1], values[2]))
                }
                "texture" => {
                    // Read the texture file
                    let values: &String = o["filename"].get().unwrap();
                    let values = FILE_RESOLVER
                        .lock()
                        .unwrap()
                        .resolve(&std::path::Path::new(&values));
                    let gamma = json_to_bool(o, "gamma", true);
                    dbg!(values.to_str().unwrap());
                    let mut values = image_load(values.to_str().unwrap(), gamma).unwrap();

                    // Read the other parameters
                    let scale = if o.contains_key("scale") {
                        o["scale"].get().unwrap()
                    } else {
                        &1.0
                    };
                    let uv_scale = json_to_vec2(o, "uv_scale", Vec2::new(1.0, 1.0));
                    let uv_offset = json_to_vec2(o, "uv_offset", Vec2::new(0.0, 0.0));

                    // Flip the image if needed
                    let vflip = json_to_bool(o, "vflip", true);
                    if vflip {
                        values.flip_vertically();
                    }
                    Texture::TextureMap {
                        values,
                        scale: *scale,
                        uv_scale,
                        uv_offset,
                    }
                }
                "checkerboard2d" => {
                    let color1 = json_to_vec3(o, "color1", Color3::new(0.0, 0.0, 0.0));
                    let color2 = json_to_vec3(o, "color2", Color3::new(1.0, 1.0, 1.0));
                    let uv_scale = json_to_vec2(o, "uv_scale", Vec2::new(1.0, 1.0));
                    let uv_offset = json_to_vec2(o, "uv_offset", Vec2::new(0.0, 0.0));
                    Texture::Checkerboard2d {
                        color1,
                        color2,
                        uv_scale,
                        uv_offset,
                    }
                }
                "checkerboard3d" => {
                    let color1 = json_to_vec3(o, "color1", Color3::new(0.0, 0.0, 0.0));
                    let color2 = json_to_vec3(o, "color2", Color3::new(1.0, 1.0, 1.0));
                    let transform = json_to_transform(o, "transform");
                    Texture::Checkerboard3d {
                        color1,
                        color2,
                        transform,
                    }
                }
                _ => panic!("Invalid texture type: {}", type_object),
            }
        }
        JsonValue::Number(v) => Texture::Constant(Color3::from_value(*v)),
        JsonValue::Array(v) => Texture::Constant(Color3::new(
            *v[0].get().unwrap(),
            *v[1].get().unwrap(),
            *v[2].get().unwrap(),
        )),
        _ => panic!("Invalid texture"),
    }
}

/// Convert a json value to a texture of f64
pub fn json_to_texture_float(
    json: &HashMap<String, JsonValue>,
    name: &str,
    default: f64,
) -> Texture<f64> {
    if !json.contains_key(name) {
        return Texture::Constant(default);
    }

    // Convert a texture of Color3 to a texture of f64
    let convert_avg = |values_rgb: Array2d<Color3>| {
        let mut values = Array2d::with_size(values_rgb.size_x(), values_rgb.size_y(), 0.0);
        for x in 0..values.size_x() {
            for y in 0..values.size_y() {
                let v = values_rgb.at(x, y);
                *values.at_mut(x, y) = (v.x + v.y + v.z) / 3.0;
            }
        }
        values
    };

    match &json[name] {
        JsonValue::String(s) => {
            let s = FILE_RESOLVER
                .lock()
                .unwrap()
                .resolve(&std::path::Path::new(&s));
            let mut values = image_load(s.to_str().unwrap(), true).unwrap();
            values.flip_vertically();
            Texture::TextureMap {
                values: convert_avg(values),
                scale: 1.0,
                uv_scale: Vec2::new(1.0, 1.0),
                uv_offset: Vec2::new(0.0, 0.0),
            }
        }
        JsonValue::Object(o) => {
            let type_object: &String = o["type"].get().unwrap();
            match type_object.as_str() {
                "constant" => {
                    let value = json_to_f64(json, "value", 0.0);
                    Texture::Constant(value)
                }
                "texture" => {
                    // Read the texture file
                    let values: &String = o["filename"].get().unwrap();
                    let values = FILE_RESOLVER
                        .lock()
                        .unwrap()
                        .resolve(&std::path::Path::new(&values));
                    let gamma = json_to_bool(o, "gamma", true);
                    let mut values = image_load(values.to_str().unwrap(), gamma).unwrap();

                    // Read the other parameters
                    let scale = if o.contains_key("scale") {
                        o["scale"].get().unwrap()
                    } else {
                        &1.0
                    };
                    let uv_scale = json_to_vec2(o, "uv_scale", Vec2::new(1.0, 1.0));
                    let uv_offset = json_to_vec2(o, "uv_offset", Vec2::new(0.0, 0.0));

                    // Flip the image if needed
                    let vflip = json_to_bool(o, "vflip", true);
                    if vflip {
                        values.flip_vertically();
                    }
                    Texture::TextureMap {
                        values: convert_avg(values),
                        scale: *scale,
                        uv_scale,
                        uv_offset,
                    }
                }
                "checkerboard2d" => {
                    let color1 = json_to_f64(o, "color1", 0.0);
                    let color2 = json_to_f64(o, "color2", 1.0);
                    let uv_scale = json_to_vec2(o, "uv_scale", Vec2::new(1.0, 1.0));
                    let uv_offset = json_to_vec2(o, "uv_offset", Vec2::new(0.0, 0.0));
                    Texture::Checkerboard2d {
                        color1,
                        color2,
                        uv_scale,
                        uv_offset,
                    }
                }
                "checkerboard3d" => {
                    let color1 = json_to_f64(o, "color1", 0.0);
                    let color2 = json_to_f64(o, "color2", 1.0);
                    let transform = json_to_transform(o, "transform");
                    Texture::Checkerboard3d {
                        color1,
                        color2,
                        transform,
                    }
                }
                _ => panic!("Invalid texture type: {}", type_object),
            }
        }
        JsonValue::Number(v) => Texture::Constant(*v),
        _ => panic!("Invalid texture"),
    }
}
