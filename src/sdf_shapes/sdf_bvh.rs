use std::{cmp::Ordering, sync::Arc};

use crate::{
    aabb::{AABB, merge_aabb},
    ray::Ray,
    vec::Point3,
};

use super::sdf_object::SDFObject;

struct Primitive {
    bounds: AABB,
    centroid: Point3,
    object: Arc<dyn SDFObject>,
}

#[derive(Clone)]
struct BvhNode {
    bounds: AABB,
    kind: BvhNodeKind,
}

#[derive(Clone)]
enum BvhNodeKind {
    Leaf { start: usize, count: usize },
    Interior { left: usize, right: usize },
}

/// Simple BVH over SDF objects to avoid marching every primitive for every ray.
pub struct SdfBvh {
    nodes: Vec<BvhNode>,
    objects: Vec<Arc<dyn SDFObject>>,
}

impl SdfBvh {
    #[must_use]
    pub fn build(objects: &[Arc<dyn SDFObject>]) -> Option<Self> {
        if objects.len() <= 1 {
            return None;
        }

        let mut primitives = Vec::with_capacity(objects.len());
        for obj in objects {
            let bounds = obj.world_bounds();
            if !bounds.min.x.is_finite()
                || !bounds.min.y.is_finite()
                || !bounds.min.z.is_finite()
                || !bounds.max.x.is_finite()
                || !bounds.max.y.is_finite()
                || !bounds.max.z.is_finite()
            {
                // Degenerate bounds make the BVH unreliable; fall back to linear traversal.
                return None;
            }
            primitives.push(Primitive {
                centroid: bounds.center(),
                bounds,
                object: Arc::clone(obj),
            });
        }

        if primitives.len() <= 1 {
            return None;
        }

        let mut nodes = Vec::new();
        let mut ordered_objects = Vec::with_capacity(primitives.len());
        Self::build_recursive(&mut primitives, &mut nodes, &mut ordered_objects);
        Some(Self {
            nodes,
            objects: ordered_objects,
        })
    }

    fn build_recursive(
        primitives: &mut [Primitive],
        nodes: &mut Vec<BvhNode>,
        ordered_objects: &mut Vec<Arc<dyn SDFObject>>,
    ) -> usize {
        let mut bounds = AABB::default();
        for prim in primitives.iter() {
            bounds = merge_aabb(&bounds, &prim.bounds);
        }

        if primitives.len() <= 2 {
            let start = ordered_objects.len();
            for prim in primitives.iter() {
                ordered_objects.push(Arc::clone(&prim.object));
            }
            let index = nodes.len();
            nodes.push(BvhNode {
                bounds,
                kind: BvhNodeKind::Leaf {
                    start,
                    count: primitives.len(),
                },
            });
            return index;
        }

        let mut centroid_bounds = AABB::default();
        for prim in primitives.iter() {
            centroid_bounds.extend(prim.centroid);
        }
        let diag = centroid_bounds.diagonal();

        let axis = if diag.x >= diag.y && diag.x >= diag.z {
            0
        } else if diag.y >= diag.z {
            1
        } else {
            2
        };

        if diag[axis].abs() < f64::EPSILON {
            // All centroids collapsed along the split axis; turn this into a leaf.
            let start = ordered_objects.len();
            for prim in primitives.iter() {
                ordered_objects.push(Arc::clone(&prim.object));
            }
            let index = nodes.len();
            nodes.push(BvhNode {
                bounds,
                kind: BvhNodeKind::Leaf {
                    start,
                    count: primitives.len(),
                },
            });
            return index;
        }

        primitives.sort_unstable_by(|a, b| {
            a.centroid[axis]
                .partial_cmp(&b.centroid[axis])
                .unwrap_or(Ordering::Equal)
        });
        let mid = primitives.len() / 2;
        let (left, right) = primitives.split_at_mut(mid);

        let node_index = nodes.len();
        nodes.push(BvhNode {
            bounds: bounds.clone(),
            kind: BvhNodeKind::Leaf { start: 0, count: 0 },
        });
        let left_index = Self::build_recursive(left, nodes, ordered_objects);
        let right_index = Self::build_recursive(right, nodes, ordered_objects);
        nodes[node_index] = BvhNode {
            bounds,
            kind: BvhNodeKind::Interior {
                left: left_index,
                right: right_index,
            },
        };
        node_index
    }

    pub fn traverse<F>(&self, ray: &Ray, visitor: &mut F)
    where
        F: FnMut(&Arc<dyn SDFObject>),
    {
        if self.nodes.is_empty() {
            return;
        }

        let mut stack = vec![0usize];
        while let Some(index) = stack.pop() {
            let node = &self.nodes[index];
            if node.bounds.hit(ray).is_none() {
                continue;
            }
            match &node.kind {
                BvhNodeKind::Leaf { start, count } => {
                    for sdf in &self.objects[*start..(*start + *count)] {
                        visitor(sdf);
                    }
                }
                BvhNodeKind::Interior { left, right } => {
                    stack.push(*right);
                    stack.push(*left);
                }
            }
        }
    }
}
