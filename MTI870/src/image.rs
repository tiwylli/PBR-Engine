use std::path::Path;

use crate::array2d::Array2d;
use crate::vec::{to_linear_rgb, to_srgb, Color3};
use crate::Real;
use image::io::Reader as ImageReader;
use image::{DynamicImage, GenericImage};

pub fn image_load(path: &str, gamma: bool) -> crate::Result<Array2d<Color3>> {
    let output_ext = match std::path::Path::new(path).extension() {
        None => panic!("No file extension provided"),
        Some(x) => std::ffi::OsStr::to_str(x).expect("Issue to unpack the file"),
    };

    match output_ext {
        "exr" => {
            let img = ImageReader::open(path).map_err(|e| crate::Error::Other(Box::new(e)))?;
            let img = img.decode().map_err(|e| crate::Error::Other(Box::new(e)))?;
            let img = img.to_rgb32f();
            let mut res = Array2d::with_size(img.width(), img.height(), Color3::new(0.0, 0.0, 0.0));
            for x in 0..res.size_x() {
                for y in 0..res.size_y() {
                    let p = img.get_pixel(x, y);
                    *res.at_mut(x, y) = Color3::new(p[0] as Real, p[1] as Real, p[2] as Real);
                }
            }
            Ok(res)
        }
        _ => {
            let img = ImageReader::open(path).map_err(|e| crate::Error::Other(Box::new(e)))?;
            let img = img.decode().map_err(|e| crate::Error::Other(Box::new(e)))?;
            let img = img.to_rgb8();
            let mut res = Array2d::with_size(img.width(), img.height(), Color3::new(0.0, 0.0, 0.0));
            for x in 0..res.size_x() {
                for y in 0..res.size_y() {
                    let p = img.get_pixel(x, y);
                    if gamma {
                        *res.at_mut(x, y) = to_linear_rgb(&Color3::new(
                            p[0] as Real / 255.0,
                            p[1] as Real / 255.0,
                            p[2] as Real / 255.0,
                        ));
                    } else {
                        *res.at_mut(x, y) = Color3::new(
                            p[0] as Real / 255.0,
                            p[1] as Real / 255.0,
                            p[2] as Real / 255.0,
                        );
                    }
                }
            }
            Ok(res)
        }
    }
}

pub fn image_save(path: &str, data: &Array2d<Color3>) -> crate::Result<()> {
    let output_ext = match std::path::Path::new(path).extension() {
        None => panic!("No file extension provided"),
        Some(x) => std::ffi::OsStr::to_str(x).expect("Issue to unpack the file"),
    };

    match output_ext {
        "exr" => {
            let mut image_hdr = DynamicImage::new_rgba32f(data.size_x(), data.size_y()).to_rgb32f();
            for x in 0..data.size_x() {
                for y in 0..data.size_y() {
                    let p = data.at(x, y);
                    image_hdr.put_pixel(x, y, image::Rgb([p[0] as f32, p[1] as f32, p[2] as f32]));
                }
            }
            image_hdr
                .save(Path::new(path))
                .map_err(|e| crate::Error::Other(Box::new(e)))?;
            Ok(())
        }
        _ => {
            let mut image_ldr = DynamicImage::new_rgb8(data.size_x(), data.size_y());
            for x in 0..data.size_x() {
                for y in 0..data.size_y() {
                    let p = to_srgb(data.at(x, y));
                    image_ldr.put_pixel(
                        x,
                        y,
                        image::Rgba([
                            (p[0] * 255.0) as u8,
                            (p[1] * 255.0) as u8,
                            (p[2] * 255.0) as u8,
                            255,
                        ]),
                    );
                }
            }
            image_ldr
                .save(Path::new(path))
                .map_err(|e| crate::Error::Other(Box::new(e)))?;
            Ok(())
        }
    }
}
