pub struct Array2d<T> {
    data: Vec<T>,
    size_x: u32,
    size_y: u32,
}

impl<T: Clone> Array2d<T> {
    pub fn new() -> Self {
        Array2d {
            data: Vec::new(),
            size_x: 0,
            size_y: 0,
        }
    }

    pub fn with_size(size_x: u32, size_y: u32, value: T) -> Self {
        let mut m_data = Vec::with_capacity((size_x * size_y) as usize);
        m_data.resize((size_x * size_y) as usize, value);
        Array2d {
            data: m_data,
            size_x,
            size_y,
        }
    }

    pub fn copy_from(other: &Array2d<T>) -> Self {
        Array2d {
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

    pub fn get_index_1d(&self, x: u32, y: u32) -> usize {
        y as usize * self.size_x as usize + x as usize
    }

    pub fn get_index_2d(&self, i: u32) -> (u32, u32) {
        (i % self.size_x, i / self.size_x)
    }

    pub fn width(&self) -> u32 {
        self.size_x
    }

    pub fn height(&self) -> u32 {
        self.size_y
    }

    pub fn size(&self) -> u32 {
        self.size_x * self.size_y
    }

    pub fn size_x(&self) -> u32 {
        self.size_x
    }

    pub fn size_y(&self) -> u32 {
        self.size_y
    }

    pub fn at(&self, x: u32, y: u32) -> &T {
        &self.data[self.get_index_1d(x, y)]
    }

    pub fn at_mut(&mut self, x: u32, y: u32) -> &mut T {
        let index = self.get_index_1d(x, y);
        &mut self.data[index]
    }

    pub fn uv(&self, x: f64, y: f64) -> &T {
        let x = ((x * self.size_x as f64) as u32).min(self.size_x - 1);
        let y = ((y * self.size_y as f64) as u32).min(self.size_y - 1);
        &self.data[self.get_index_1d(x, y)]
    }

    pub fn uv_mut(&mut self, x: f64, y: f64) -> &mut T {
        let x = ((x * self.size_x as f64) as u32).min(self.size_x - 1);
        let y = ((y * self.size_y as f64) as u32).min(self.size_y - 1);
        let index = self.get_index_1d(x, y);
        &mut self.data[index]
    }

    pub fn flip_vertically(&mut self) {
        let mut flip_image = Array2d::copy_from(self);
        for x in 0..self.size_x {
            for y in 0..self.size_y {
                *flip_image.at_mut(x, y) = self.at(x, self.size_y - 1 - y).clone();
            }
        }
        *self = flip_image;
    }
}

impl<T: Clone> Default for Array2d<T> {
    fn default() -> Self {
        Self::new()
    }
}
