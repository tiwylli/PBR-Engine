use std::{collections::HashMap, sync::Arc};

use cgmath::InnerSpace;
use cgmath::Zero;
use log::{error, info};
use tinyjson::JsonValue;

use crate::{
    aabb::AABB,
    fileresolver::FILE_RESOLVER,
    json::json_to_bool,
    materials::Material,
    transform::MyTransform,
    vec::{Point3, Vec2, Vec3, Vec3u},
};

pub struct Mesh {
    // Normal informations from a shape
    pub material: Arc<dyn Material>,
    // Other information for the mesh
    /// Vertex positions (world space)
    pub positions: Vec<Point3>,
    /// Vertex normals (world space)
    pub normals: Vec<Vec3>,
    /// Vertex tangents (world space)
    pub tangents: Vec<Vec3>,
    /// Vertex texture coordinates
    pub uvs: Vec<Vec2>,
    /// Face index
    pub face_positions_idx: Vec<Vec3u>,
    /// Indices for normals (for each faces)
    pub face_normals_idx: Vec<Vec3u>,
    /// Indices for texture coordinates (for each faces)
    pub face_uvs_idx: Vec<Vec3u>,
    /// Indices for tangents (for each faces)
    pub face_tangents_idx: Vec<Vec3u>,
}

use std::mem;
fn integer_decode(val: f64) -> (u64, i16, i8) {
    let bits: u64 = unsafe { mem::transmute(val) };
    let sign: i8 = if bits >> 63 == 0 { 1 } else { -1 };
    let mut exponent: i16 = ((bits >> 52) & 0x7ff) as i16;
    let mantissa = if exponent == 0 {
        (bits & 0xfffffffffffff) << 1
    } else {
        (bits & 0xfffffffffffff) | 0x10000000000000
    };
    exponent -= 1023 + 52;
    (mantissa, exponent, sign)
}

#[derive(Hash, Eq, PartialEq)]
struct Distance((u64, i16, i8));
impl Distance {
    fn new(val: f64) -> Distance {
        Distance(integer_decode(val))
    }
}

impl Mesh {
    pub fn from_json(
        json: &HashMap<String, JsonValue>,
        transform: MyTransform,
        material: Arc<dyn Material>,
    ) -> Option<Self> {
        // Get the filename and resolve where is the file
        if !json.contains_key("filename") {
            panic!("Mesh need to contains filename");
        }
        let filename = json["filename"]
            .get::<String>()
            .expect("Filename need to be a string")
            .clone();
        let filename = FILE_RESOLVER
            .lock()
            .unwrap()
            .resolve(&std::path::Path::new(&filename));

        // Mesh information
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut face_positions_idx = Vec::new();
        let mut face_normals_idx = Vec::new();
        let mut face_uvs_idx = Vec::new();

        // Add triangulate from tobj
        let mut option = tobj::OFFLINE_RENDERING_LOAD_OPTIONS;
        option.triangulate = true;
        let res = tobj::load_obj(&filename, &option);
        if res.is_err() {
            error!("Failed to load file: {:?}", filename);
            return None;
        }
        let (models, _) = res.unwrap();

        // AABB of the whole mesh
        let mut aabb = AABB::default();

        for m in models {
            let offset_position = positions.len() as u32;
            let offset_normal = normals.len() as u32;
            let offset_uvs = uvs.len() as u32;

            let mesh = m.mesh;
            // Read the indices (all are triangle)
            for face in (0..mesh.indices.len()).step_by(3) {
                let face_indices = &mesh.indices[face..face + 3];

                face_positions_idx.push(Vec3u::new(
                    face_indices[0] + offset_position,
                    face_indices[1] + offset_position,
                    face_indices[2] + offset_position,
                ));
                if !mesh.texcoord_indices.is_empty() {
                    let texcoord_face_indices = &mesh.texcoord_indices[face..face + 3];
                    face_uvs_idx.push(Vec3u::new(
                        texcoord_face_indices[0] + offset_uvs,
                        texcoord_face_indices[1] + offset_uvs,
                        texcoord_face_indices[2] + offset_uvs,
                    ));
                }
                if !mesh.normal_indices.is_empty() {
                    let normal_face_indices = &mesh.normal_indices[face..face + 3];
                    face_normals_idx.push(Vec3u::new(
                        normal_face_indices[0] + offset_normal,
                        normal_face_indices[1] + offset_normal,
                        normal_face_indices[2] + offset_normal,
                    ));
                }
            }

            // Read all the positions and transform them
            assert_eq!(mesh.positions.len() % 3, 0);
            for pos in mesh.positions[..].chunks_exact(3) {
                positions.push(transform.point(&Point3::new(
                    pos[0] as f64,
                    pos[1] as f64,
                    pos[2] as f64,
                )));
                aabb.extend(*positions.last().unwrap());
            }

            // Reall all the normal (if provided exists)
            if !mesh.normals.is_empty() {
                assert_eq!(mesh.normals.len() % 3, 0);
                for normal in mesh.normals[..].chunks_exact(3) {
                    normals.push(
                        transform
                            .normal(&Vec3::new(
                                normal[0] as f64,
                                normal[1] as f64,
                                normal[2] as f64,
                            ))
                            .normalize(),
                    );
                }
            }

            // Real all the texture coordinates
            if !mesh.texcoords.is_empty() {
                assert_eq!(mesh.texcoords.len() % 2, 0);
                for uv in mesh.texcoords[..].chunks_exact(2) {
                    uvs.push(Vec2::new(uv[0] as f64, uv[1] as f64));
                }
            }
        }

        let smooth = json_to_bool(json, "smooth", true);
        if !smooth {
            // Remove normal and face normals
            normals.clear();
            face_normals_idx.clear();
        }

        // Compute tangent if normal provided
        let mut face_tangents_idx = Vec::new();
        let mut tangents = Vec::new();
        if !uvs.is_empty() {
            // Compute unique vertices based on position, uv and normal
            // Use this unique vertices to compute tangent indices
            info!("Compute unique vertices for mesh ... ");
            let mut unique_vertices = HashMap::new();
            for face in 0..face_positions_idx.len() {
                let face_idx = face_positions_idx[face];
                let face_uv_idx = face_uvs_idx[face];
                let face_normal_idx = face_normals_idx[face];

                let mut unique_idx = Vec3u::new(0, 0, 0);
                for i in 0..3 {
                    let p = positions[face_idx[i] as usize];
                    let uv = uvs[face_uv_idx[i] as usize];
                    let normal = normals[face_normal_idx[i] as usize];

                    // Hashing the key (f64 is not hashable)
                    let key = (
                        Distance::new(p.x),
                        Distance::new(p.y),
                        Distance::new(p.z),
                        Distance::new(uv.x),
                        Distance::new(uv.y),
                        Distance::new(normal.x),
                        Distance::new(normal.y),
                        Distance::new(normal.z),
                    );

                    // Insert the key if not exists, create new index
                    if !unique_vertices.contains_key(&key) {
                        unique_idx[i] = unique_vertices.len() as u32;
                        unique_vertices.insert(key, unique_vertices.len() as u32);
                    } else {
                        unique_idx[i] = *unique_vertices.get(&key).unwrap();
                    }
                }

                // Insert the unique index for the face
                face_tangents_idx.push(unique_idx);
            }

            info!("Compute tangent for mesh ... ");
            tangents.resize(unique_vertices.len(), Vec3::zero());
            for face in 0..face_positions_idx.len() {
                let face_idx = face_positions_idx[face];
                let face_uv_idx = face_uvs_idx[face];
                let face_unique = face_tangents_idx[face];

                let (p0, p1, p2) = (
                    positions[face_idx.x as usize],
                    positions[face_idx.y as usize],
                    positions[face_idx.z as usize],
                );
                let (uv0, uv1, uv2) = (
                    uvs[face_uv_idx.x as usize],
                    uvs[face_uv_idx.y as usize],
                    uvs[face_uv_idx.z as usize],
                );

                let duv02 = uv0 - uv2;
                let duv12 = uv1 - uv2;

                let determinant = duv02[0] * duv12[1] - duv02[1] * duv12[0];
                let inv_determinant = 1.0 / determinant;
                let dp02 = p0 - p2;
                let dp12 = p1 - p2;
                let tangent = (duv12[1] * dp02 - duv02[1] * dp12) * inv_determinant;

                tangents[face_unique.x as usize] += tangent;
                tangents[face_unique.y as usize] += tangent;
                tangents[face_unique.z as usize] += tangent;
            }

            for tangent in tangents.iter_mut() {
                *tangent = tangent.normalize();
            }
        }

        info!("Loaded: {:?}", filename);
        info!(
            " - AABB (untransform): [{:?}, {:?}]",
            transform.inverse().point(&aabb.min),
            transform.inverse().point(&aabb.max)
        );
        info!(" - AABB     : [{:?}, {:?}]", aabb.min, aabb.max);
        info!(" - centroid : {:?}", aabb.center());
        info!(" - #faces   : {:?}", face_positions_idx.len());
        info!(" - #vertices: {:?}", positions.len());
        info!(" - #tangents: {:?}", tangents.len());
        info!(" - has_normal: {}", !normals.is_empty());
        info!(" - has_uv    : {}", !uvs.is_empty());

        Some(Self {
            material,
            positions,
            normals,
            tangents,
            uvs,
            face_positions_idx,
            face_normals_idx,
            face_uvs_idx,
            face_tangents_idx,
        })
    }

    pub fn has_normal(&self) -> bool {
        !self.normals.is_empty()
    }

    pub fn has_uv(&self) -> bool {
        !self.uvs.is_empty()
    }

    pub fn has_tangent(&self) -> bool {
        !self.tangents.is_empty()
    }
}
