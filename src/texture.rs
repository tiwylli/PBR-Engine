use std::collections::HashMap;

use cgmath::{Array, ElementWise, Zero};
use rand::{SeedableRng, seq::SliceRandom};
use rand_chacha::ChaCha8Rng;
use tinyjson::JsonValue;

use crate::{
    array2d::Array2d,
    fileresolver::FILE_RESOLVER,
    image::image_load,
    json::{json_to_bool, json_to_f64, json_to_vec2, json_to_vec3},
    modulo,
    transform::{MyTransform, json_to_transform},
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
            Self::Constant(c) => *c,
            Self::TextureMap {
                values,
                scale,
                uv_scale,
                uv_offset,
            } => {
                let uv = uv.mul_element_wise(*uv_scale) + *uv_offset;
                let uv = Vec2::new(modulo(uv.x, 1.0), modulo(uv.y, 1.0));

                // Compute bilinear interpolation
                let x = uv.x.mul_add(f64::from(values.size_x()) - 1.0, -0.5);
                let y = uv.y.mul_add(f64::from(values.size_y()) - 1.0, -0.5);
                let x0 = x.floor() as u32;
                let x1 = x.ceil() as u32;
                let y0 = y.floor() as u32;
                let y1 = y.ceil() as u32;
                let x = x - f64::from(x0);
                let y = y - f64::from(y0);
                let v00 = *values.at(x0, y0);
                let v01 = *values.at(x0, y1);
                let v10 = *values.at(x1, y0);
                let v11 = *values.at(x1, y1);
                let v0 = v00 * (1.0 - x) + v10 * x;
                let v1 = v01 * (1.0 - x) + v11 * x;
                let v = v0 * (1.0 - y) + v1 * y;

                v * (*scale)
            }
            Self::Checkerboard2d {
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
            Self::Checkerboard3d {
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
                .resolve(std::path::Path::new(&s));
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
                        .resolve(std::path::Path::new(&values));
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
                // code from and chadgpt helped translate and scaffold https://rtouti.github.io/graphics/perlin-noise-algorithm
                "perlin_noise" => {
                    let width = json_to_f64(o, "width", 512.0).max(1.0) as u32;
                    let height = json_to_f64(o, "height", 512.0).max(1.0) as u32;
                    let frequency = json_to_f64(o, "frequency", 4.0).max(0.000_1);
                    let octaves = json_to_f64(o, "octaves", 4.0).max(1.0) as u32;
                    let persistence = json_to_f64(o, "persistence", 0.5).clamp(0.0, 1.0);
                    let lacunarity = json_to_f64(o, "lacunarity", 2.0).max(0.000_1);
                    let contrast = json_to_f64(o, "contrast", 1.0).max(0.0);
                    let offset = json_to_vec2(o, "offset", Vec2::new(0.0, 0.0));
                    let color1 = json_to_vec3(o, "color1", Color3::new(0.25, 0.25, 0.23));
                    let color2 = json_to_vec3(o, "color2", Color3::new(0.65, 0.6, 0.55));
                    let uv_scale = json_to_vec2(o, "uv_scale", Vec2::new(1.0, 1.0));
                    let uv_offset = json_to_vec2(o, "uv_offset", Vec2::new(0.0, 0.0));
                    let seed = o
                        .get("seed")
                        .and_then(|value| value.get::<f64>())
                        .map(|v| *v as u64);

                    let values = generate_perlin_texture(
                        width,
                        height,
                        frequency,
                        octaves,
                        persistence,
                        lacunarity,
                        offset,
                        color1,
                        color2,
                        contrast,
                        seed,
                    );

                    Texture::TextureMap {
                        values,
                        scale: 1.0,
                        uv_scale,
                        uv_offset,
                    }
                }
                _ => panic!("Invalid texture type: {type_object}"),
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

#[allow(clippy::too_many_arguments)]
fn generate_perlin_texture(
    width: u32,
    height: u32,
    base_frequency: f64,
    octaves: u32,
    persistence: f64,
    lacunarity: f64,
    offset: Vec2,
    color1: Color3,
    color2: Color3,
    contrast: f64,
    seed: Option<u64>,
) -> Array2d<Color3> {
    let mut values = Array2d::with_size(width, height, Color3::zero());
    let perm = build_permutation(seed);
    let width_f = f64::from(width.max(1));
    let height_f = f64::from(height.max(1));
    let contrast = contrast.max(1.0e-4);

    for y in 0..height {
        for x in 0..width {
            let u = f64::from(x) / width_f;
            let v = f64::from(y) / height_f;
            let noise = perlin_fbm(
                u + offset.x,
                v + offset.y,
                &perm,
                base_frequency,
                octaves,
                persistence,
                lacunarity,
            );
            let mapped = noise.mul_add(0.5, 0.5).clamp(0.0, 1.0).powf(contrast);
            let color = color1 * (1.0 - mapped) + color2 * mapped;
            *values.at_mut(x, y) = color;
        }
    }

    values
}

fn perlin_fbm(
    x: f64,
    y: f64,
    perm: &[u8; 256],
    base_frequency: f64,
    octaves: u32,
    persistence: f64,
    lacunarity: f64,
) -> f64 {
    let mut amplitude = 1.0;
    let mut frequency = base_frequency.max(1.0e-4);
    let mut sum = 0.0;
    let mut weight = 0.0;

    for _ in 0..octaves {
        sum += amplitude * perlin2d(x * frequency, y * frequency, perm);
        weight += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    if weight > 0.0 { sum / weight } else { 0.0 }
}

#[allow(clippy::many_single_char_names)]
fn perlin2d(x: f64, y: f64, perm: &[u8; 256]) -> f64 {
    let xi = (x.floor() as i32 & 255) as usize;
    let yi = (y.floor() as i32 & 255) as usize;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let u = fade(xf);
    let v = fade(yf);

    let xi1 = (xi + 1) & 255;
    let yi1 = (yi + 1) & 255;

    let a = perm[xi] as usize;
    let aa = perm[(a + yi) & 255];
    let ab = perm[(a + yi1) & 255];

    let b = perm[xi1] as usize;
    let ba = perm[(b + yi) & 255];
    let bb = perm[(b + yi1) & 255];

    let x1 = lerp(u, grad(aa, xf, yf), grad(ba, xf - 1.0, yf));
    let x2 = lerp(u, grad(ab, xf, yf - 1.0), grad(bb, xf - 1.0, yf - 1.0));

    lerp(v, x1, x2)
}

fn fade(t: f64) -> f64 {
    t * t * t * t.mul_add(t.mul_add(6.0, -15.0), 10.0)
}

fn lerp(t: f64, a: f64, b: f64) -> f64 {
    t.mul_add(b - a, a)
}

fn grad(hash: u8, x: f64, y: f64) -> f64 {
    match hash & 0x3 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        _ => -x - y,
    }
}

fn build_permutation(seed: Option<u64>) -> [u8; 256] {
    let mut perm = [0u8; 256];
    if let Some(seed) = seed {
        let mut values: Vec<u8> = (0..=255).collect();
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        values.shuffle(&mut rng);
        perm.copy_from_slice(&values[..256]);
    } else {
        perm.copy_from_slice(&DEFAULT_PERM);
    }
    perm
}

const DEFAULT_PERM: [u8; 256] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30, 69,
    142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219,
    203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
    74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122, 60, 211, 133, 230,
    220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25, 63, 161, 1, 216, 80, 73, 209, 76,
    132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173,
    186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212, 207, 206,
    59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213, 119, 248, 152, 2, 44, 154, 163,
    70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232,
    178, 185, 112, 104, 218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162,
    241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157, 184, 84, 204,
    176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141,
    128, 195, 78, 66, 215, 61, 156, 180,
];

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
                .resolve(std::path::Path::new(&s));
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
                        .resolve(std::path::Path::new(&values));
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
                _ => panic!("Invalid texture type: {type_object}"),
            }
        }
        JsonValue::Number(v) => Texture::Constant(*v),
        _ => panic!("Invalid texture"),
    }
}
