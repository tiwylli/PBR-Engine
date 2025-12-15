// Pour retrirer les warning -- a enlever
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(unused_mut)]

use cgmath::ElementWise;
use cgmath::{Array, EuclideanSpace, InnerSpace, Zero};
use clap::Parser;
use core::num;
use log::{error, info, warn};
use render::{
    array2d::Array2d,
    camera::CameraPerspective,
    deg2rad, function,
    image::image_save,
    materials::{json_to_material, Material},
    ray::Ray,
    samplers::{independent::Independent, Sampler},
    scene::{create_example_scene, Scene},
    shapes::{json_to_shape, shape_group::ShapeGroup, Intersection, Shape},
    transform::MyTransform,
    vec::{Color3, Frame, Mat4, Point3, Vec2, Vec2u, Vec3},
    votrecodeici,
};
use std::thread::sleep;
use tinyjson::JsonValue;

fn ray2color(r: Ray) -> Color3 {
    r.d.normalize() * 0.5 + Color3::new(0.5, 0.5, 0.5)
}

fn vec2color(v: Vec3) -> Color3 {
    (v + Color3::new(1.0, 1.0, 1.0)) * 0.5
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Log ouput
    #[arg(short, long)]
    log: Option<String>,
}

fn main() {
    // Lecture de la ligne de commande
    let args = Args::parse();
    if let Some(log_out) = args.log {
        let target = Box::new(std::fs::File::create(log_out).expect("Can't create file"));
        pretty_env_logger::formatted_builder()
            .filter_level(log::LevelFilter::Info)
            .target(env_logger::Target::Pipe(target))
            .init();
    } else {
        pretty_env_logger::formatted_builder()
            .filter_level(log::LevelFilter::Info)
            .init();
    }

    task1_rays();
    task2_transform();
    task3_improved_camera();
    task4_sphere_intersection();
    task5_intersection_primitives();
    task6_materiaux();
    task7_recursive_raytracing();
}

fn task1_rays() {
    info!("=====================================");
    info!("Tache 1: Generation rayons");
    info!("=====================================");

    // Definition of the image and its size
    let image_width = 400;
    let aspect_ratio = 16.0 / 9.0;
    let image_height = (image_width as f64 / aspect_ratio) as u32;
    let mut im = Array2d::with_size(image_width, image_height, Color3::zero());

    // Definition of the virtual image plane
    let viewport_height = 2.0;
    let viewport_width = aspect_ratio * viewport_height;

    let viewport_u = Vec3::new(viewport_width, 0.0, 0.0);
    let viewport_v = Vec3::new(0.0, -viewport_height, 0.0);
    let pixel_delta_u = viewport_u / image_width as f64;
    let pixel_delta_v = viewport_v / image_height as f64;
    let focal_length = 1.0; // Distance from the plane to the camera position

    // Definition of the camera
    let origin = Point3::new(0.0, 0.0, 0.0);

    let viewport_upper_left =
        origin - Vec3::new(0.0, 0.0, focal_length) - viewport_u / 2.0 - viewport_v / 2.0;
    let pixel00_loc = viewport_upper_left + 0.5 * (pixel_delta_u + pixel_delta_v);

    for x in 0..image_width {
        for y in 0..image_height {
            /* TODO: Calculez l'origine et la direction du rayon
            généré par une caméra perspective canonique (espace local, voir diapositive pour plus d'information). delta_u et delta_v sont les tailles d'un pixel dans le plan image.
            Pour ce faire, il faut que :
            1) La direction du rayon au pixel (0,0) soit égale à [-viewport_width/2 + delta_u/2, viewport_height/2 - delta_v/2, -focal_length].
               Ce pixel correspond au pixel en haut à gauche de l'image.
            2) La direction du rayon au pixel (image_width, image_height) soit [viewport_width/2 - delta_u/2, -viewport_height/2 + delta_v/2, -focal_length].

            Vous pouvez vous référer à la section 4.2 du livre :
            https://raytracing.github.io/books/RayTracingInOneWeekend.html#rays,asimplecamera,andbackground/sendingraysintothescene

            */

            // Coordinate in the image plane for this task
            // Attention, the pixel at (0,0) has coordinates (delta_u, delta_v)
            //let u = (f64::from(x) + 0.5) / f64::from(image_width);
            //let v = (f64::from(y) + 0.5) / f64::from(image_height);

            // TODO: Calculez l'origine et la direction du rayon
            //votrecodeici!("Devoir 1: Calculez l'origine et la direction d'un rayon");

            let ray_origin = pixel00_loc + (x as f64 * pixel_delta_u) + (y as f64 * pixel_delta_v);
            let ray_direction = ray_origin - origin;

            let ray = Ray::new(&ray_origin, &ray_direction);
            *im.at_mut(x, y) = ray2color(ray);
        }
    }

    let path = "task1_rays.png";
    info!("Saving image: {}", path);
    image_save(path, &im);
}

fn task2_transform() {
    info!("=====================================");
    info!("Tache 2: Transformations");
    info!("=====================================");

    // Construction of the transformation
    // Each line here specifies a column of the matrix
    let m = Mat4::new(
        0.688844, 0.515909, -0.158857, 0.000000, -0.482166, 0.022549, -0.190132, 0.000000,
        0.567597, -0.393375, -0.046806, 0.000000, 0.166764, 0.816226, 0.009374, 1.000000,
    );

    let t = MyTransform::new(m);
    info!("transformation: {:?}", t.m);
    info!("transformation inverse: {:?}", t.m_inv);

    let my_vector = Vec3::new(-0.436324, 0.511608, 0.236738);
    let my_point = Point3::new(-0.498987, 0.819493, 0.965571);
    let my_normal = Vec3::new(0.620434, 0.804332, -0.379705).normalize();
    let my_ray = Ray::new(
        &Point3::new(0.459663, 0.797677, 0.367968),  // Origin
        &Vec3::new(-0.055715, -0.798598, -0.131656), // Direction
    )
    .with_distance_max(1000.0);

    info!("vecteur: {:?}", my_vector);
    info!("point: {:?}", my_point);
    info!("normal: {:?}", my_normal);
    info!("rayon: o = {:?}, d = {:?}", my_ray.o, my_ray.d);

    let my_vector_transformed = t.vector(&my_vector);
    let my_point_transformed = t.point(&my_point);
    let my_normal_transformed = t.normal(&my_normal);
    let my_ray_transformed = t.ray(&my_ray);

    let vector_expected = Vec3::new(-0.412867, -0.306694, -0.0390407);
    let point_expected = Vec3::new(-0.0240367, 0.197441, -0.112365);
    let normal_expected = Vec3::new(-0.365388, 0.70678, -2.89047);
    let ray_expected = Ray::new(
        &Point3::new(0.307645, 0.926608, -0.232534), // Origin
        &Vec3::new(0.27195, 0.00503872, 0.166852),   // Direction
    )
    .with_distance_max(1000.0);

    // Lambda function to help checking the results
    let check_result = |value: Vec3, correct: Vec3, name: &str| {
        info!("{} transformed: {:?}", name, value);
        if (correct - value).magnitude2() > 0.0001 {
            warn!(
                "Resultat incorrect! (erreur: {})",
                (correct - value).magnitude2()
            );
            warn!("Le resultat attendu est: {:?}", correct);
        } else {
            info!("Resultat correct!");
        }
    };

    check_result(my_vector_transformed, vector_expected, "vecteur");
    check_result(my_point_transformed.to_vec(), point_expected, "point");
    check_result(my_normal_transformed, normal_expected, "normal");
    check_result(
        my_ray_transformed.o.to_vec(),
        ray_expected.o.to_vec(),
        "ray (origin)",
    );
    check_result(my_ray_transformed.d, ray_expected.d, "ray (direction)");

    info!(
        "distance maximal du rayon transformed: {}",
        my_ray_transformed.tmax
    );
    if (my_ray_transformed.tmax - ray_expected.tmax).abs() > 0.0001 {
        warn!("Resultat incorrect!");
        warn!(
            "La distance maximal du rayon attendu est: {}",
            ray_expected.tmax
        );
    } else {
        info!("Resultat correct!");
    }
}

fn task3_improved_camera() {
    info!("=====================================");
    info!("Tache 3: Amelioration camera");
    info!("=====================================");
    /*TODO:
    Aller mettre en oeuvre la caméra perspective définie dans "include/render/camera.h"
    Le code ci-dessous va uniquement tester cet objet pour produire une image.
    Bien vérifier que vous générer la bonne image.
    */

    // Définition de l'image et de sa taille
    let image_width = 400;
    let aspect_ratio = 16.0 / 9.0;
    let image_height = (image_width as f64 / aspect_ratio) as u32;
    let mut im = Array2d::with_size(image_width, image_height, Color3::zero());

    // Construction d'une camera perspective
    let camera = {
        // Transformation de type "look at". Plus d'information:
        // https://raytracing.github.io/books/RayTracingInOneWeekend.html#positionablecamera/positioningandorientingthecamera
        let json: JsonValue = r#"{
            "vfov" : 90.0,
            "resolution" : [400, 225],
            "fdist" : 1.0,
            "transform" : {
                "from" : [5.0, 15.0, -25.0],
                "to" :   [0.0, 0.0, 0.0],
                "up" :   [0.0, 1.0, 0.0]
            }
        }"#
        .parse()
        .unwrap();
        CameraPerspective::from_json(&json.get().unwrap())
    };

    // Objet pour générer des nombres aléatoires
    // ignorer pour l'instant
    let mut sampler = Independent::new(1);

    for x in 0..image_width {
        for y in 0..image_height {
            // Position au centre du pixel
            let pos_img = Vec2::new(x as f64 + 0.5, y as f64 + 0.5);
            let ray = camera.generate_ray(&pos_img, &mut sampler);
            *im.at_mut(x, y) = ray2color(ray);
        }
    }

    let path = "task3_camera.png";
    info!("Sauvegarde de l'image: {}", path);
    image_save(path, &im);
}

// Structure nous permettant de stocker
// sous forme d'image une intersection
struct IntersectionImage {
    position: Array2d<Color3>,
    normal: Array2d<Color3>,
    distance: Array2d<Color3>,

    // Mise a l'echelle
    pub scale_distance: f64,
    pub scale_position: f64,
}

impl IntersectionImage {
    pub fn new(size_x: u32, size_y: u32) -> Self {
        IntersectionImage {
            position: Array2d::with_size(size_x, size_y, Color3::zero()),
            normal: Array2d::with_size(size_x, size_y, Color3::zero()),
            distance: Array2d::with_size(size_x, size_y, Color3::zero()),
            scale_distance: 1.0,
            scale_position: 1.0,
        }
    }

    pub fn add(&mut self, p: Vec2u, its: &Intersection) {
        *self.distance.at_mut(p.x, p.y) = Color3::from_value(its.t * self.scale_distance);
        *self.normal.at_mut(p.x, p.y) = vec2color(its.n);
        *self.position.at_mut(p.x, p.y) = vec2color(its.p.to_vec() * self.scale_position);
    }

    pub fn save(&self, path: &str) {
        image_save(&(path.to_string() + "_distance.png"), &self.distance);
        image_save(&(path.to_string() + "_normal.png"), &self.normal);
        image_save(&(path.to_string() + "_position.png"), &self.position);
    }
}

fn generate_intersection_images(
    camera: &CameraPerspective,
    shape: &dyn Shape,
    path: &str,
    sampler: &mut dyn Sampler,
    scale_distance: f64,
    scale_position: f64,
) {
    let resolution = camera.resolution;
    let mut im = IntersectionImage::new(resolution.x, resolution.y);
    im.scale_position = scale_position;
    im.scale_distance = scale_distance;

    info!("Generation images des intersections ...");

    for x in 0..resolution.x {
        for y in 0..resolution.y {
            // Generate the camera ray
            let pos_img = Vec2::new(x as f64 + 0.5, y as f64 + 0.5); // Position au centre du pixel
            let ray = camera.generate_ray(&pos_img, sampler);

            // Compute the intersection with the shape
            if let Some(its) = shape.hit(&ray) {
                // If intersection, save it
                im.add(Vec2u::new(x, y), &its);
            }
        }
    }

    info!("Sauvegarde de l'image: {}", path);
    im.save(path);
}

fn task4_sphere_intersection() {
    info!("=====================================");
    info!("Tache 4: Intersection sphere simple");
    info!("=====================================");

    // Define the image and its size
    let image_width = 400;
    let aspect_ratio = 16.0 / 9.0;
    let image_height = (image_width as f64 / aspect_ratio) as u32;

    // Create a perspective camera
    let camera = {
        // Transformation de type "look at". Plus d'information:
        // https://raytracing.github.io/books/RayTracingInOneWeekend.html#positionablecamera/positioningandorientingthecamera
        let json: JsonValue = r#"{
            "vfov" : 90.0,
            "resolution" : [400, 225],
            "fdist" : 1.0,
            "transform" : {
                "from" : [2.0, 2.0, 2.0],
                "to" :   [0.0, 0.0, 0.0],
                "up" :   [0.0, 1.0, 0.0]
            }
        }"#
        .parse()
        .unwrap();
        CameraPerspective::from_json(&json.get().unwrap())
    };

    // Create a sampler
    let mut sampler = Box::new(Independent::new(1));

    // Create a Lambertian material (for testing purposes)
    let material = {
        let json: JsonValue = r#"{
            "type" : "diffuse"
         }"#
        .parse()
        .unwrap();
        json_to_material(json.get().unwrap())
    };

    ///////////////////////////////////
    // Test sphere without transformations
    ///////////////////////////////////
    let sphere = {
        let json: JsonValue = r#"{
            "type" : "sphere",
            "radius" : 2.0
        }"#
        .parse()
        .unwrap();
        json_to_shape(json.get().unwrap(), material.clone())
    };

    let path = "task4_sphere_no_transform";
    generate_intersection_images(
        &camera,
        sphere.as_ref(),
        path,
        sampler.as_mut(),
        1.0 / 4.0,
        1.0,
    );

    // Camera interieur de la sphere
    let camera_inside = {
        // Transformation de type "look at". Plus d'information:
        // https://raytracing.github.io/books/RayTracingInOneWeekend.html#positionablecamera/positioningandorientingthecamera
        let json: JsonValue = r#"{
            "vfov" : 90.0,
            "resolution" : [400, 225],
            "fdist" : 1.0,
            "transform" : {
                "from" : [0.0, 0.0, 0.0],
                "to" :   [1.0, 0.0, 0.0],
                "up" :   [0.0, 1.0, 0.0]
            }
        }"#
        .parse()
        .unwrap();
        CameraPerspective::from_json(&json.get().unwrap())
    };
    let path = "task4_sphere_inside";
    generate_intersection_images(
        &camera_inside,
        sphere.as_ref(),
        path,
        sampler.as_mut(),
        1.0 / 4.0,
        1.0,
    );

    // Simple test d'un rayon qui ne devrait pas intersecter la sphere
    // car le centre de la sphere est en (0,0,0) et son rayon est 2.0
    {
        // Rayon partant de (10,0,0) et allant vers (1,0,0)
        let ray_miss = Ray::new(&Point3::new(10.0, 0.0, 0.0), &Vec3::new(1.0, 0.0, 0.0));
        if let Some(its) = sphere.as_ref().hit(&ray_miss) {
            warn!("Rayon qui ne devrait pas intersecter la sphere a intersecte:");
            warn!("\tRayon: o = {:?}, d = {:?}", ray_miss.o, ray_miss.d);
            warn!(
                "\tIntersection: t = {}, p = {:?}, n = {:?}",
                its.t, its.p, its.n
            );
        } else {
            info!("Rayon qui ne devrait pas intersecter la sphere n'a pas intersecte. Correct!");
        }
    }

    ///////////////////////////////////
    // Test sphere with transformations
    ///////////////////////////////////
    let sphere_with_transform = {
        let json: JsonValue = r#"{
            "type" : "sphere",
            "radius" : 1.0,
            "transform": [
                {"scale": [0.2, 2.0, 1.0]},
                {"translate": [0.5, 0.5, -0.5]}
            ]
        }"#
        .parse()
        .unwrap();
        json_to_shape(json.get().unwrap(), material.clone())
    };

    let path = "task4_sphere_transform";
    generate_intersection_images(
        &camera,
        sphere_with_transform.as_ref(),
        path,
        sampler.as_mut(),
        1.0 / 4.0,
        1.0,
    );
    //sleep(std::time::Duration::from_millis(50000000));
}

fn task5_intersection_primitives() {
    info!("=====================================");
    info!("Tache 5: Intersection d'objet multiples");
    info!("=====================================");

    let mut sampler = Box::new(Independent::new(1));

    // Scene 1: a sphere and a plane
    {
        let (example_scene, _, _) = Scene::from_json(create_example_scene(0).get().unwrap());
        let path = "task5_scene1";
        generate_intersection_images(
            &example_scene.camera,
            &example_scene.root,
            path,
            sampler.as_mut(),
            1.0 / 8.0,
            1.0,
        );
    }

    // Scene 2: Peter Shirley's scene (multiple spheres)
    {
        let (example_scene, _, _) = Scene::from_json(create_example_scene(1).get().unwrap());
        let path = "task5_scene2";
        generate_intersection_images(
            &example_scene.camera,
            &example_scene.root,
            path,
            sampler.as_mut(),
            1.0 / 30.0,
            1.0 / 10.0,
        );
    }
}

// Structure permettant de representer la direction
// que l'on s'attend à générer avec les matériaux
struct ExpectedSample {
    direction: Vec3, // outgoing direction
    weight: Color3,  // weight (color of the material)
    invalid: bool,   // if the sample will be invalid (or not)
}

// Create invalid sample (e.g., sample in the wrong hemisphere)
fn invalid() -> ExpectedSample {
    ExpectedSample {
        direction: Vec3::zero(),
        weight: Color3::zero(),
        invalid: true,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SphericalCoords {
    theta: f64,
    phi: f64,
}

impl Into<Vec3> for SphericalCoords {
    fn into(self) -> Vec3 {
        let theta = deg2rad(self.theta);
        let phi = deg2rad(self.phi);
        Vec3::new(
            theta.sin() * phi.cos(),
            theta.sin() * phi.sin(),
            theta.cos(),
        )
    }
}

fn test_one_material(
    material: &dyn Material,
    directions: &[SphericalCoords],
    expected: &[ExpectedSample],
) -> bool {
    let mut sampler = Independent::new(1);
    let mut passed = 0;
    // let mut res = "".to_string();
    for i in 0..directions.len() {
        let exp = &expected[i];
        let mut incorrect = false;

        // Information du sample
        let (failed, weight, direction) =
            if let Some(res) = material.sample(&directions[i].into(), &mut sampler.next2d()) {
                (false, res.weight, res.wi)
            } else {
                // Echec de l'echantillionage
                (true, Color3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0))
            };

        incorrect |= exp.invalid != failed;
        incorrect |= (direction - exp.direction).magnitude() > 0.0001;
        incorrect |= (weight - exp.weight).magnitude() > 0.0001;

        if !incorrect {
            passed += 1;
        } else {
            warn!("Failed material test:");
            let vec_dir: Vec3 = directions[i].into();
            warn!("Incoming direction: {:?} => {:?}", directions[i], vec_dir);
            if exp.invalid {
                warn!("\tExpected invalid sample (None)");
            } else {
                warn!(
                    "\tExpected valid sample: d = {:?}, weight = {:?}",
                    exp.direction, exp.weight
                );
            }
            if failed {
                warn!("\tGenerated invalid sample (None)");
            } else {
                warn!(
                    "\tGenerated valid sample: d = {:?}, weight = {:?}",
                    direction, weight
                );
            }
        }
        // if failed {
        //     res += "invalid(),\n";
        // } else {
        //     res += "ExpectedSample {";
        //     res += &format!("direction: Vec3::new({}, {}, {}), weight: Color3::new({}, {}, {}), invalid: false",
        //                 direction.x, direction.y, direction.z, weight.x, weight.y, weight.z);
        //     res += "},\n";
        // }
    }
    // println!("{}",res);
    let success = passed == directions.len();
    success
}

fn task6_materiaux() {
    info!("=====================================");
    info!("Tache 6: Materiaux");
    info!("=====================================");
    /*TODO:
    Le code ci-dessous va tester votre mise en oeuvre des different materiaux.
    Veuillez bien mettre en oeuvre les méthodes "sample(...)" des matériaux suivant:
    - Materiau diffus: include/render/materiaux/diffuse.h
    - Materiau reflechissant: include/render/materiaux/metal.h
    - Materiaux dielectrique: include/render/materiaux/dielectric.h

    Pour tester vos materiaux, nous allons directement générer des directions incidente dans l'espace local
    du materiau. Ici on utilise les coordonnées sphériques.
    */
    let mut directions = vec![
        SphericalCoords {
            theta: 0.0,
            phi: 0.0,
        },
        SphericalCoords {
            theta: 180.0,
            phi: 0.0,
        },
        SphericalCoords {
            theta: 89.0,
            phi: 90.0,
        },
        SphericalCoords {
            theta: 91.0,
            phi: -90.0,
        },
    ];
    let mut sampler = Independent::new(1);
    directions.extend((0..3).map(|_| SphericalCoords {
        theta: sampler.next() * 90.0,
        phi: sampler.next() * 360.0,
    }));
    directions.extend((0..3).map(|_| SphericalCoords {
        theta: sampler.next() * 90.0 + 90.0,
        phi: sampler.next() * 360.0,
    }));

    // Show informations
    info!("Directions tested: ");
    for d in &directions {
        info!("{:?}", d);
    }

    {
        info!("Test materiaux type: lambertien ... ");
        let s = r#"{
            "type" : "diffuse"
        }"#;
        let s: JsonValue = s.parse().unwrap();
        let material = json_to_material(s.get().unwrap());
        let expected = vec![
            ExpectedSample {
                direction: Vec3::new(0.3917282584690241, -0.06384980439655656, 0.9178628296185366),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            invalid(),
            ExpectedSample {
                direction: Vec3::new(0.04084447324139645, -0.2533597547306289, 0.9665094741845361),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            invalid(),
            ExpectedSample {
                direction: Vec3::new(0.3232843166945053, -0.7033747091112983, 0.6330491838411637),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(0.3928980199881561, 0.02688508813162865, 0.9191889565946391),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(0.022133653048097317, 0.3880136272218649, 0.921387826321185),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            invalid(),
            invalid(),
            invalid(),
        ];
        if test_one_material(material.as_ref(), &directions, &expected) {
            info!("All test passed! :)");
        } else {
            warn!("Some test have failed :(");
        }
    }

    {
        info!("Test materiaux type: metal (speculaire) ... ");
        let s = r#"{
            "type" : "metal"
        }"#;
        let s: JsonValue = s.parse().unwrap();
        let material = json_to_material(s.get().unwrap());
        let expected = vec![
            ExpectedSample {
                direction: Vec3::new(-0.0, -0.0, 1.0),
                weight: Color3::new(1.0, 1.0, 1.0),
                invalid: false,
            },
            invalid(),
            ExpectedSample {
                direction: Vec3::new(
                    -0.00000000000000006122301397540666,
                    -0.9998476951563913,
                    0.0174524064372836,
                ),
                weight: Color3::new(1.0, 1.0, 1.0),
                invalid: false,
            },
            invalid(),
            ExpectedSample {
                direction: Vec3::new(0.8768947055397616, -0.19068387980566373, 0.4412429414490321),
                weight: Color3::new(1.0, 1.0, 1.0),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(-0.8275151027648804, -0.3286663195218558, 0.4551891970466642),
                weight: Color3::new(1.0, 1.0, 1.0),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(0.934850523421044, 0.30071032912528795, 0.18875326968495412),
                weight: Color3::new(1.0, 1.0, 1.0),
                invalid: false,
            },
            invalid(),
            invalid(),
            invalid(),
        ];
        if test_one_material(material.as_ref(), &directions, &expected) {
            info!("All test passed! :)");
        } else {
            warn!("Some test have failed :(");
        }
    }

    {
        info!("Test materiaux type: metal (roughness = 0.4) ... ");
        let s = r#"{
            "type" : "metal",
            "ks" : 0.8,
            "roughness" : 0.4
        }"#;
        let s: JsonValue = s.parse().unwrap();
        let material = json_to_material(s.get().unwrap());
        let expected = vec![
            ExpectedSample {
                direction: Vec3::new(
                    0.14275800051902648,
                    -0.02326886103343023,
                    0.9894840642446024,
                ),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            invalid(),
            ExpectedSample {
                direction: Vec3::new(
                    0.01684250741935937,
                    -0.9795491888708416,
                    0.20049866963724458,
                ),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            invalid(),
            ExpectedSample {
                direction: Vec3::new(0.8613515484811701, -0.3815424519502118, 0.33540850807472233),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(-0.6467406548796032, -0.329416534034484, 0.6879035342477965),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(0.8636204677450282, 0.4053714486402815, 0.299722665674684),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            invalid(),
            invalid(),
            invalid(),
        ];
        if test_one_material(material.as_ref(), &directions, &expected) {
            info!("All test passed! :)");
        } else {
            warn!("Some test have failed :(");
        }
    }

    {
        info!("Test materiaux type: dielectric (glass) ... ");
        let s = r#"{
            "type" : "dielectric",
            "ks" : 0.8
        }"#;
        let s: JsonValue = s.parse().unwrap();
        let material = json_to_material(s.get().unwrap());
        let expected = vec![
            ExpectedSample {
                direction: Vec3::new(-0.0, -0.0, -1.0),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(-0.00000000000000018369701987210297, 0.0, 1.0),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(
                    -0.00000000000000006122301397540666,
                    -0.9998476951563913,
                    0.0174524064372836,
                ),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(
                    -0.00000000000000006122301397540666,
                    0.9998476951563913,
                    -0.017452406437283477,
                ),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(0.8768947055397616, -0.19068387980566373, 0.4412429414490321),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(
                    -0.5516767351765869,
                    -0.21911087968123721,
                    -0.8047628236134026,
                ),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(0.623233682280696, 0.20047355275019196, -0.7559035202448527),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(-0.3657961455178893, 0.1571316472410266, 0.9173346310695879),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(
                    -0.25744560599303556,
                    -0.37521467848273443,
                    0.8904693734239134,
                ),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
            ExpectedSample {
                direction: Vec3::new(-0.0359855178167492, 0.27709755228696975, 0.9601676879712369),
                weight: Color3::new(0.8, 0.8, 0.8),
                invalid: false,
            },
        ];
        if test_one_material(material.as_ref(), &directions, &expected) {
            info!("All test passed! :)");
        } else {
            warn!("Some test have failed :(");
        }
    }
}

fn trace(r: Ray, shapes: &ShapeGroup, sampler: &mut dyn Sampler, depth: i32) -> Color3 {
    const max_depth: i32 = 32;

    /*TODO:
    Mettre en oeuvre le pseudo code ci-dessous
    */
    // if depth >= max_depth:
    //      return black;
    // if scene.intersect:
    //       compute local frame
    // 		 if hit_material.sample(....) is successful:
    //			recursive_color = call this function recursively with the scattered ray and increased depth
    //          return attenuation * recursive_color
    //		else
    //			return hit_material.emission(...); // return the emission of the material
    // else:
    // 		return black; // Couleur de l'environement

    // votrecodeici!("Devoir 1: ray tracing recursif");
    // Color3::zero()
    let black = Color3::new(0.0, 0.0, 0.0);
    if depth >= max_depth {
        return black;
    }
    if let Some(intersec) = shapes.hit(&r) {
        let f = Frame::new(&intersec.n);
        let le = intersec.material.emission(&f.to_local(&-r.d));
        if let Some(sd) = intersec
            .material
            .sample(&f.to_local(&-r.d), &sampler.next2d())
        {
            return le
                + sd.weight.mul_element_wise(trace(
                    Ray::new(&intersec.p, &f.to_world(&sd.wi)),
                    shapes,
                    sampler,
                    depth + 1,
                ));
        } else {
            return le;
        }
    } else {
        return black;
    }
}

fn task7_recursive_raytracing() {
    info!("=====================================");
    info!("Tache 7: Tracer de rayon recursif");
    info!("=====================================");
    /*TODO:
    Mettez en oeuvre la fonction trace (juste au dessus).
    Mettez en oeuvre le calcul de la valeur moyenne d'un pixel
    */

    // Creation d'une scene composee de plusieurs sphere et plan.
    // Une source de lumiere formée par un plan.
    let (example_scene, mut sampler, _) = Scene::from_json(create_example_scene(3).get().unwrap());
    let resolution = example_scene.camera.resolution;
    let camera = &example_scene.camera;
    let root = &example_scene.root;

    // On va utiliser 8 echantillions par pixels
    let num_samples = 64;
    sampler.set_nb_samples(num_samples);

    let mut im = Array2d::with_size(resolution.x, resolution.y, Color3::zero());
    for x in 0..resolution.x {
        for y in 0..resolution.y {
            let ray =
                camera.generate_ray(&Vec2::new(x as f64 + 0.5, y as f64 + 0.5), sampler.as_mut()); // centre du pixel
            let mut color = Color3::zero();

            /*TODO:
            Calculer la valeure moyenne d'un pixel
            Utiliser la fonction "trace" plusieurs fois dans le pixel (x,y) pour evaluer la lumiere incidente.
            */
            // votrecodeici!("Devoir 1: moyenne de la valeur d'un pixel");

            for e in 0..num_samples {
                color = color + trace(ray, &root, sampler.as_mut(), 0)
            }
            color = color / (num_samples as f64);
            *im.at_mut(x, y) = color;
        }
    }

    let path = "task7_recursive.png";
    image_save(path, &im);
}
