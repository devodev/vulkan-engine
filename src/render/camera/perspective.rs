use cgmath::{Deg, Matrix4, SquareMatrix};

#[derive(Debug, Copy, Clone)]
pub struct CameraPerspective {
    width: usize,
    height: usize,
    aspect_ratio: f32,
    fov: Deg<f32>,
    near: f32,
    far: f32,
    proj: Matrix4<f32>,
}

impl CameraPerspective {
    pub fn new(width: usize, height: usize) -> Self {
        let mut camera = Self { ..Self::default() };
        camera.resize(width, height);
        camera
    }

    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    pub fn projection_matrix(&self) -> Matrix4<f32> {
        self.proj
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.aspect_ratio = width as f32 / height as f32;
        self.compute_projection_matrix()
    }

    fn compute_projection_matrix(&mut self) {
        self.proj = cgmath::perspective(self.fov, self.aspect_ratio, self.near, self.far)
    }
}

impl Default for CameraPerspective {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            aspect_ratio: 0.0,
            fov: Deg(60.0f32),
            near: 0.1,
            far: 10.0,
            proj: Matrix4::identity(),
        }
    }
}
