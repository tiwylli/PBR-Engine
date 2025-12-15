use crate::{
    Real,
    aabb::{AABB, merge_aabb},
    materials::Material,
    ray::Ray,
    shapes::EmitterSample,
    vec::{Point3, Vec2, Vec3},
};

use super::{Intersection, Shape};

#[derive(Default)]
pub struct ShapeGroup {
    shapes: Vec<Box<dyn Shape>>,
    emitters: Vec<usize>,
}

impl Shape for ShapeGroup {
    fn hit<'a>(&'a self, r: &Ray) -> Option<Intersection<'a>> {
        /*
        Mettre en œuvre l'intersection de plusieurs primitives comme nous l'avons vu en cours.
        Les principales étapes sont:
        1) Copier le ray (r) dans un rayon temporaire que vous allez pouvoir modifier
        2) Pour toutes les formes:
            -   Tester s’il y a intersection. Si oui, mettre à jour le tmax du rayon.
        3) retourner vrai si on a trouvé au moins une intersection, sinon retourner faux
        */
        let mut closest = None;
        let mut r = *r;

        for shape in &self.shapes {
            if let Some(intersection) = shape.hit(&r)
                && intersection.t
                    < closest
                        .as_ref()
                        .map_or(f64::MAX, |i: &Intersection<'_>| i.t)
            {
                r.tmax = intersection.t;
                closest = Some(intersection);
            }
        }

        closest
    }

    #[allow(clippy::suboptimal_flops)]
    fn sample_direct(&self, p: &Point3, sample: &Vec2) -> (EmitterSample, &dyn Shape) {
        let j = (sample.x * self.emitters.len() as f64) as usize;
        let k = self.emitters[j];

        // Rescale random number
        let sample = Vec2::new(sample.x * self.emitters.len() as f64 - j as f64, sample.y);

        // Sample shape
        let (mut ps, shape) = self.shapes[k].sample_direct(p, &sample);
        ps.pdf /= self.emitters.len() as f64; // Update PDF
        (ps, shape)
    }

    fn pdf_direct(&self, shape: &dyn Shape, p: &Point3, y: &Point3, n: &Vec3) -> Real {
        let pdf = 1.0 / self.emitters.len() as Real;
        pdf * shape.pdf_direct(shape, p, y, n)
    }

    fn material(&self) -> &dyn Material {
        panic!("Cannot call .material() on a ShapeGroup!");
    }

    fn add_shape(&mut self, shape: Box<dyn Shape>) {
        if shape.material().have_emission() {
            self.emitters.push(self.shapes.len());
        }
        self.shapes.push(shape);
    }

    fn build(&mut self) {}

    fn aabb(&self) -> AABB {
        let mut aabb = AABB::default();
        for s in &self.shapes {
            aabb = merge_aabb(&aabb, &s.aabb());
        }
        aabb
    }
}
