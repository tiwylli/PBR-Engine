use std::{collections::HashMap, f64, fmt::Write, sync::Arc};

use cgmath::{ElementWise, InnerSpace, Zero};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::{info, warn};
use tinyjson::JsonValue;

use crate::{
    NUMBER_TRACED_RAYS, Real,
    array2d::Array2d,
    camera::CameraPerspective,
    image::image_load,
    integrators::{Integrator, json_to_integrator},
    json::{json_to_bool, json_to_medium, json_to_string, json_to_vec3},
    materials::{Material, json_to_material},
    medium::Medium,
    ray::Ray,
    samplers::{Sampler, independent::Independent, json_to_sampler},
    sdf::{SDFObject, SdfBvh, json_to_sdf_object},
    shapes::{
        EmitterSample, Intersection, Shape, bvh::BVH, json_to_shape, shape_group::ShapeGroup,
        triangle::Triangle,
    },
    vec::{Color3, Frame, Point3, Vec2, Vec3},
};

pub enum SceneBackground {
    Uniform(Color3),
    Texture(Array2d<Color3>),
}

pub struct Scene {
    pub root: Box<dyn Shape>,
    pub materials: HashMap<String, Arc<dyn Material>>,
    pub camera: CameraPerspective,
    pub background: SceneBackground,
    pub max_depth: usize,
    pub sdf_objects: Vec<Arc<dyn SDFObject>>,
    pub sdf_bvh: Option<SdfBvh>,
    pub medium: Option<Box<dyn Medium>>,
    has_analytic_emitters: bool,
    pub ignore_nans: bool,
}

impl Scene {
    #[must_use]
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
    ) -> (Self, Box<dyn Sampler>, Box<dyn Integrator>) {
        // Read the scene to create the camera, materials and shapes
        let background_color = json_to_vec3(json, "background", Vec3::zero());
        let background_tex = json_to_string(json, "background_texture", "");
        let background_tex = if background_tex.is_empty() {
            None
        } else {
            image_load(&background_tex, true).ok()
        };
        let background = background_tex.map_or(SceneBackground::Uniform(background_color), |tex| {
            SceneBackground::Texture(tex)
        });

        // Create camera
        let camera = CameraPerspective::from_json(json["camera"].get().unwrap());

        // Create sampler
        let sampler = json_to_sampler(json["sampler"].get().unwrap());

        // Create integrator
        let integrator = if json.contains_key("integrator") {
            json_to_integrator(json["integrator"].get().unwrap())
        } else {
            let s = r#"{
                "type" : "path"
            }"#;
            let s: JsonValue = s.parse().unwrap();
            json_to_integrator(s.get().unwrap())
        };

        // Create all the materials
        let materials = if json.contains_key("materials") {
            let jmats = &json["materials"];
            assert!(
                jmats.is_array(),
                "Materials needs to be specified as an array\n\t{jmats:?}"
            );

            let jmats: &Vec<JsonValue> = jmats.get().unwrap();
            jmats
                .iter()
                .map(|jmat| {
                    assert!(
                        jmat.is_object(),
                        "Material needs to be specified as object\n\t{jmat:?}"
                    );
                    let jmat: &HashMap<_, _> = jmat.get().unwrap();
                    assert!(
                        jmat.contains_key("name"),
                        "Materials need to have a name\n\t{jmat:?}"
                    );
                    let name: String = jmat["name"].get::<String>().unwrap().clone();
                    info!("Create material: {name}");

                    (name, json_to_material(jmat))
                })
                .collect()
        } else {
            HashMap::new()
        };

        let medium = if json.contains_key("medium") {
            let medium_json: &HashMap<String, JsonValue> = json["medium"].get().unwrap();
            Some(json_to_medium(medium_json).unwrap())
        } else {
            None
        };

        let mut root: Box<dyn Shape> = if json.contains_key("accelerator") {
            let json_accel = json["accelerator"].get().unwrap();
            let accel_type_str = json_to_string(json_accel, "type", "linear");
            match accel_type_str.as_str() {
                "linear" => Box::new(ShapeGroup::default()),
                "bvh" => Box::new(BVH::from_json(json_accel)),
                _ => {
                    warn!("Unknown accelerator type: {accel_type_str} -- use linear instead");
                    Box::new(ShapeGroup::default())
                }
            }
        } else {
            Box::new(ShapeGroup::default())
        };

        let mut has_analytic_emitters = false;
        if json.contains_key("shapes") {
            let jshapes = &json["shapes"];
            assert!(
                jshapes.is_array(),
                "Shapes needs to be specified as an array\n\t{jshapes:?}"
            );
            let jshapes: &Vec<_> = jshapes.get().unwrap();

            for jshape in jshapes {
                assert!(
                    jshape.is_object(),
                    "Shape needs to be specified as object\n\t{jshape:?}"
                );
                let jshape: &HashMap<_, _> = jshape.get().unwrap();

                assert!(
                    jshape.contains_key("material"),
                    "Shapes needs to be specified as an array\n\t{jshape:?}"
                );
                let material_name: String = jshape["material"].get::<String>().unwrap().clone();
                assert!(
                    materials.contains_key(&material_name),
                    "Impossible to found a material named: {material_name}"
                );

                let material = materials[&material_name].clone();
                if material.have_emission() {
                    has_analytic_emitters = true;
                }

                let s = json_to_shape(jshape, material);
                match s {
                    crate::shapes::JsonShape::Shape(s) => {
                        if s.material().have_emission() {
                            has_analytic_emitters = true;
                        }
                        root.add_shape(s);
                    }
                    crate::shapes::JsonShape::Mesh(m) => {
                        if let Some(m) = m {
                            // Convert mesh to triangle
                            let m = Arc::new(m);
                            for face_id in 0..m.face_positions_idx.len() {
                                root.add_shape(Box::new(Triangle::new(face_id, m.clone())));
                            }
                        }
                    }
                }
            }
        }

        let mut sdf_objects = Vec::new();
        if json.contains_key("sdf_objects") {
            let jsdfs = &json["sdf_objects"];
            assert!(
                jsdfs.is_array(),
                "`sdf_objects` needs to be specified as an array\n\t{jsdfs:?}"
            );
            let jsdfs: &Vec<JsonValue> = jsdfs.get().unwrap();
            for jsdf in jsdfs {
                assert!(
                    jsdf.is_object(),
                    "SDF object needs to be specified as object\n\t{jsdf:?}"
                );
                let jsdf: &HashMap<_, _> = jsdf.get().unwrap();
                sdf_objects.push(json_to_sdf_object(jsdf, &materials));
            }
        }

        info!("Build acceleration structure ...");
        let now = std::time::Instant::now();
        root.build();
        info!(
            "Acceleration structure build in {}s",
            now.elapsed().as_secs_f64()
        );

        let sdf_bvh = SdfBvh::build(&sdf_objects);

        let ignore_nans = json_to_bool(json, "ignore_nans", false);
        if ignore_nans {
            warn!("Ignoring NaN samples in render");
        }

        (
            Self {
                root,
                materials,
                camera,
                background,
                max_depth: 16,
                sdf_objects,
                sdf_bvh,
                medium,
                has_analytic_emitters,
                ignore_nans,
            },
            sampler,
            integrator,
        )
    }

    #[allow(
        clippy::missing_panics_doc,
        clippy::cast_precision_loss,
        clippy::literal_string_with_formatting_args
    )]
    pub fn render(&self, sampler: &mut dyn Sampler) -> Array2d<Color3> {
        let mut im = Array2d::with_size(
            self.camera.resolution.x,
            self.camera.resolution.y,
            Color3::zero(),
        );

        let progress = ProgressBar::new(u64::from(self.camera.resolution.x));
        progress.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{wide_bar}] {pos:>7}/{len:7} ({eta})",
            )
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn Write| {
                write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap();
            })
            .progress_chars("#>-"),
        );
        for x in 0..self.camera.resolution.x {
            for y in 0..self.camera.resolution.y {
                let color = (0..sampler.nb_samples())
                    .map(|_| {
                        let ray = self.camera.generate_ray(
                            &Vec2::new(
                                f64::from(x) + sampler.next(),
                                f64::from(y) + sampler.next(),
                            ),
                            sampler,
                        );
                        self.trace(ray, sampler, 0)
                    })
                    .sum::<Vec3>()
                    / sampler.nb_samples() as f64;
                *im.at_mut(x, y) = color;
            }

            progress.inc(1);
        }
        im
    }

    pub fn trace(&self, r: Ray, sampler: &mut dyn Sampler, depth: usize) -> Color3 {
        if depth >= self.max_depth {
            Color3::zero()
        } else if let Some(intersection) = self.hit(&r) {
            let frame = Frame::new(&intersection.n);
            let dir_world = -r.d;
            let dir_local = frame.to_local(&dir_world);
            intersection
                .material
                .sample(
                    &dir_local,
                    &intersection.uv,
                    &intersection.p,
                    &sampler.next2d(),
                )
                .map_or_else(
                    || {
                        intersection.material.emission(
                            &dir_local,
                            &intersection.uv,
                            &intersection.p,
                        )
                    },
                    |sampled_direction| {
                        let r = Ray::new(&intersection.p, &frame.to_world(&sampled_direction.wi));
                        let recursive_color = self.trace(r, sampler, depth + 1);
                        sampled_direction.weight.mul_element_wise(recursive_color)
                    },
                )
        } else {
            self.background(r.d)
        }
    }

    #[must_use]
    pub fn background(&self, d: Vec3) -> Color3 {
        match &self.background {
            SceneBackground::Uniform(color) => *color,
            SceneBackground::Texture(tex) => {
                let longitude = 0.5 + d.z.atan2(d.x) / (2.0 * f64::consts::PI);
                let latitude = 0.5 + (d.y / d.magnitude()).asin() / f64::consts::FRAC_PI_2;
                let u = f64::from(tex.size_x()) * longitude;
                let v = f64::from(tex.size_y()) * (1.0 - latitude);
                *tex.at(u as u32, v as u32)
            }
        }
    }

    #[must_use]
    pub fn hit<'a>(&'a self, r: &Ray) -> Option<Intersection<'a>> {
        NUMBER_TRACED_RAYS.with(|f| *f.borrow_mut() += 1);
        self.root.hit(r)
    }

    #[must_use]
    pub fn visible(&self, p0: &Point3, p1: &Point3) -> bool {
        // Calcul de la direction entre p1 et p0
        let d = p1 - p0;
        let dist = d.magnitude();
        let d = d / dist;
        // Prise en compte de tmin et tmax
        let dist = crate::constants::RAY_EPS.mul_add(-2.0, dist);
        // Vérifie s'il y a une intersection entre p0 et p1
        let r = Ray::new(p0, &d).with_distance_max(dist);
        self.hit(&r).is_none()
    }

    #[must_use]
    pub fn sample_direct(&self, p: &Point3, sample: &Vec2) -> (EmitterSample, &dyn Shape) {
        self.root.sample_direct(p, sample)
    }

    pub fn pdf_direct(&self, shape: &dyn Shape, p: &Point3, y: &Point3, n: &Vec3) -> Real {
        self.root.pdf_direct(shape, p, y, n)
    }

    #[must_use]
    pub const fn has_analytic_emitters(&self) -> bool {
        self.has_analytic_emitters
    }
}

/////////////////////////
// Example scene
fn create_sphere_scene() -> JsonValue {
    r#"{
    "camera": {
        "transform": {
            "o": [0, 0, 4]
        },
        "vfov": 45,
        "resolution": [640, 480]
    },
    "background": [
        1, 1, 1
    ],
    "sampler": {
        "type": "independent",
        "samples": 1
    },
    "materials": [
        {
            "name": "mat_sphere",
            "type": "diffuse",
            "albedo": [0.6, 0.4, 0.4]
        },
        {
            "name": "mat_plane",
            "type": "diffuse",
            "albedo": [0.75, 0.75, 0.75]
        }
    ],
    "shapes": [
        {
            "type": "sphere",
            "radius": 1,
            "material": "mat_sphere"
        }, 
        {
            "type": "quad",
            "transform": {
                "o": [
                    0, -1, 0
                ],
                "x": [
                    1, 0, 0
                ],
                "y": [
                    0, 0, -1
                ],
                "z": [0, 1, 0]
            },
            "size": 100,
            "material": "mat_plane"
        }
        ]
    }"#
    .parse()
    .unwrap()
}

/// Create the scene used in the class
fn create_three_sphere_class() -> JsonValue {
    r#"{
        "camera": {
            "transform": {
                "o": [0.7, 0, 0]
            },
            "vfov": 90,
            "resolution": [640, 480]
        },
        "background": [
            0, 0, 0
        ],
        "sampler": {
            "type": "independent",
            "samples": 1
        },
        "materials" : [
            {
                "name": "glass",
                "type": "dielectric",
                "eta_int": 1.5
            },
            {
                "name": "wall",
                "type": "diffuse"
            }, 
            {
                "name": "light",
                "type": "diffuse_light"
            },
            {
                "name": "red ball",
                "type": "metal", 
                "roughness": 0.1,
                "ks" : [0.9, 0.1, 0.1]
            },
            {
                "name" : "blue ball",
                "type" : "diffuse", 
                "albedo" : [0.1, 0.1, 0.9]
            }
        ],
        "shapes" : [
            {
                "type": "quad",
                "size": [100.0, 100.0], 
                "transform": [
                    {"angle": -90, "axis": [1, 0, 0]}, 
                    {"translate": [0.0, -1.0, 0.0]}
                ],
                "material": "wall"
            }, 
            {
                "type": "quad",
                "size": [100.0, 100.0], 
                "transform": [
                    {"translate": [0.0, 0.0, -10.0]}
                ],
                "material": "wall"
            }, 
            {
                "type": "quad",
                "size": [20.0, 20.0], 
                "transform": [
                    {"o": [0.0, 10.0, 0.0], "z": [0, -1, 0], "y" : [0, 0, 1]}
                ],
                "material": "light"
            }, 
            {
                "type" : "sphere",
                "transform" : {
                    "o" : [0.0, 0.0, -2.0]
                },
                "material" : "red ball"
            },
            {
                "type" : "sphere",
                "transform" : {
                    "o" : [1.8, -0.2, -2.2]
                },
                "radius" : 0.8,
                "material" : "glass"
            },
            {
                "type" : "sphere",
                "transform" : {
                    "o" : [-1.5, -0.5, -1.5]
                },
                "radius" : 0.5,
                "material" : "blue ball"
            }
        ]
    }"#
    .parse()
    .unwrap()
}

#[allow(clippy::suboptimal_flops)]
fn create_peter_shirley_scene(depth_of_field: bool) -> JsonValue {
    // Create root keys
    let mut m: JsonValue = {
        let s = r#"{
            "camera" : {},
            "sampler": {},
            "background" : [1.0, 1.0, 1.0],
            "materials" : [],
            "shapes" : []
        }"#;
        s.parse().unwrap()
    };

    // Camera
    {
        let s = r#"{
            "vfov" : 20,
            "resolution" : [600, 400],
            "transform" : {
                "from": [13, 2, 3],
                "to"  : [0, 0, 0],
                "up"  : [0, 1, 0]
            },
            "fdist" : 1.0,
            "aperture" : 0.0
        }"#;
        m["camera"] = s.parse().unwrap();
        if depth_of_field {
            m["camera"]["fdist"] = JsonValue::from(10.0);
            m["camera"]["aperture"] = JsonValue::from(0.1);
        }
    }

    // Sampler
    {
        let s = r#"{
            "samples" : 1,
            "type" : "independent"
        }"#;
        m["sampler"] = s.parse().unwrap();
    }

    // Shapes and materials
    let mut materials: Vec<JsonValue> = Vec::new();
    let mut shapes: Vec<JsonValue> = Vec::new();

    // Create ground
    {
        let s = r#"{
            "name" : "ground",
            "type" : "diffuse",
            "albedo" : [0.5, 0.5, 0.5]
        }"#;
        materials.push(s.parse().unwrap());
        let s = r#"{
            "type" : "quad",
            "size" : [100, 100],
            "transform" : {
                "o" : [0, 0, 0],
                "x" : [1, 0, 0],
                "y" : [0, 0, -1],
                "z" : [0, 1, 0]
            },
            "material" : "ground"
        }"#;
        shapes.push(s.parse().unwrap());
    }

    // Create dielectric
    {
        let s = r#"{
            "name" : "glass",
            "type" : "dielectric",
            "eta_int" : 1.5
        }"#;
        materials.push(s.parse().unwrap());
    }

    // Create random spheres
    let mut sampler = Independent::new(1);
    let mut mat_id = 0;
    for a in -11..11 {
        for b in -11..11 {
            let r = sampler.next2d();
            let center = Vec3::new(f64::from(a) + 0.9 * r.x, 0.2, f64::from(b) + 0.9 * r.y);
            if (center - Vec3::new(4.0, 0.2, 0.0)).magnitude() > 0.9 {
                let mat_name = format!("material_{mat_id}");
                mat_id += 1;

                // Create sphere
                let s = r#"{
                    "type" : "sphere",
                    "radius" : 0.2,
                    "transform" : {
                        "translate" : [0.0, 0.0, 0.0]
                    },
                    "material" : "ground"
                }"#;
                let mut s: JsonValue = s.parse().unwrap();
                s["transform"]["translate"] =
                    JsonValue::Array(vec![center.x.into(), center.y.into(), center.z.into()]);
                s["material"] = mat_name.clone().into();

                // Create material
                let rand_mat = sampler.next();
                if rand_mat < 0.8 {
                    let r1 = sampler.next();
                    let r2 = sampler.next();
                    let r3 = sampler.next();
                    let r4 = sampler.next();
                    let r5 = sampler.next();
                    let r6 = sampler.next();
                    let albedo = Color3::new(r1 * r2, r3 * r4, r5 * r6);
                    let m: Vec<(String, JsonValue)> = vec![
                        ("name".to_string(), mat_name.into()),
                        ("type".to_string(), "diffuse".to_string().into()),
                        (
                            "albedo".to_string(),
                            vec![albedo.x.into(), albedo.y.into(), albedo.z.into()].into(),
                        ),
                    ];
                    materials.push(JsonValue::Object(m.into_iter().collect()));
                } else if rand_mat < 0.95 {
                    let r1 = sampler.next();
                    let r2 = sampler.next();
                    let r3 = sampler.next();
                    let r4 = sampler.next();

                    let albedo = Color3::new(0.5 * (1.0 + r1), 0.5 * (1.0 + r2), 0.5 * (1.0 + r3));
                    let rough = 0.5 * r4;
                    let m: Vec<(String, JsonValue)> = vec![
                        ("name".to_string(), mat_name.into()),
                        ("type".to_string(), "metal".to_string().into()),
                        (
                            "ks".to_string(),
                            vec![albedo.x.into(), albedo.y.into(), albedo.z.into()].into(),
                        ),
                        ("roughness".to_string(), rough.into()),
                    ];
                    materials.push(JsonValue::Object(m.into_iter().collect()));
                } else {
                    // Replace to glass
                    s["material"] = "glass".to_string().into();
                }

                shapes.push(s);
            }
        }
    }

    // Create the big spheres
    {
        let s = r#"{
            "type" : "sphere",
            "radius" : 1.0,
            "transform" : {
                "translate" : [0, 1, 0]
            },
            "material" : "glass"
        }"#;
        shapes.push(s.parse().unwrap());
    }
    {
        let s = r#"{
            "name" : "big_mat_1",
            "type" : "diffuse",
            "albedo" : [0.4, 0.2, 0.1]
        }"#;
        materials.push(s.parse().unwrap());
        let s = r#"{
            "type" : "sphere",
            "radius" : 1.0,
            "transform" : {
                "translate" : [-4, 1, 0]
            },
            "material" : "big_mat_1"
        }"#;
        shapes.push(s.parse().unwrap());
    }
    {
        let s = r#"{
            "name" : "big_mat_2",
            "type" : "metal",
            "ks" : [0.7, 0.6, 0.5],
            "roughness" : 0.0
        }"#;
        materials.push(s.parse().unwrap());
        let s = r#"{
            "type" : "sphere",
            "radius" : 1.0,
            "transform" : {
                "translate" : [4, 1, 0]
            },
            "material" : "big_mat_2"
        }"#;
        shapes.push(s.parse().unwrap());
    }

    // Return the complete scene
    m["shapes"] = JsonValue::from(shapes);
    m["materials"] = JsonValue::from(materials);
    m
}

#[must_use]
pub fn create_example_scene(id: u32) -> JsonValue {
    match id {
        0 => create_sphere_scene(),
        1 => create_peter_shirley_scene(false),
        2 => create_peter_shirley_scene(true),
        3 => create_three_sphere_class(),
        _ => panic!("Wrong example scene id ({id}). Need to be between 0 to 3"),
    }
}
