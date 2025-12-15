use std::{cmp::Ordering, collections::HashMap};

use log::{info, warn};
use tinyjson::JsonValue;

use crate::{
    NUMBER_INTERSECTIONS,
    aabb::{AABB, merge_aabb},
    json::json_to_string,
    ray::Ray,
    vec::Vec2,
};

use super::{Intersection, Shape};

// Structure to cache the AABB for a given shape
// this structure is only used for BVH construction
pub struct CachedShapeAABB {
    pub aabb: AABB,
    pub shape: Box<dyn Shape>,
}

pub enum BVHNodeInfo {
    Leaf {
        frist_primitive: usize,
        primitive_count: usize,
    },
    Node {
        left: usize,
    },
}

pub struct BVHNode {
    aabb: AABB, // AABB of the node
    pub info: BVHNodeInfo,
}
impl BVHNode {
    /// Create a new leaf node by computing the AABB
    #[must_use]
    #[allow(clippy::needless_range_loop)]
    pub fn new_leaf(
        frist_primitive: usize,
        primitive_count: usize,
        aabbs: &[CachedShapeAABB],
    ) -> Self {
        // Calculez le AABB pour le nouveau noeud
        // ce noeud represente la plage des forme [frist_primitive, frist_primitive+primitive_count[
        // votrecodeici!("Devoir 2: Calculer la boite englobante correspondant au noeud donnee");
        // Self {
        //     aabb: AABB::default(),
        //     info: BVHNodeInfo::Leaf {
        //         frist_primitive: frist_primitive as u32,
        //         primitive_count: primitive_count as u32,
        //     },
        // }

        // SOLUTION
        let mut aabb = AABB::default();
        for i in frist_primitive..(frist_primitive + primitive_count) {
            aabb = merge_aabb(&aabb, &aabbs[i].aabb);
        }

        Self {
            aabb,
            info: BVHNodeInfo::Leaf {
                frist_primitive,
                primitive_count,
            },
        }
    }

    #[must_use]
    pub fn frist_primitive(&self) -> usize {
        match self.info {
            BVHNodeInfo::Leaf {
                frist_primitive, ..
            } => frist_primitive,
            BVHNodeInfo::Node { .. } => unimplemented!(), // Not possible, have you already converted the node?
        }
    }

    #[must_use]
    pub fn primitive_count(&self) -> usize {
        match self.info {
            BVHNodeInfo::Leaf {
                primitive_count, ..
            } => primitive_count,
            BVHNodeInfo::Node { .. } => unimplemented!(), // Not possible, have you already converted the node?
        }
    }

    /// convert to a tree node. Left is the indice value of the left element (inside the node array)
    pub const fn to_node(&mut self, left: usize) {
        self.info = BVHNodeInfo::Node { left }
    }
}

pub struct BVH {
    axis_selection: AxisSelection,
    builder: Builder,
    nodes: Vec<BVHNode>,
    shapes: Option<Vec<Box<dyn Shape>>>, // Use option to make possible to transfer ownership during building
    aabbs: Vec<AABB>,
    emitters: Vec<usize>,
}

#[derive(Debug)]
pub enum AxisSelection {
    ERoundRobin, // Alternate between x, y, z
    ELongest,    // Take the longest axis
}

#[derive(Debug)]
pub enum Builder {
    EMedian,  // Split using the median using the selected axis
    ESpatial, // Split using the middle using the selected axis
    ESAH,     // Perform sweep SAH
}

impl BVH {
    #[must_use]
    pub fn from_json(json: &HashMap<String, JsonValue>) -> Self {
        // Read axis split strategy
        // this strategy is ignored is we use SAH
        let axis_selection_str = json_to_string(json, "axis_selection", "roundrobin");
        let axis_selection = match &axis_selection_str[..] {
            "roundrobin" => AxisSelection::ERoundRobin,
            "longest" => AxisSelection::ELongest,
            _ => {
                warn!("Unknown axis_selection: {axis_selection_str}, use Round robin instead.");
                AxisSelection::ERoundRobin
            }
        };

        // Builder BVH strategy
        let builder_str = json_to_string(json, "builder", "sah");
        let builder = match &builder_str[..] {
            "sah" => Builder::ESAH,
            "median" => Builder::EMedian,
            "spatial" => Builder::ESpatial,
            _ => {
                warn!("Unknown builder: {builder_str}, use median instead.");
                Builder::EMedian
            }
        };

        info!("Using BVH accelerator: {builder:?} {axis_selection:?}");

        Self {
            axis_selection,
            builder,
            nodes: vec![],
            shapes: Some(vec![]),
            aabbs: vec![],
            emitters: vec![],
        }
    }

    // Fonction récursive pour calculer l'intersection ou non d'un noeud.
    // Le noeud est représenter par l'indice (node_idx)
    #[allow(clippy::needless_range_loop)]
    pub fn hit_bvh<'a>(&'a self, r: &mut Ray, node: &BVHNode) -> Option<Intersection<'a>> {
        /*Pseudo-code:

        ========================
        Si c'est une feuille, calculer l'intersection avec toutes les shapes
        representees par cette feuille (node.prim_count et node.first_prim).
        Ce code ressemble fortement a la version naive dans ShapeGroup.
        Retournez vrai uniquement si vous avez trouver une intersection.

        =======================
        Sinon, verifier l'intersection par rapport aux deux enfants.
        Effectuer l'appel recursif avec l'enfant le plus proche d'abord.
        Pour recuperer cette distance d'intersection:

        let distance = noeud_enfant.aabb.hit(r); // Distance est un Option<f64>


        Apres avoir effectuer l'appel a l'enfant le plus proche, verifier si vous toucher l'enfant le plus loin.
        En effet, r.tmax est mis a jour lors des appels recursifs. Verifier uniquement l'enfant le plus loin
        si il est toujours possible d'avoir un intersection.
        */
        // votrecodeici!("Devoir 2: calcul de l'intersection d'un noeud");
        // match node.info {
        //     BVHNodeInfo::Leaf { frist_primitive, primitive_count } => None,
        //     BVHNodeInfo::Node { left } => None,
        // }

        // SOLUTION
        match node.info {
            BVHNodeInfo::Leaf {
                frist_primitive,
                primitive_count,
            } => {
                let mut hit = None;
                let nodes = self.shapes.as_ref().unwrap();
                for i in frist_primitive..frist_primitive + primitive_count {
                    if self.aabbs[i].hit(r).is_some()
                        && let Some(h) = nodes[i].hit(r)
                    {
                        r.tmax = h.t;
                        hit = Some(h);
                    }
                }
                hit
            }
            BVHNodeInfo::Node { left } => {
                let n1 = &self.nodes[left];
                let n2 = &self.nodes[left + 1];
                let d1 = n1.aabb.hit(r).unwrap_or(f64::MAX);
                let d2 = n2.aabb.hit(r).unwrap_or(f64::MAX);

                let (n1, d1, n2, d2) = if d1 > d2 {
                    (n2, d2, n1, d1)
                } else {
                    (n1, d1, n2, d2)
                };

                let mut hit = if d1 < r.tmax {
                    self.hit_bvh(r, n1)
                } else {
                    None
                };
                if d2 < r.tmax {
                    let new_hit = self.hit_bvh(r, n2);
                    if new_hit.is_some() {
                        hit = new_hit;
                    }
                }

                hit
            }
        }
    }

    /// Subdive the node recursively. Note that aabbs are passed as slices
    /// If you are more confortable, you can change this as &mut Vec<CachedShapeAABB>
    fn subdivide_node(
        &mut self,
        node_idx: usize,
        cached_aabbs: &mut [CachedShapeAABB],
        depth: usize,
    ) {
        // The number of current node
        // this will be usefull to compute the left element (if we transform the current node).
        let nodes_len = self.nodes.len();
        let node = &mut self.nodes[node_idx];

        // Referer vous au cours pour les differentes approches (median, spatial, sah)
        /*
        Pour trier les objets en fonction d'un axe vous pouvez faire:
        aabbs[begin..end].sort_by(|a, b| {
            a.aabb.center()[axis]
                .partial_cmp(&b.aabb.center()[axis])
                .unwrap()
        });

        Pour separer les elements avec un element pivot:
        aabbs[begin..end].select_nth_unstable_by(mid, |a, b| {
                a.aabb.center()[axis]
                    .partial_cmp(&b.aabb.center()[axis])
                    .unwrap()
        });
        Regarder la documentation Rust pour plus d'information.

        Faites tres attention au calcul d'indexes. Les erreurs sont tres courante.

        Lors de la creation des enfants, utilisez BVHNode::new_leaf(...). Attention, vous devez
        mettre en oeuvre le caclul de l'AABB de la nouvelle feuille.

        N'oubliez pas de mettre a jour le noeud subdiviser avec la methode to_node(...)
        */
        // votrecodeici!("Devoir 2: Mettre en oeuvre les differents algorithmes de subdivision");

        // SOLUTION
        let axis = match self.axis_selection {
            AxisSelection::ERoundRobin => depth % 3,
            AxisSelection::ELongest => {
                let d = node.aabb.diagonal();
                if d.x > d.y && d.x > d.z {
                    0
                } else if d.y > d.z {
                    1
                } else {
                    2
                }
            }
        };

        if node.primitive_count() <= 2 {
            // Do not subdivide
            return;
        }

        let nb_prim = node.primitive_count();
        let first_prim = node.frist_primitive();
        node.to_node(nodes_len);

        let offset = match self.builder {
            Builder::EMedian => {
                let offset = nb_prim / 2;
                cached_aabbs[first_prim..(first_prim + nb_prim)].select_nth_unstable_by(
                    offset,
                    |a, b| {
                        a.aabb.center()[axis]
                            .partial_cmp(&b.aabb.center()[axis])
                            .unwrap()
                    },
                );
                offset
            }
            Builder::ESpatial => {
                let index = itertools::partition(
                    &mut cached_aabbs[first_prim..first_prim + nb_prim],
                    |elt| elt.aabb.center()[axis] > node.aabb.center()[axis],
                );
                if index == 0 || index == nb_prim {
                    cached_aabbs[first_prim..(first_prim + nb_prim)].sort_by(|a, b| {
                        a.aabb.center()[axis]
                            .partial_cmp(&b.aabb.center()[axis])
                            .unwrap()
                    });
                    nb_prim / 2 // Central split (median)
                } else {
                    index
                }
            }
            Builder::ESAH => {
                let mut best_pos = 0;
                let mut best_cost = f64::INFINITY;
                let mut best_axis = 3;
                if false {
                    let mut aabb_centroid = AABB::default();
                    for aabb in &cached_aabbs[first_prim..first_prim + nb_prim] {
                        aabb_centroid.extend(aabb.aabb.center());
                    }

                    // 16 bins
                    let mut scores = [0.0; 16 - 1];
                    for o in 0..3 {
                        let delta = (aabb_centroid.max[o] - aabb_centroid.min[o]) / 16.0;

                        // Compute bins
                        let mut bins_count = [0; 16];
                        let mut bins_aabbs = vec![AABB::default(); 16];
                        for aabb in &cached_aabbs[first_prim..first_prim + nb_prim] {
                            let idx = (((aabb.aabb.center()[o] - aabb_centroid.min[o]) / delta)
                                .floor() as usize)
                                .min(15);
                            bins_count[idx] += 1;
                            bins_aabbs[idx] = merge_aabb(&bins_aabbs[idx], &aabb.aabb);
                        }

                        let mut tmp = AABB::default();
                        let mut nb = 0;
                        for id in 0..15 {
                            let id_left = 16 - id - 1;
                            if bins_count[id_left] == 0 {
                                continue;
                            }
                            tmp = merge_aabb(&tmp, &bins_aabbs[id_left]);
                            nb += bins_count[id_left];
                            scores[id_left - 1] = tmp.area() * f64::from(nb);
                        }
                        tmp = AABB::default();
                        nb = 0;
                        for id in 0..15 {
                            if bins_count[id] == 0 {
                                continue;
                            }
                            tmp = merge_aabb(&tmp, &bins_aabbs[id]);
                            nb += bins_count[id];
                            scores[id] += tmp.area() * f64::from(nb);
                            if scores[id] < best_cost {
                                best_cost = scores[id];
                                best_axis = o;
                                best_pos = id + 1;
                            }
                        }
                    }

                    // Sort on the selected axis
                    // aabbs[first_prim..first_prim + nb_prim].sort_by(|a, b| -> Ordering {
                    //     if a.aabb.center()[best_axis] < b.aabb.center()[best_axis] {
                    //         Ordering::Less
                    //     } else {
                    //         Ordering::Greater
                    //     }
                    // });
                    let delta =
                        (aabb_centroid.max[best_axis] - aabb_centroid.min[best_axis]) / 16.0;
                    let split_index = itertools::partition(
                        &mut cached_aabbs[first_prim..first_prim + nb_prim],
                        |elt| {
                            (elt.aabb.center()[best_axis] - aabb_centroid.min[best_axis])
                                > delta * (best_pos as f64 - 0.5)
                        },
                    );
                    if split_index == nb_prim || split_index == 0 {
                        ((nb_prim as f32 * 0.5) as usize).max(1)
                    } else {
                        split_index
                    }
                } else {
                    // Use SAH Sweep
                    {
                        let mut scores = vec![0.0; nb_prim - 1];
                        for o in 0..3 {
                            cached_aabbs[first_prim..first_prim + nb_prim].sort_by(
                                |a, b| -> Ordering {
                                    if a.aabb.center()[o] < b.aabb.center()[o] {
                                        Ordering::Less
                                    } else {
                                        Ordering::Greater
                                    }
                                },
                            );
                            let mut tmp = AABB::default();
                            for id in 0..nb_prim - 1 {
                                let id_left = nb_prim - id - 1;
                                tmp = merge_aabb(&tmp, &cached_aabbs[id_left + first_prim].aabb);
                                scores[id_left - 1] = tmp.area() * (id + 1) as f64;
                            }
                            tmp = AABB::default();
                            for id in 0..nb_prim - 1 {
                                tmp = merge_aabb(&tmp, &cached_aabbs[id + first_prim].aabb);
                                scores[id] += tmp.area() * (id + 1) as f64;
                                if scores[id] < best_cost {
                                    best_cost = scores[id];
                                    best_axis = o;
                                    best_pos = id + 1;
                                }
                            }
                        }
                    }
                    // Sort on the selected axis
                    cached_aabbs[first_prim..first_prim + nb_prim].sort_by(|a, b| -> Ordering {
                        if a.aabb.center()[best_axis] < b.aabb.center()[best_axis] {
                            Ordering::Less
                        } else {
                            Ordering::Greater
                        }
                    });
                    if best_pos == nb_prim || best_pos == 0 {
                        ((nb_prim as f32 * 0.5) as usize).max(1)
                    } else {
                        best_pos
                    }
                }
            }
        };

        self.nodes
            .push(BVHNode::new_leaf(first_prim, offset, cached_aabbs));
        self.nodes.push(BVHNode::new_leaf(
            first_prim + offset,
            nb_prim - offset,
            cached_aabbs,
        ));

        self.subdivide_node(nodes_len, cached_aabbs, depth + 1);
        self.subdivide_node(nodes_len + 1, cached_aabbs, depth + 1);
    }
}

impl Shape for BVH {
    fn add_shape(&mut self, shape: Box<dyn Shape>) {
        self.shapes.as_mut().unwrap().push(shape);
    }

    fn build(&mut self) {
        if !self.nodes.is_empty() {
            warn!("Try to build several time!");
            return;
        }

        // Build the cached aabb representation
        // note that we temporary transform cached_aabbs to None
        // to get the shape ownership (in order to reorganise the vector)
        let mut cached_aabbs = Vec::with_capacity(self.shapes.as_ref().unwrap().len());
        let shapes = self.shapes.take().unwrap();
        for shape in shapes {
            cached_aabbs.push(CachedShapeAABB {
                aabb: shape.aabb(),
                shape,
            });
        }

        // Add root node inside the hierachy
        self.nodes
            .push(BVHNode::new_leaf(0, cached_aabbs.len(), &cached_aabbs));

        // Subdivide recursively the root node ()
        self.subdivide_node(0, &mut cached_aabbs, 0);

        // Copy back the shape ptr
        // indeed, during the build, we might have sorted the shapes
        // thus we need to store the correct order inside the AABB
        self.shapes = Some(cached_aabbs.into_iter().map(|e| e.shape).collect());
        self.aabbs = self
            .shapes
            .as_ref()
            .unwrap()
            .iter()
            .map(|e| e.aabb())
            .collect();
        self.emitters.clear();
        for (i, shape) in self.shapes.as_ref().unwrap().iter().enumerate() {
            if shape.material().have_emission() {
                self.emitters.push(i);
            }
        }
    }

    fn hit<'a>(&'a self, r: &Ray) -> Option<Intersection<'a>> {
        NUMBER_INTERSECTIONS.with(|f| *f.borrow_mut() += 1);
        if self.nodes.is_empty() || self.nodes[0].aabb.hit(r).is_none() {
            None
        } else {
            let mut r = *r;
            self.hit_bvh(&mut r, &self.nodes[0])
        }
    }

    fn aabb(&self) -> AABB {
        if self.nodes.is_empty() {
            AABB::default()
        } else {
            self.nodes[0].aabb.clone()
        }
    }

    fn sample_direct(
        &self,
        p: &crate::vec::Point3,
        sample: &crate::vec::Vec2,
    ) -> (super::EmitterSample, &dyn Shape) {
        let j = (sample.x * self.emitters.len() as f64) as usize;
        let k = self.emitters[j];
        // Rescale random number
        let sample = Vec2::new(
            sample.x.mul_add(self.emitters.len() as f64, -(j as f64)),
            sample.y,
        );
        // Sample shape
        let nodes = self.shapes.as_ref().unwrap();
        let (mut ps, shape) = nodes[k].sample_direct(p, &sample);
        ps.pdf *= 1.0 / self.emitters.len() as f64;
        (ps, shape)
    }

    fn pdf_direct(
        &self,
        shape: &dyn Shape,
        p: &crate::vec::Point3,
        y: &crate::vec::Point3,
        n: &crate::vec::Vec3,
    ) -> crate::Real {
        let pdf = 1.0 / self.emitters.len() as crate::Real;
        pdf * shape.pdf_direct(shape, p, y, n)
    }

    fn material(&self) -> &dyn crate::materials::Material {
        panic!("Shape group does not have a material")
    }
}
