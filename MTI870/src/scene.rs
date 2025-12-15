use std::{collections::HashMap, fmt::Write, sync::Arc};

use cgmath::{ElementWise, InnerSpace, Zero};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::{info, warn};
use tinyjson::JsonValue;

use crate::{
    array2d::Array2d,
    camera::CameraPerspective,
    integrators::{json_to_integrator, Integrator},
    json::{json_to_string, json_to_vec3},
    materials::{json_to_material, Material},
    ray::Ray,
    samplers::{independent::Independent, json_to_sampler, Sampler},
    sdf::{json_to_sdf_object, SDFObject},
    shapes::{
        bvh::BVH, json_to_shape, shape_group::ShapeGroup, triangle::Triangle, EmitterSample,
        Intersection, Shape,
    },
    vec::{Color3, Frame, Point3, Vec2, Vec3},
    Real, NUMBER_TRACED_RAYS,
};

pub struct Scene {
    pub root: Box<dyn Shape>,
    pub materials: HashMap<String, Arc<dyn Material>>,
    pub camera: CameraPerspective,
    pub background: Color3,
    pub max_depth: usize,
    pub envmap: Option<Array2d<Color3>>,
    /// Collection of implicit primitives evaluated through SDF marching.
    pub sdf_objects: Vec<Arc<dyn SDFObject>>,
    /// True when at least one analytic shape emits light (used for direct sampling guards).
    has_analytic_emitters: bool,
}

impl Scene {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
    ) -> (Scene, Box<dyn Sampler>, Box<dyn Integrator>) {
        // Read the scene to create the camera, materials and shapes
        let background = json_to_vec3(json, "background", Vec3::zero());

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
            if !jmats.is_array() {
                panic!("Materials needs to be specified as an array\n\t{:?}", jmats);
            }

            let jmats: &Vec<JsonValue> = jmats.get().unwrap();
            jmats
                .iter()
                .map(|jmat| {
                    if !jmat.is_object() {
                        panic!("Material needs to be specified as object\n\t{:?}", jmat);
                    }
                    let jmat: &HashMap<_, _> = jmat.get().unwrap();
                    if !jmat.contains_key("name") {
                        panic!("Materials need to have a name\n\t{:?}", jmat);
                    }
                    let name: String = jmat["name"].get::<String>().unwrap().to_string();
                    info!("Create material: {}", name);

                    (name, json_to_material(jmat))
                })
                .collect()
        } else {
            HashMap::new()
        };

        let mut root: Box<dyn Shape> = if json.contains_key("accelerator") {
            let json_accel = json["accelerator"].get().unwrap();
            let accel_type_str = json_to_string(json_accel, "type", "linear");
            match &accel_type_str[..] {
                "linear" => Box::new(ShapeGroup::default()),
                "bvh" => Box::new(BVH::from_json(json_accel)),
                _ => {
                    warn!(
                        "Unknown accelerator type: {} -- use linear instead",
                        accel_type_str
                    );
                    Box::new(ShapeGroup::default())
                }
            }
        } else {
            Box::new(ShapeGroup::default())
        };

        let mut has_analytic_emitters = false;
        if json.contains_key("shapes") {
            let jshapes = &json["shapes"];
            if !jshapes.is_array() {
                panic!("Shapes needs to be specified as an array\n\t{:?}", jshapes);
            }
            let jshapes: &Vec<_> = jshapes.get().unwrap();

            for jshape in jshapes {
                if !jshape.is_object() {
                    panic!("Shape needs to be specified as object\n\t{:?}", jshape);
                }
                let jshape: &HashMap<_, _> = jshape.get().unwrap();

                if !jshape.contains_key("material") {
                    panic!("Shapes needs to be specified as an array\n\t{:?}", jshape);
                }
                let material_name: String = jshape["material"].get::<String>().unwrap().to_string();
                if !materials.contains_key(&material_name) {
                    panic!("Impossible to found a material named: {}", material_name);
                }

                if materials[&material_name].have_emission() {
                    has_analytic_emitters = true;
                }

                let s = json_to_shape(jshape, materials[&material_name].clone());
                match s {
                    crate::shapes::JsonShape::Shape(s) => root.add_shape(s),
                    crate::shapes::JsonShape::Mesh(m) => {
                        if let Some(m) = m {
                            // Convert mesh to triangle
                            let m = Arc::new(m);
                            for face_id in 0..m.face_positions_idx.len() {
                                root.add_shape(Box::new(Triangle {
                                    face_id,
                                    mesh: m.clone(),
                                }))
                            }
                        }
                    }
                }
            }
        }

        // Parse optional SDF objects list.
        let mut sdf_objects: Vec<Arc<dyn SDFObject>> = Vec::new();
        if let Some(jsdfs) = json.get("sdf_objects") {
            if !jsdfs.is_array() {
                panic!("`sdf_objects` must be an array");
            }
            let jsdfs: &Vec<JsonValue> = jsdfs.get().unwrap();
            for jsdf in jsdfs {
                if !jsdf.is_object() {
                    panic!("SDF object entries must be objects: {:?}", jsdf);
                }
                let jsdf: &HashMap<String, JsonValue> = jsdf.get().unwrap();
                sdf_objects.push(json_to_sdf_object(jsdf, &materials));
            }
        }

        // charger l'envmap si présent --- Help from ChadGPT
        let envmap = if let Some(j) = json.get("background_image") {
            if let Some(path) = j.get::<String>() {
                let img = image::open(path)
                    .unwrap_or_else(|e| panic!("Failed to load background_image {}: {}", path, e))
                    .to_rgba8();
                let (w, h) = img.dimensions();
                let mut tex = Array2d::with_size(w, h, Color3::zero());
                for y in 0..h {
                    for x in 0..w {
                        let p = img.get_pixel(x, y);
                        let r = p[0] as f64 / 255.0;
                        let g = p[1] as f64 / 255.0;
                        let b = p[2] as f64 / 255.0;
                        *tex.at_mut(x, y) = Color3::new(r, g, b);
                    }
                }
                Some(tex)
            } else {
                None
            }
        } else {
            None
        };

        info!("Build acceleration structure ...");
        let now = std::time::Instant::now();
        root.build();
        info!(
            "Acceleration structure build in {}s",
            now.elapsed().as_secs_f64()
        );

        (
            Scene {
                root,
                materials,
                camera,
                background,
                max_depth: 16,
                envmap,
                sdf_objects,
                has_analytic_emitters,
            },
            sampler,
            integrator,
        )
    }

    pub fn render(&self, sampler: &mut dyn Sampler) -> Array2d<Color3> {
        let mut im = Array2d::with_size(
            self.camera.resolution.x,
            self.camera.resolution.y,
            Color3::zero(),
        );

        let progress = ProgressBar::new(self.camera.resolution.x as u64);
        progress.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{wide_bar}] {pos:>7}/{len:7} ({eta})",
            )
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn Write| {
                write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
            })
            .progress_chars("#>-"),
        );
        for x in 0..self.camera.resolution.x {
            for y in 0..self.camera.resolution.y {
                /*TODO:
                L'interieur de la boucle de: "task7_recursive_raytracing"
                Adapter ce code: sampler.nb_samples() vous permet de savoir combien d'echantillion par pixel.

                Pour rappel, les principales étapes:
                - Initialiser une couleure temporaire à noire.
                - Pour chaque échantillions (disponible:
                    * Génération d'un point 2D aléatoire dans le pixel (si filtre de reconstruction de type boite)
                    * Génération du rayon passant par ce point avec la caméra
                    * Appel de "trace" pour évaluer la lumière incidente. Accumuler dans la couleure temporaire
                - Calculer la moyenne de la couleur temporaire, sauvegardez la dans l'image pour le pixel donné
                */
                // votrecodeici!(
                //     "Devoir 1: calculer la moyenne de la lumiere passant par le pixel (x,y)"
                // );

                let mut color = Color3::zero();
                let num_samples = sampler.nb_samples();
                for e in 0..num_samples {
                    let dx = sampler.next();
                    let dy = sampler.next();
                    let ray = self
                        .camera
                        .generate_ray(&Vec2::new(x as f64 + dx, y as f64 + dy), sampler);

                    color = color + self.trace(ray, sampler, self.max_depth)
                }
                color = color / (num_samples as f64);
                *im.at_mut(x, y) = color;
            }

            progress.inc(1);
        }
        im
    }

    pub fn trace(&self, r: Ray, sampler: &mut dyn Sampler, depth: usize) -> Color3 {
        /*TODO:
        Copier et adapter le code de la fonction "trace" dans "examples/devoir_1.rs".
        La couleur de l'environement peut etre obtenu avec "background".

        Boucle de rendu recursif, ou les principale étapes sont:
        1) Si depth == 0, on termine le chemin en retournant la couleur noire
        2) On test l'intersection en utilisant sur tout les objects de la scène (m_root)
            - Si nous avons un intersection valide:
                * On construit le repère local en utilisant la normale de l'intersection et l'objet Frame
                * On essaye d'échantillioné le materiau (avec la bonne direction d'incidence dans les coordonnées locales)
                    - Si on a reussi: on creér un nouveau rayon et on retourne le weight due au sampling et la lumière incidente (en utilisant trace)
                    - Sinon on renvoye la valeure d'emission du matriaux
            - Sinon (pas d'intersection), retourner la couleur du fond (background)

        Rappel: pour la multiplication par element: mul_element_wise
        */
        let black = Color3::new(0.0, 0.0, 0.0);
        if depth == 0 {
            return black;
        }
        if let Some(intersec) = self.root.hit(&r) {
            let f = Frame::new(&intersec.n);
            let le = intersec.material.emission(&f.to_local(&-r.d));
            if let Some(sd) = intersec
                .material
                .sample(&f.to_local(&-r.d), &sampler.next2d())
            {
                return le
                    + sd.weight.mul_element_wise(self.trace(
                        Ray::new(&intersec.p, &f.to_world(&sd.wi)),
                        sampler,
                        depth - 1,
                    ));
            } else {
                return le;
            }
        } else {
            return self.background(r.d);
        }
    }

    // pub fn background(&self, _d: Vec3) -> Color3 {
    //     self.background
    // }
    pub fn background(&self, d: Vec3) -> Color3 {
        self.sample_env_latlong(d)
    }

    //ChadGPT assisted
    fn dir_to_uv(d: Vec3) -> (f64, f64) {
        use std::f64::consts::PI;
        let dn = d.normalize();
        // theta from +Y (north pole) -> v=0 at top of the image
        let theta = (dn.y.max(-1.0).min(1.0)).acos(); // [0, π]
                                                      // phi such that -Z maps to u=0.5
        let phi = (dn.x).atan2(-dn.z); // (-π, π]
        let u = 0.5 + phi / (2.0 * PI); // wrap later
        let v = theta / PI; // [0,1]
        (u, v)
    }

    //ChadGPT assisted
    fn sample_env_latlong(&self, d: Vec3) -> Color3 {
        let tex = match &self.envmap {
            Some(t) => t,
            None => return self.background, // fallback couleur
        };
        let w = tex.width() as f64;
        let h = tex.height() as f64;

        let (mut u, mut v) = Self::dir_to_uv(d);
        // wrap u into [0,1)
        u = u - u.floor();
        // clamp v in [0,1]
        v = v.max(0.0).min(1.0);

        // (0,0) = top-left: v=0 top, donc y = v*(h-1) direct
        let x = u * (w - 1.0);
        let y = v * (h - 1.0);

        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = ((x0 + 1) % tex.width());
        let y1 = (y0 + 1).min(tex.height() - 1);

        let tx = x - x0 as f64;
        let ty = y - y0 as f64;

        let c00 = *tex.at(x0, y0);
        let c10 = *tex.at(x1, y0);
        let c01 = *tex.at(x0, y1);
        let c11 = *tex.at(x1, y1);

        let cx0 = c00 * (1.0 - tx) + c10 * tx;
        let cx1 = c01 * (1.0 - tx) + c11 * tx;
        cx0 * (1.0 - ty) + cx1 * ty
    }

    pub fn hit<'a>(&'a self, r: &Ray) -> Option<Intersection<'a>> {
        NUMBER_TRACED_RAYS.with_borrow_mut(|f| (*f) += 1);
        self.root.hit(r)
    }

    fn sample_direct(
        &self,
        p: &crate::vec::Point3,
        sample: &crate::vec::Vec2,
    ) -> (EmitterSample, &dyn Shape) {
        return self.root.sample_direct(p, sample);
    }

    fn pdf_direct(
        &self,
        shape: &dyn Shape,
        p: &crate::vec::Point3,
        y: &crate::vec::Point3,
        n: &crate::vec::Vec3,
    ) -> crate::Real {
        return self.root.pdf_direct(shape, p, y, n);
    }

    pub fn visible(&self, p0: &Point3, p1: &Point3) -> bool {
        // Calcul de la direction entre p1 et p0
        let d = p1 - p0;
        let dist = d.magnitude();
        let d = d / dist;

        // Prise en compte de tmin et tmax
        let dist = dist - crate::constants::RAY_EPS * 2.0;

        // Verifie s'il y a une intersection entre p0 et p1
        let r = Ray::new(p0, &d).with_distance_max(dist);
        return self.hit(&r).is_none();
    }

    pub fn has_analytic_emitters(&self) -> bool {
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

/// Create the scene used in the class
fn create_four_sphere_class() -> JsonValue {
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
                "name": "red fresnel ball",
                "type": "metal",
                "use_fresnel": true,
                "roughness": 0.1,
                "ks" : [0.9, 0.1, 0.1],
                "eta": [0.271, 0.676, 1.316],
                "k":   [3.609, 2.624, 2.292]
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
                "material" : "red fresnel ball"
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

fn create_five_sphere_class() -> JsonValue {
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
        "background_image": "./qwantani_noon_puresky_4k.exr",
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
                "name": "red fresnel ball",
                "type": "metal",
                "use_fresnel": true,
                "roughness": 0.1,
                "ks" : [0.9, 0.1, 0.1],
                "eta": [0.271, 0.676, 1.316],
                "k":   [3.609, 2.624, 2.292]
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
                "material" : "red fresnel ball"
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
            let center = Vec3::new(a as f64 + 0.9 * r.x, 0.2, b as f64 + 0.9 * r.y);
            if (center - Vec3::new(4.0, 0.2, 0.0)).magnitude() > 0.9 {
                let mat_name = format!("material_{}", mat_id);
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

pub fn create_example_scene(id: u32) -> JsonValue {
    match id {
        0 => create_sphere_scene(),
        1 => create_peter_shirley_scene(false),
        2 => create_peter_shirley_scene(true),
        3 => create_three_sphere_class(),
        4 => create_four_sphere_class(),
        5 => create_five_sphere_class(),
        _ => panic!("Wrong example scene id ({}). Need to be between 0 to 5", id),
    }
}
