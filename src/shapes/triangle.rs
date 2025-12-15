use std::{collections::HashMap, sync::Arc};

use cgmath::{EuclideanSpace, InnerSpace, Matrix3};
use tinyjson::JsonValue;

use crate::{
    NUMBER_INTERSECTIONS, Real,
    aabb::AABB,
    json::json_to_vec3s,
    materials::Material,
    ray::Ray,
    shapes::{EmitterSample, surfacial_to_solid_angle},
    vec::{Point3, Vec2, Vec3, Vec3u},
};

use super::{Intersection, Shape, mesh::Mesh};

pub struct Triangle {
    pub face_id: usize,
    pub mesh: Arc<Mesh>,
    area: Real,
}

impl Triangle {
    #[must_use]
    pub fn new(face_id: usize, mesh: Arc<Mesh>) -> Self {
        let idx = mesh.face_positions_idx[face_id];
        let p0 = mesh.positions[idx.x as usize];
        let p1 = mesh.positions[idx.y as usize];
        let p2 = mesh.positions[idx.z as usize];
        let e1 = p1 - p0;
        let e2 = p2 - p0;
        let area = e1.cross(e2).magnitude() * 0.5;

        Self {
            face_id,
            mesh,
            area,
        }
    }

    pub fn from_json(json: &HashMap<String, JsonValue>, material: Arc<dyn Material>) -> Self {
        assert!(
            json.contains_key("positions") && json["positions"].is_array(),
            "Invalid triangle: missing positions. Need to specify an array of 3 positions."
        );

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
        assert!(
            (positions.len() == 3),
            "Invalid triangle: need to specify an array of 3 positions."
        );
        let positions = json_to_vec3s(positions).unwrap();
        mesh.positions = positions
            .iter()
            .map(|p| Point3::new(p.x, p.y, p.z))
            .collect();
        mesh.face_positions_idx = vec![Vec3u::new(0, 1, 2)];

        // Reading the normals (if provided)
        if json.contains_key("normals") && json["normals"].is_array() {
            let normals: &Vec<JsonValue> = json["normals"].get().unwrap();
            assert!(
                (normals.len() == 3),
                "Invalid triangle: need to specify an array of 3 normals."
            );
            let normals = json_to_vec3s(normals).unwrap();
            mesh.normals = normals;
            mesh.face_normals_idx = vec![Vec3u::new(0, 1, 2)];
        }

        Self::new(0, Arc::new(mesh))
    }
}

impl Shape for Triangle {
    #[allow(clippy::many_single_char_names, clippy::suboptimal_flops)]
    fn hit<'a>(&'a self, r: &Ray) -> Option<Intersection<'a>> {
        NUMBER_INTERSECTIONS.with(|f| *f.borrow_mut() += 1);
        // Example pour recuprer les positions des sommets
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
        let tt = r.o - p0;

        let p = r.d.cross(e2);
        let det = p.dot(e1);

        if det.abs() < 1e-6 {
            return None;
        }

        let mult = 1.0 / det;

        let u = mult * p.dot(tt);
        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let q = tt.cross(e1);
        let v = mult * q.dot(r.d);
        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = mult * q.dot(e2);

        if t > r.tmin && t < r.tmax {
            let w = 1.0 - u - v;
            let base_n = if self.mesh.has_normal() {
                let idx = self.mesh.face_normals_idx[self.face_id];
                let n_u = self.mesh.normals[idx.y as usize];
                let n_v = self.mesh.normals[idx.z as usize];
                let n_w = self.mesh.normals[idx.x as usize];
                u * n_u + v * n_v + w * n_w
            } else {
                e1.cross(e2).normalize()
            };
            let (uv_w, uv_v, uv_u) = if self.mesh.has_uv() {
                let idx = self.mesh.face_uvs_idx[self.face_id];
                let uv_u = self.mesh.uvs[idx.y as usize];
                let uv_v = self.mesh.uvs[idx.z as usize];
                let uv_w = self.mesh.uvs[idx.x as usize];
                (uv_w, uv_v, uv_u)
            } else {
                let uv_w = Vec2::new(0.0, 0.0);
                let uv_u = Vec2::new(1.0, 0.0);
                let uv_v = Vec2::new(1.0, 1.0);
                (uv_w, uv_v, uv_u)
            };
            let p = r.point_at(t);
            let uv = u * uv_u + v * uv_v + w * uv_w;
            let n = if self.mesh.material.have_normal_map() {
                let delta_uv1 = uv_u - uv_w;
                let delta_uv2 = uv_v - uv_w;
                let denom = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv2.x * delta_uv1.y);
                let tangent = denom * (delta_uv2.y * e1 - delta_uv1.y * e2);
                let bitangent = denom * (-delta_uv2.x * e1 + delta_uv1.x * e2);
                let tbn = Matrix3::from_cols(tangent, bitangent, base_n);
                let n = tbn * self.mesh.material.get_normal_map_value(&uv, &p);
                if n.dot(-r.d) < 0.0 { base_n } else { n }
            } else {
                base_n
            };
            Some(Intersection {
                t,
                p,
                n,
                uv,
                material: self.mesh.material.as_ref(),
                shape: self,
            })
        } else {
            None
        }
    }

    #[allow(clippy::many_single_char_names)]
    fn sample_direct(&self, p: &Point3, sample: &Vec2) -> (EmitterSample, &dyn Shape) {
        let (u, v) = if sample.x + sample.y > 1.0 {
            (1.0 - sample.x, 1.0 - sample.y)
        } else {
            (sample.x, sample.y)
        };
        let w = 1.0 - u - v;

        let idx = self.mesh.face_positions_idx[self.face_id];
        let p0 = self.mesh.positions[idx.x as usize];
        let p1 = self.mesh.positions[idx.y as usize];
        let p2 = self.mesh.positions[idx.z as usize];
        let y = u * p0.to_vec() + v * p1.to_vec() + w * p2.to_vec();
        let y = Point3::new(y.x, y.y, y.z);

        let n = if self.mesh.has_normal() {
            let w = 1.0 - u - v;
            let idx = self.mesh.face_normals_idx[self.face_id];
            let n_u = self.mesh.normals[idx.y as usize];
            let n_v = self.mesh.normals[idx.z as usize];
            let n_w = self.mesh.normals[idx.x as usize];
            u * n_u + v * n_v + w * n_w
        } else {
            let e1 = p1 - p0;
            let e2 = p2 - p0;
            e1.cross(e2).normalize()
        };

        (
            EmitterSample {
                y,
                n,
                pdf: self.pdf_direct(self, p, &y, &n),
            },
            self,
        )
    }

    fn pdf_direct(&self, _shape: &dyn Shape, p: &Point3, y: &Point3, n: &Vec3) -> Real {
        surfacial_to_solid_angle(1.0 / self.area, p, y, n)
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

        aabb
    }
}
