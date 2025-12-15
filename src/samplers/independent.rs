use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::vec::Vec2;

use super::Sampler;

pub struct Independent {
    rnd: ChaCha8Rng,
    nspp: usize,
}

impl Sampler for Independent {
    fn next(&mut self) -> f64 {
        self.rnd.random()
    }

    fn next2d(&mut self) -> Vec2 {
        Vec2::new(self.rnd.random(), self.rnd.random())
    }

    fn clone_box(&mut self) -> Box<dyn Sampler> {
        Box::new(Self {
            rnd: ChaCha8Rng::seed_from_u64(self.rnd.random()),
            nspp: self.nspp,
        })
    }

    fn nb_samples(&self) -> usize {
        self.nspp
    }

    fn set_nb_samples(&mut self, nspp: usize) {
        self.nspp = nspp;
    }
}

impl Independent {
    #[must_use]
    pub fn new(nspp: usize) -> Self {
        Self {
            rnd: ChaCha8Rng::seed_from_u64(0),
            nspp,
        }
    }
}
