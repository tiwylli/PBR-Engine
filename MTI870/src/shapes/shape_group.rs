use crate::NUMBER_INTERSECTIONS;
use crate::ray::Ray;
use crate::aabb::{AABB, merge_aabb};
use super::{Intersection, Shape};

#[derive(Default)]
pub struct ShapeGroup {
    shapes: Vec<Box<dyn Shape>>,
    emitters: Vec<usize>,
}

impl ShapeGroup {
    pub fn add_shape(&mut self, shape: Box<dyn Shape>) {
        // push shape and register it as emitter if its material emits
        let idx = self.shapes.len();
        self.shapes.push(shape);
        if self.shapes[idx].material().have_emission() {
            self.emitters.push(idx);
        }
    }
}

#[derive(Default)]
pub struct SimpleShapeGroup {
    inner: ShapeGroup,
}

impl SimpleShapeGroup {
    pub fn add_shape(&mut self, shape: Box<dyn Shape>) {
        self.inner.add_shape(shape);
    }

    pub fn build(&mut self) {
        // Placeholder for compatibility with older assignments/tests
    }
}

impl Shape for ShapeGroup {
    fn hit<'a>(&'a self, r: &Ray) -> Option<Intersection<'a>> {
        NUMBER_INTERSECTIONS.with(|f| *f.borrow_mut() += 1);
        /*
        Mettre en œuvre l'intersection de plusieurs primitives comme nous l'avons vu en cours.
        Les principales étapes sont:
        1) Copier le ray (r) dans un rayon temporaire que vous allez pouvoir modifier
        2) Pour toutes les formes:
            -   Tester s’il y a intersection. Si oui, mettre à jour le tmax du rayon.
        3) retourner vrai si on a trouvé au moins une intersection, sinon retourner faux
        */
        // votrecodeici!("Devoir 1: intersection d'un groupe de formes");
        let mut ray_copy = r.clone();
        let mut closest_intersection = None;
        for s in &self.shapes {
            if let Some(intersec) = s.hit(&ray_copy) {
                if intersec.t < ray_copy.tmax {
                    ray_copy.tmax = intersec.t;
                    closest_intersection = Some(intersec);
                }
            }
        }
        return closest_intersection;
    }

    fn sample_direct(
        &self,
        p: &crate::vec::Point3,
        sample: &crate::vec::Vec2,
    ) -> (super::EmitterSample, &dyn Shape) {
        let j = (sample.x * self.emitters.len() as f64) as usize;
        let k = self.emitters[j];
        // Rescale random number so x is back in [0,1]
        let sample =
            crate::vec::Vec2::new(sample.x * self.emitters.len() as f64 - j as f64, sample.y);
        // Sample the selected emitter shape
        let (mut ps, shape) = self.shapes[k].sample_direct(p, &sample);
        ps.pdf *= 1.0 / self.emitters.len() as f64; // Update PDF
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
        if self.shapes.is_empty() {
            panic!("ShapeGroup::material() called on empty ShapeGroup");
        }
        self.shapes[0].material()
    }

    fn build(&mut self) {}

    fn add_shape(&mut self, shape: Box<dyn Shape>) {
        self.shapes.push(shape);
    }

    fn aabb(&self) -> AABB {
        let mut aabb = AABB::default();
        for shape in &self.shapes {
            aabb = merge_aabb(&aabb, &shape.aabb());
        }
        aabb
    }
}

impl Shape for SimpleShapeGroup {
    fn hit<'a>(&'a self, r: &Ray) -> Option<Intersection<'a>> {
        self.inner.hit(r)
    }

    fn sample_direct(
        &self,
        p: &crate::vec::Point3,
        sample: &crate::vec::Vec2,
    ) -> (super::EmitterSample, &dyn Shape) {
        self.inner.sample_direct(p, sample)
    }

    fn pdf_direct(
        &self,
        shape: &dyn Shape,
        p: &crate::vec::Point3,
        y: &crate::vec::Point3,
        n: &crate::vec::Vec3,
    ) -> crate::Real {
        self.inner.pdf_direct(shape, p, y, n)
    }

    fn material(&self) -> &dyn crate::materials::Material {
        self.inner.material()
    }

    fn add_shape(&mut self, _: Box<dyn Shape>) {}
    fn build(&mut self) {}
    fn aabb(&self) -> AABB {
        self.inner.aabb()
    }
}
