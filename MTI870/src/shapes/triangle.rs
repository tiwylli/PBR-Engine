use std::{collections::HashMap, sync::Arc};

use cgmath::InnerSpace;
use tinyjson::JsonValue;

use crate::{
    NUMBER_INTERSECTIONS, aabb::AABB, json::json_to_vec3s, materials::Material, ray::Ray, vec::{Point3, Vec2, Vec3, Vec3u}
};

use super::{mesh::Mesh, Intersection, Shape};

pub struct Triangle {
    pub face_id: usize,
    pub mesh: Arc<Mesh>,
}

impl Triangle {
    pub fn from_json(json: &HashMap<String, JsonValue>, material: Arc<dyn Material>) -> Self {
        if !json.contains_key("positions") || !json["positions"].is_array() {
            panic!("Invalid triangle: missing positions. Need to specify an array of 3 positions.");
        }

        // Create empty mesh
        let mut mesh = Mesh {
            material,
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            tangents: Vec::new(),
            face_positions_idx: Vec::new(),
            face_normals_idx: Vec::new(),
            face_uvs_idx: Vec::new(),
            face_tangents_idx: Vec::new(),
        };

        // Reading the positions
        let positions: &Vec<JsonValue> = json["positions"].get().unwrap();
        if positions.len() != 3 {
            panic!("Invalid triangle: need to specify an array of 3 positions.");
        }
        let positions = json_to_vec3s(positions).unwrap();
        mesh.positions = positions
            .iter()
            .map(|p| Point3::new(p.x, p.y, p.z))
            .collect();
        mesh.face_positions_idx = vec![Vec3u::new(0, 1, 2)];

        // Reading the normals (if provided)
        if json.contains_key("normals") && json["normals"].is_array() {
            let normals: &Vec<JsonValue> = json["normals"].get().unwrap();
            if normals.len() != 3 {
                panic!("Invalid triangle: need to specify an array of 3 normals.");
            }
            let normals = json_to_vec3s(normals).unwrap();
            mesh.normals = normals;
            mesh.face_normals_idx = vec![Vec3u::new(0, 1, 2)];
        }

        Triangle {
            face_id: 0,
            mesh: Arc::new(mesh),
        }
    }
}

impl Shape for Triangle {
    fn hit<'a>(&'a self, r: &Ray) -> Option<Intersection<'a>> {
        NUMBER_INTERSECTIONS.with(|f| *f.borrow_mut() += 1);
        // Example pour récupérer les positions des sommets
        let idx = self.mesh.face_positions_idx[self.face_id];
        let p0 = self.mesh.positions[idx.x as usize];
        let p1 = self.mesh.positions[idx.y as usize];
        let p2 = self.mesh.positions[idx.z as usize];
        /* Les autres informations auxiliaires:
        normale: self.mesh.normals
        coordonnée de texture: self.mesh.uvs

        Vous avez aussi les méthodes self.mesh.has_uv() et self.mesh.has_normal()
        pour vérifier si ces informations auxiliaires existent ou non.

        Enfin, attention: les indices pour ces informations auxiliaires peuvent être différents.
        Par exemple: self.mesh.face_normals_idx[self.face_id], renvoie les indices à utiliser pour les normales.

        Si c'est une information auxiliaire n'est pas disponible, utilisez la valeur par défaut:
        - normals: utiliser la normale du plan contenant le triangle (attention à l'ordre des vecteurs lors du calcul avec le produit vectoriel).
        - uv (uniquement pour la fin du devoir 1): utiliser les coordonnées de textures vues en cours (0,0),(1,0),(1,1)

        self.mesh.material.as_ref() pour récupérer le matériau lié au mesh.
        */

        let e1 = p1 - p0;
        let e2 = p2 - p0;

        let h = r.d.cross(e2);
        let a: f64 = e1.dot(h);

        // If a is near zero, ray is parallel to triangle
        if a.abs() < 1e-12 {
            return None;
        }

        let f = 1.0 / a;
        let s = r.o - p0;
        let u = f * s.dot(h);
        if u < 0.0 || u > 1.0 {
            return None;
        }

        let q = s.cross(e1);
        let v = f * r.d.dot(q);
        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        // Compute distance along ray
        let t = f * e2.dot(q);
        if t < r.tmin || t > r.tmax {
            return None;
        }

        // Barycentric coordinates (u, v, w)
        let w = 1.0 - u - v;

        // Compute normal: interpolate vertex normals if present, otherwise use geometric normal
        let n: Vec3 = if self.mesh.has_normal() && !self.mesh.face_normals_idx.is_empty() {
            let n_idx = self.mesh.face_normals_idx[self.face_id];
            let n0 = self.mesh.normals[n_idx.x as usize];
            let n1 = self.mesh.normals[n_idx.y as usize];
            let n2 = self.mesh.normals[n_idx.z as usize];
            (n0 * w + n1 * u + n2 * v).normalize()
        } else {
            e1.cross(e2).normalize()
        };

        let material = self.mesh.material.as_ref();

        Some(Intersection {
            t,
            p: r.point_at(t),
            n,
            material,
            shape: self,
        })
    }

    fn sample_direct(&self, p: &Point3, sample: &Vec2) -> (super::EmitterSample, &dyn Shape) {
        // Uniform sampling on triangle using the parallelogram trick
        let mut xi = *sample;
        if xi.x + xi.y > 1.0 {
            xi.x = 1.0 - xi.x;
            xi.y = 1.0 - xi.y;
        }

        let idx = self.mesh.face_positions_idx[self.face_id];
        let p0 = self.mesh.positions[idx.x as usize];
        let p1 = self.mesh.positions[idx.y as usize];
        let p2 = self.mesh.positions[idx.z as usize];

        let pos = p0 + (p1 - p0) * xi.x + (p2 - p0) * xi.y;

        // Normal: interpolated if available, else geometric
        let e1 = p1 - p0;
        let e2 = p2 - p0;
        let area = 0.5 * e1.cross(e2).magnitude();

        let n = if self.mesh.has_normal() && !self.mesh.face_normals_idx.is_empty() {
            let n_idx = self.mesh.face_normals_idx[self.face_id];
            let n0 = self.mesh.normals[n_idx.x as usize];
            let n1 = self.mesh.normals[n_idx.y as usize];
            let n2 = self.mesh.normals[n_idx.z as usize];
            (n0 * (1.0 - xi.x - xi.y) + n1 * xi.x + n2 * xi.y).normalize()
        } else {
            e1.cross(e2).normalize()
        };

        // Surface PDF and conversion to solid angle
        let p_a = 1.0 / (area.max(1e-12));
        let d = pos - *p;
        let dist2 = d.magnitude2();
        let dist = dist2.sqrt();
        let dir = d / dist.max(1e-12);
        let cos_theta = n.dot(dir).abs().max(1e-12);
        let p_omega = p_a * dist2 / cos_theta;

        let mut es = super::EmitterSample::new();
        es.y = pos;
        es.n = n;
        es.pdf = p_omega;
        return (es, self);
    }

    fn pdf_direct(&self, shape: &dyn Shape, p: &Point3, y: &Point3, n: &Vec3) -> crate::Real {
        // if !std::ptr::eq(self as &dyn Shape, shape) {
        //     return 0.0;
        // }
        let idx = self.mesh.face_positions_idx[self.face_id];
        let p0 = self.mesh.positions[idx.x as usize];
        let p1 = self.mesh.positions[idx.y as usize];
        let p2 = self.mesh.positions[idx.z as usize];
        let e1 = p1 - p0;
        let e2 = p2 - p0;
        let area = 0.5 * e1.cross(e2).magnitude();
        let p_a = 1.0 / area.max(1e-12);

        let d = *y - *p;
        let dist2 = d.magnitude2();
        let dist = dist2.sqrt();
        let dir = d / dist.max(1e-12);
        let cos_theta = n.dot(dir).abs().max(1e-12);
        let p_omega = p_a * dist2 / cos_theta;
        return p_omega;
    }

    fn material(&self) -> &dyn Material {
        self.mesh.material.as_ref()
    }

    fn add_shape(&mut self, _: Box<dyn Shape>) {}
    fn build(&mut self) {}
    fn aabb(&self) -> AABB {
        let mut aabb = AABB::default();
        let idx = self.mesh.face_positions_idx[self.face_id];
        aabb.extend(self.mesh.positions[idx.x as usize]);
        aabb.extend(self.mesh.positions[idx.y as usize]);
        aabb.extend(self.mesh.positions[idx.z as usize]);

        // Make sure that the AABB is enough
        // thick for all the dimensions
        let diag = aabb.diagonal();
        for i in 0..3 {
            if diag[i] < 2e-4 {
                aabb.min[i] -= 1e-4;
                aabb.max[i] += 1e-4;
            }
        }

        return aabb;
    }
}
