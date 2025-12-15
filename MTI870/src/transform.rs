use std::collections::HashMap;

use cgmath::{Matrix, SquareMatrix, Transform, Zero};
use tinyjson::JsonValue;

use crate::json::json_to_mat4;
use crate::ray::Ray;
use crate::vec::{Mat4, Point3, Vec3, Vec4};

// Note: Unfortunatly, we cannot call this class "Transform"
// As it is a trait defined in cgmath.
#[derive(Debug, PartialEq)]
pub struct MyTransform {
    pub m: Mat4,
    pub m_inv: Mat4,
}

impl MyTransform {
    /// Simple transform constructor which compute the inverse
    pub fn new(m: Mat4) -> MyTransform {
        Self {
            m: m.clone(),
            m_inv: m.invert().unwrap(),
        }
    }

    /// Create new transformation based on a given matrix
    /// and its inverse
    pub fn new_with_inverse(m: Mat4, m_inv: Mat4) -> MyTransform {
        Self { m, m_inv }
    }

    /// Return the inverse transformation
    pub fn inverse(&self) -> MyTransform {
        Self {
            m: self.m_inv,
            m_inv: self.m,
        }
    }

    /// Apply the homogenous transformation to a 3D direction vector
    pub fn vector(&self, v: &Vec3) -> Vec3 {
        /*
        TODO: Vous devez utiliser les coordonnées homogènes pour
        appliquer la transformation à un vecteur.

        Dans le cours, nous avons vu comment exprimer des vecteurs dans les coordonnées
        homogènes.
        */
        //votrecodeici!("Devoir 1: transformation pour un vecteur");
        Transform::transform_vector(&self.m, *v)
    }

    /// Apply transformation to a 3D normal vector
    pub fn normal(&self, n: &Vec3) -> Vec3 {
        /* TODO: Mettre en oeuvre la transformation d'un vecteur normal.
        Nous avons vu en cours que cela demande le calcul d'une matrice
        particulière.
        */
        //votrecodeici!("Devoir 1: transformation pour un vecteur normal");
        Transform::transform_vector(&self.m_inv.transpose(), *n)
    }

    /// Apply transformation to a 3D point
    pub fn point(&self, p: &Point3) -> Point3 {
        /* TODO: Dans le cours nous avons des moyens.

        Si vous utilisez une presentation par vector, n'oubliez pas
        la conversion en coordonnée homogène
         */
        //votrecodeici!("Devoir 1: transformation pour un point");
        Transform::transform_point(&self.m, *p)
    }

    pub fn ray(&self, r: &Ray) -> Ray {
        /*
        TODO: Utiliser les methodes définie plus haut
        pour transformer l'origine (r.o) et la direction (r.d)
        d'un rayon.

        The distance ranges are unchanged
        */
        //votrecodeici!("Devoir 1: transformation pour un rayon");

        Ray::new(&self.point(&r.o), &self.vector(&r.d)).with_range(r.tmin, r.tmax)
    }
}

/// The multiplication of two transformation
impl std::ops::Mul for MyTransform {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        MyTransform {
            m: self.m * rhs.m,
            m_inv: self.m_inv * rhs.m_inv,
        }
    }
}

/// Empty transformation
impl Default for MyTransform {
    fn default() -> Self {
        Self {
            m: Mat4::identity(),
            m_inv: Mat4::identity(),
        }
    }
}

/// Read the JSON given transform or use the identity transform
pub fn json_to_transform(json: &HashMap<String, JsonValue>, name: &str) -> MyTransform {
    if json.contains_key(name) {
        MyTransform::new(json_to_mat4(json, name).unwrap())
    } else {
        MyTransform::default()
    }
}
