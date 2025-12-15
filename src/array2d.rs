use itertools::Itertools;

use crate::vec::Vec3;

pub struct Array2d<T> {
    data: Vec<T>,
    size_x: u32,
    size_y: u32,
}

impl<T: Clone> Array2d<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            data: Vec::new(),
            size_x: 0,
            size_y: 0,
        }
    }

    pub fn with_size(size_x: u32, size_y: u32, value: T) -> Self {
        let mut m_data = Vec::with_capacity((size_x * size_y) as usize);
        m_data.resize((size_x * size_y) as usize, value);
        Self {
            data: m_data,
            size_x,
            size_y,
        }
    }

    #[must_use]
    pub fn copy_from(other: &Self) -> Self {
        Self {
            data: other.data.clone(),
            size_x: other.size_x,
            size_y: other.size_y,
        }
    }

    pub fn set_size(&mut self, size_x: u32, size_y: u32, value: T) {
        if size_x == self.size_x && size_y == self.size_y {
            return;
        }

        self.data.resize((size_x * size_y) as usize, value);
        self.size_x = size_x;
        self.size_y = size_y;
    }

    pub fn reset(&mut self, value: T) {
        for i in &mut self.data {
            *i = value.clone();
        }
    }

    #[must_use]
    pub const fn get_index_1d(&self, x: u32, y: u32) -> usize {
        y as usize * self.size_x as usize + x as usize
    }

    #[must_use]
    pub const fn get_index_2d(&self, i: u32) -> (u32, u32) {
        (i % self.size_x, i / self.size_x)
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.size_x
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.size_y
    }

    #[must_use]
    pub const fn size(&self) -> u32 {
        self.size_x * self.size_y
    }

    #[must_use]
    pub const fn size_x(&self) -> u32 {
        self.size_x
    }

    #[must_use]
    pub const fn size_y(&self) -> u32 {
        self.size_y
    }

    #[must_use]
    pub fn at(&self, x: u32, y: u32) -> &T {
        &self.data[self.get_index_1d(x, y)]
    }

    pub fn at_mut(&mut self, x: u32, y: u32) -> &mut T {
        let index = self.get_index_1d(x, y);
        &mut self.data[index]
    }

    #[must_use]
    pub fn uv(&self, x: f64, y: f64) -> &T {
        let x = ((x * f64::from(self.size_x)) as u32).min(self.size_x - 1);
        let y = ((y * f64::from(self.size_y)) as u32).min(self.size_y - 1);
        &self.data[self.get_index_1d(x, y)]
    }

    #[must_use]
    pub fn uv_mut(&mut self, x: f64, y: f64) -> &mut T {
        let x = ((x * f64::from(self.size_x)) as u32).min(self.size_x - 1);
        let y = ((y * f64::from(self.size_y)) as u32).min(self.size_y - 1);
        let index = self.get_index_1d(x, y);
        &mut self.data[index]
    }

    pub fn flip_vertically(&mut self) {
        let mut flip_image = Self::copy_from(self);
        for x in 0..self.size_x {
            for y in 0..self.size_y {
                *flip_image.at_mut(x, y) = self.at(x, self.size_y - 1 - y).clone();
            }
        }
        *self = flip_image;
    }
}

impl Array2d<Vec3> {
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.data
            .iter()
            .all(|vec| vec.x.is_finite() && vec.y.is_finite() && vec.z.is_finite())
    }
}

impl<T: Clone> Default for Array2d<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl Array2d<Vec3> {
    #[must_use]
    pub fn from_flat(size_x: u32, size_y: u32, array: &[f32]) -> Self {
        let data = array
            .iter()
            .tuples()
            .map(|tup: (&f32, &f32, &f32)| {
                Vec3::new(f64::from(*tup.0), f64::from(*tup.1), f64::from(*tup.2))
            })
            .collect::<Vec<_>>();

        Self {
            data,
            size_x,
            size_y,
        }
    }
}

#[must_use]
pub fn flattened_arr_vec3(array: &Array2d<Vec3>) -> Vec<f32> {
    array
        .data
        .iter()
        .flat_map(|vec3| [vec3.x as f32, vec3.y as f32, vec3.z as f32])
        .collect()
}
