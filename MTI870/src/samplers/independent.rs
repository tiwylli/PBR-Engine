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
        self.rnd.gen()
    }

    fn next2d(&mut self) -> Vec2 {
        Vec2::new(self.rnd.gen(), self.rnd.gen())
    }

    fn clone_box(&mut self) -> Box<dyn Sampler> {
        Box::new(Independent {
            rnd: ChaCha8Rng::seed_from_u64(self.rnd.gen()),
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
    pub fn new(nspp: usize) -> Self {
        Independent {
            rnd: ChaCha8Rng::seed_from_u64(0),
            nspp,
        }
    }
}
