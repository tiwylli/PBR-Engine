#![allow(clippy::all)]
#![allow(warnings)]

use std::{collections::HashMap, sync::Arc};

use cgmath::{EuclideanSpace, InnerSpace, Zero};
use clap::Parser;
use log::{error, info};
use render::{
    Result,
    image::image_save,
    json::{json_to_f64, json_to_vec3},
    materials::json_to_material,
    ray::Ray,
    samplers::independent::Independent,
    shapes::{Shape, json_to_shape, triangle::Triangle},
    utils::generate_histogram,
    vec::{Point3, Vec2, Vec3},
};
use tinyjson::JsonValue;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// fichier de test
    #[arg(short, long)]
    input: String,

    /// fichier de sortie
    #[arg(short, long, default_value = "out")]
    output: String,
}

fn main() -> Result<()> {
    // Lecture de la ligne de commande
    let args = Args::parse();
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Read the json file
    info!("Reading json file");
    let contents = std::fs::read_to_string(&args.input).expect("Impossible le fichier de scene");

    info!("Parsing json file");
    let json: JsonValue = contents
        .parse()
        .map_err(|err| render::Error::Other(Box::new(err)))?;
    let json: &HashMap<String, JsonValue> = json.get().unwrap();

    // Create sampler
    let mut sampler = Box::new(Independent::new(1));

    if json.contains_key("material") {
        // Create material
        info!("Creating material");
        let material = json_to_material(json["material"].get().unwrap());

        // Create directions
        info!("Creating directions");
        let direction_json: &Vec<JsonValue> = json["directions"].get().unwrap();
        let directions = direction_json
            .into_iter()
            .map(|dir_json| {
                let dir_json: &HashMap<String, JsonValue> = dir_json.get().unwrap();
                let dir_name: String = dir_json["name"].get::<String>().unwrap().clone();
                if dir_json.contains_key("wo") {
                    (dir_name, json_to_vec3(dir_json, "wo", Vec3::zero()))
                } else if dir_json.contains_key("theta") || dir_json.contains_key("phi") {
                    let theta = json_to_f64(dir_json, "theta", 0.0).to_radians();
                    let phi = json_to_f64(dir_json, "phi", 0.0).to_radians();
                    (
                        dir_name,
                        Vec3::new(
                            theta.sin() * phi.cos(),
                            theta.sin() * phi.sin(),
                            theta.cos(),
                        ),
                    )
                } else {
                    panic!("Direction must have wo or theta and phi")
                }
            })
            .collect::<Vec<_>>();

        for (name, dir) in &directions {
            info!("{}: {:?}", name, dir);

            let lambda_pdf =
                |wi: &Vec3| -> f64 { material.pdf(dir, wi, &Vec2::zero(), &Point3::origin()) };
            let lambda_sample = |sample: &Vec2| -> Vec3 {
                if let Some(res) = material.sample(dir, &Vec2::zero(), &Point3::origin(), sample) {
                    res.wi
                } else {
                    Vec3::zero()
                }
            };

            let (pdf, hist, diff) =
                generate_histogram(&lambda_pdf, &lambda_sample, 50, sampler.as_mut());
            image_save(format!("{}-{}-hist.png", args.output, name).as_str(), &hist).unwrap();
            image_save(format!("{}-{}-pdf.png", args.output, name).as_str(), &pdf).unwrap();
            image_save(format!("{}-{}-diff.png", args.output, name).as_str(), &diff).unwrap();
        }
    } else if json.contains_key("shapes") {
        // Create dummy material
        let material = {
            let json: JsonValue = r#"{
                "type" : "diffuse_light"
             }"#
            .parse()
            .unwrap();
            json_to_material(json.get().unwrap())
        };

        // Create shape
        info!("Creating shape");
        let mut shape_refs = {
            let json: &Vec<JsonValue> = json["shapes"].get().unwrap();
            json.into_iter()
                .map(|shape_json| {
                    let shape_json: &HashMap<String, JsonValue> = shape_json.get().unwrap();
                    json_to_shape(shape_json, material.clone())
                })
                .collect::<Vec<_>>()
        };
        let shape = if shape_refs.len() == 1 {
            match shape_refs.pop().unwrap() {
                render::shapes::JsonShape::Shape(s) => s,
                render::shapes::JsonShape::Mesh(Some(m)) => {
                    // Convert mesh to triangle
                    let m = Arc::new(m);
                    let mut root: Box<dyn Shape> =
                        Box::new(render::shapes::shape_group::ShapeGroup::default());
                    for face_id in 0..m.face_positions_idx.len() {
                        root.add_shape(Box::new(Triangle::new(face_id, m.clone())))
                    }
                    root
                }
                _ => panic!("invalid shape"),
            }
        } else {
            let mut root = render::shapes::shape_group::ShapeGroup::default();
            for shape_ref in shape_refs {
                match shape_ref {
                    render::shapes::JsonShape::Shape(s) => root.add_shape(s),
                    render::shapes::JsonShape::Mesh(Some(m)) => {
                        // Convert mesh to triangle
                        let m = Arc::new(m);
                        for face_id in 0..m.face_positions_idx.len() {
                            root.add_shape(Box::new(Triangle::new(face_id, m.clone())))
                        }
                    }
                    _ => panic!("invalid shape"),
                }
            }
            Box::new(root)
        };

        // Define the PDF
        let lambda_pdf = |wi: &Vec3| -> f64 {
            let r = Ray::new(&Point3::new(0.0, 0.0, 0.0), wi);
            if let Some(its) = shape.hit(&r) {
                shape.pdf_direct(its.shape, &Point3::new(0.0, 0.0, 0.0), &its.p, &its.n)
            } else {
                0.0
            }
        };

        // Define the sample
        let lambda_sample = |sample: &Vec2| -> Vec3 {
            let visible = |p0: &Point3, p1: &Point3| -> bool {
                // Caclul de la direction entre p1 et p0
                let d = p1 - p0;
                let dist = d.magnitude();
                let d = d / dist;

                // Prise en compte de tmin et tmax
                let dist = dist - render::constants::RAY_EPS * 2.0;

                // Verification si on intersecte un objet entre p0 et p1
                let r = Ray::new(p0, &d).with_distance_max(dist);
                shape.hit(&r).is_none()
            };

            let (ps, _) = shape.sample_direct(&Point3::new(0.0, 0.0, 0.0), sample);
            let d = (ps.y - Point3::new(0.0, 0.0, 0.0)).normalize();
            // Verification si on intersecte un objet entre p0 et p1
            if visible(&Point3::new(0.0, 0.0, 0.0), &ps.y) {
                d
            } else {
                Vec3::zero()
            }
        };

        let (pdf, hist, diff) =
            generate_histogram(&lambda_pdf, &lambda_sample, 50, sampler.as_mut());
        image_save(format!("{}-hist.png", args.output).as_str(), &hist).unwrap();
        image_save(format!("{}-pdf.png", args.output).as_str(), &pdf).unwrap();
        image_save(format!("{}-diff.png", args.output).as_str(), &diff).unwrap();
    } else {
        error!("No material or shape in json file");
    }

    Ok(())
}
