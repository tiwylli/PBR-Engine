// Pour retrirer les warning -- a enlever
#![allow(unused_variables)]
#![allow(unused_imports)]

// La libraire matricelle
use cgmath::{dot, ElementWise, InnerSpace, SquareMatrix, Transform, Vector3, Zero};
// Lecture des entree ligne de commande
use clap::Parser;
// Generation des nombres aléatoires
use rand::prelude::*;

// Pour la configuration des logs
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

// Pour la generation des nombres aléatoires
use rand_chacha::ChaCha8Rng;

// Nos imports de render (notre libraire)
use render::{
    array2d::Array2d,
    image::{image_load, image_save},
    rad2deg,
    vec::{luminance, Color3, Mat4, Point3, Vec3, Vec4},
    Real,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the island image
    #[arg(short, long, default_value = "scenes/devoir0/island.jpg")]
    input: String,

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

    task_1_maths();
    task_2_images(&args.input)
}

fn task_1_maths() {
    info!("=====================================");
    info!("Tache 1: Vecteurs et matrices");
    info!("=====================================");

    ////////////////////////////////////////////////////////////////////////////////////////
    /* Introduction:

    Le code de base vous fournit des objets pour représenter des vecteurs et des matrices.
    Ces entités sont très utilisées dans les applications d'infographie.

    Les définitions de ces objets se trouvent dans: src/vec.rs
    Ces définitions se basent sur la libraire mathématique `linalg`: https://github.com/rustgd/cgmath

    Dans les applications en infographie, il est commun que la dimension du vecteur soit 2, 3, ou 4.
    Un vecteur de dimension 4 sera utilisé pour exprimer une direction (ou position) en coordonnées homogènes.
    Le type est dépendant de comment est utilisé le vecteur:
    - Une direction 3D utilisera le type f32 ou f64. Dans notre cas, on utilise Real qui est defini dans lib.rs
    - Une dimension d'une image (vecteur 2D) utilisent u32 ou usize

    Les vecteurs seront utilisés pour représenter normales ou des directions. Les points seront representé explicitement ou utiliserons les vecteurs.
    Dans ce cas, faire attention lors des transformations.

    */
    ////////////////////////////////////////////////////////////////////////////////////////

    ////////////////////////////////////////////////////////////////////////////////////////
    /* Création d'un vecteur:
       Unique facon de faire
    */
    ////////////////////////////////////////////////////////////////////////////////////////
    let v0 = Vec3::zero(); // Le vecteur null doit etre explicite
    let v1 = Vec3::new(1.0, 2.0, 3.0);
    let v2 = Vec3::new(0.5, 0.5, 0.5);

    ////////////////////////////////////////////////////////////////////////////////////////
    /* Accès aux valeurs d'un vecteur:
    Pour accéder aux valeurs d'un vecteur, il y a plusieurs façons.
    La première est l'utilisation de l'opérateur "[]" comme pour les tableaux.
    v0[0] va retourner la première composante du vecteur.
    Une autre alternative est pour les vecteurs 2, 3, et 4D. Pour ces vecteurs, nous pouvons
    directement accéder aux composantes x, y, z, w.
    Voici quelques exemples:
    */
    ////////////////////////////////////////////////////////////////////////////////////////

    // Attention en Rust les variables sont par defaut non mutable
    // Ici on redeclare la variable mutable
    let mut v0 = v0;
    v0.x = 1.0;
    v0[2] = 3.0;

    // Affichage d'un vecteur en entier
    info!("v2: {:?}", v2);

    // Affichage de la première et dernier composant de v1
    info!("v1[0] = {}, v1[2] = {}", v1[0], v1[2]);

    // Affichage de la composante .z de v0
    // error!("v0.z = {}", "TODO");
    info!("v0.z = {}", v0.z);

    ////////////////////////////////////////////////////////////////////////////////////////
    /* Operations vectorielles:
    Les opérations de base telles que la multiplication, division, soustraction et addition
    sont disponibles. Enfin, les opérations vectorielles tel
    que le produit scalaire (dot), le produit vectoriel (cross), la norme euclidienne (distance),
    ... sont aussi accessibles. La liste complète de ces opérations est disponible à:
    https://docs.rs/cgmath/latest/cgmath/index.html
    */
    ////////////////////////////////////////////////////////////////////////////////////////

    // Addition
    info!("v0 + v1 = {:?}", v0 + v1);

    // Multiplication par scalaire
    info!("v0 = {:?}, v0 * 5.0 = {:?}", v0, v0 * 5.0);

    // Division par un vecteur
    info!("v0 / v1 = {:?}", v0.sub_element_wise(v1));

    // Calcul de la norme d'un vecteur
    info!("||v0|| =  {}", v0.magnitude());

    // Normalisation d'un vecteur
    let v0norm = v0.normalize();
    info!("normalized(v0) = {:?}", v0norm);

    // Une autre façon de normaliser
    let v0norm2 = v0 / v0.magnitude();
    info!("v0norm == v0norm2 ? {}", v0norm == v0norm2);

    // Calcul de l'angle en degré entre le vecteur v0 et v1
    // Tips: Voir les diapositives du cours sur le rappel des notions mathématiques
    // Faites attentions aux conversion radian et angles (regardez lib.rs)

    let degree = 0.0; // TODO
    let degree = rad2deg((dot(v0, v1) / (v0.magnitude() * v1.magnitude())).acos()); //cos-1(dot(v0,v1)/(norm(v0)*norm(v1)))
    info!("angle(v0, v1) = {}", degree);

    if (degree - 32.311532338 as Real).abs() > 1e-4 {
        error!(" Resultat incorrect!");
    } else {
        info!("Resultat correct");
    }

    ////////////////////////////////////////////////////////////////////////////////////////
    /* Matrices:
    Les matrices se comportent de façon très similaire aux vecteurs.
    La liste des opérations sur les matrices: https://github.com/sgorsten/linalg#matrix-algebra
    */
    ////////////////////////////////////////////////////////////////////////////////////////

    // Initialisation
    let m0 = Mat4::zero(); // Attention par défaut, on construit une matrice avec tous les coefficients a 0.0
    let _m1 = Mat4::identity(); // Matrice identité
                                // Les matrices sont données sous forme de colonnes
                                // Pour m2, la premiere colonne est: [1.0, 0.0, 0.0, 0.0]
    let m2 = Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 2.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 0.1, 0.0),
        Vec4::new(10.0, 10.0, 10.0, 1.0),
    );

    // Affichage
    info!("m0 = {:?}", m0);
    info!("_m1 = {:?}", _m1);
    info!("m2 = {:?}", m2);
    info!("inv(m2) = {:?}", m2.invert().unwrap()); // unwarp here is because the invert can failed
                                                   // Column major: le premier adressage spécifie la colonne
                                                   // le second la ligne
    info!("m2[3][0] = {}", m2[3][0]);

    ////////////////////////////////////////////////////////////////////////////////////////
    /* Attention aux multiplications matricielles.
    Dependament des libraires, ca peut etre une operation element-wise ou une multiplication
    matricielle.
    */
    ////////////////////////////////////////////////////////////////////////////////////////
    let res = m2 * m2.invert().unwrap();
    info!("m2*inv(m2) = {:?}", res);
    if res.is_identity() {
        info!("Resultat correct");
    } else {
        error!(" Resultat incorrect!");
    }

    // Nous avons en cours les coordonnées homogènes
    // Soit un point et un vecteur 3D aléatoire
    // Nous allons utiliser un générateur que l'on peut controller.
    let mut gen = ChaCha8Rng::seed_from_u64(0);
    // Ici on choisi de representer un point avec l'objet point
    // Regardez si le resultat change si vous utilisez une representation par vecteur.
    // (seulement après que votre code est fonctionnel)
    let point = Point3::new(gen.gen(), gen.gen(), gen.gen());
    let vector: cgmath::Vector3<f64> = Vec3::new(gen.gen(), gen.gen(), gen.gen()).normalize();

    // TODO: appliquez la matrice "m2" pour transformer le point et le vecteur
    // Vous avez des methodes transform pour chacun de ces types
    // Faites aussi attention aux dimensions des vecteurs et de la matrice

    // let new_point = Point3::new(0.0, 0.0, 0.0);
    let new_point = Transform::transform_point(&m2, point);

    // let new_vector = Vec3::new(0.0, 0.0, 0.0);
    let new_vector = Transform::transform_vector(&m2, vector);

    info!("Transformed point: {:?}", new_point);
    if (new_point - Point3::new(10.709075415426561, 10.93184344457922, 10.069914324267474))
        .magnitude()
        < 10e-4
    {
        info!("Resultat correct");
    } else {
        error!(" Resultat incorrect!");
    }

    info!("Transformed vector: {:?}", new_vector);
    if (new_vector
        - Vec3::new(
            0.057941570417450725,
            1.693071923486465,
            0.052916881984204245,
        ))
    .magnitude()
        < 10e-4
    {
        info!("Resultat correct");
    } else {
        error!(" Resultat incorrect!");
    }
}

fn task_2_images(path: &str) {
    info!("=====================================");
    info!("Tache 2: Images et couleurs");
    info!("=====================================");

    ////////////////////////////////////////////////////////////////////////////////////////
    /* Couleurs:
    Dans ce cours, les couleurs seront représentée en RVG (ou RGB).
    Cela signifie que les couleurs seront exprimée sous forme de vecteur.
    Cependant, nous allons utiliser l'objet "Color3" et non "Vec3" pour bien distinguer que c'est une couleur.
    L'initialisation et les opérations arthimétiques s'effectuent de la même facon qu'un vecteur.
    Dans ce devoir, nous utiliserons des couleurs a basse dynamique ou chaque composante est défini entre: [0.0, 1.0].
    */
    ////////////////////////////////////////////////////////////////////////////////////////

    // Initialisation de quelques couleurs
    let white = Color3::new(1.0, 1.0, 1.0);
    let black = Color3::zero();
    let gray = Color3::new(0.5, 0.5, 0.5);
    let red = Color3::new(1.0, 0.0, 0.0);
    let blue = Color3::new(0.0, 0.0, 1.0);
    let green = Color3::new(0.0, 1.0, 0.0);

    // Nous pouvons faire des operations sur les couleurs
    let darker_gray = gray * 0.8;
    info!("gray: {:?}, darker gray: {:?}", gray, darker_gray);
    let still_red = red.mul_element_wise(white);
    info!("red: {:?}, still red: {:?}", red, still_red);
    let blue_is_new_black = blue.mul_element_wise(black);
    info!("blue is new black: {:?}", blue_is_new_black);

    // TODO: Faite la couleur "purple" (violet) en additionant le rouge et bleu
    // let purple = Color3::zero(); // TODO
    let purple = blue + red; // TODO
    info!("purple: {:?}", purple);

    ////////////////////////////////////////////////////////////////////////////////////////
    /* Image 2D
    Une image 2D est simplement un tableau 2D ou chaque élément est une couleur.
    La définition d'une image peut etre trouvée dans le fichier: src/image.rs

    Pour créer une image, nous devons définir sa résolution en X et Y.
    */
    ////////////////////////////////////////////////////////////////////////////////////////
    let mut im_black = Array2d::with_size(512, 512, Color3::zero());

    // Acceder a un pixel en particulier
    info!("pixel value at position (0,0) : {:?}", im_black.at(0, 0));

    /* TODO: Changer l'image noire en effectuant une interpolation bilineaire
    avec les couleurs définie dans les coins de la manière suivante:
    - Rouge en haut à gauche ou (0,0)
    - Bleu en haut à droite ou (width, 0)
    - Vert en bas à gauche ou (0, height)
    - Blanc en bas à droite ou (width, height)
    Vous pouvez utiliser les couleurs qui ont déjà initialisée.
    */
    let width = im_black.size_x() - 1;
    let height = im_black.size_y() - 1;

    for x in 0..im_black.size_x() {
        for y in 0..im_black.size_y() {
            // TODO:Changer la ligne ci-dessous et calculez l'interpolation bilineaire
            // Plus d'information sur cette interpolation
            // https://www.f-legrand.fr/scidoc/docmml/image/niveaux/interpolation/interpolation.html
            // Notez ici on applique cette interpolation entre les différents coins de l'image
            // *im_black.at_mut(x, y) = black;
            let w: f64 = (width as f64 - x as f64) / (width as f64);
            let h: f64 = (height as f64 - y as f64) / (height as f64);

            // *im_black.at_mut(x, y) += w * red + (1.0 - w) * blue;
            // *im_black.at_mut(x, y) += h * red + (1.0 - h) * green;
            // *im_black.at_mut(x, y) += w * green + (1.0 - w) * white;
            // *im_black.at_mut(x, y) += h * blue + (1.0 - h) * white;

            *im_black.at_mut(x, y) = w * h * red
                + (1.0 - w) * h * blue
                + (1.0 - h) * w * green
                + (1.0 - w) * (1.0 - h) * white;
        }
    }

    // On peut sauvegarder l'image facilement avec la fonction "save_image"
    image_save("bilinear.png", &im_black).unwrap();

    // Chargement de l'image
    info!("Chargement de l'image {}", path);
    let mut im = image_load(path, true).unwrap();
    info!(" - dimensions: {} {}", im.width(), im.height());

    // TODO: Calculer de la moyenne des pixels composant l'image
    let mut avg = Color3::zero();
    for x in 0..im.size_x() {
        for y in 0..im.size_y() {
            avg += *im.at(x, y);
        }
    }
    let s: f64 = im.size_x() as f64 * im.size_y() as f64;
    avg = avg / s;

    info!("Moyenne des pixels: {:?}", avg);

    // TODO: Transformer l'image en noir et blanc et stocker la dans im_lum
    // Vous pouvez utiliser la fonction "double luminace(const Color3& c)" définie dans vec.h
    let mut im_lum: Array2d<Vector3<f64>>;
    im_lum = Array2d::copy_from(&im);
    for x in 0..im_lum.size_x() {
        for y in 0..im_lum.size_y() {
            let c = *im_lum.at(x, y);
            let lum = luminance(&c);
            let grayval = Color3::new(lum, lum, lum);
            *im_lum.at_mut(x, y) = grayval;
        }
    }

    image_save("luminance.png", &im_lum).unwrap();
}
