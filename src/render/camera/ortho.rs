use cgmath::{Matrix4, SquareMatrix};

#[derive(Debug, Copy, Clone)]
pub struct CameraOrthographic {
    width: u32,
    height: u32,
    aspect_ratio: f32,
    zoom_base: f32,
    zoom: f32,
    near: f32,
    far: f32,
    proj: Matrix4<f32>,
}

impl CameraOrthographic {
    pub fn new(width: u32, height: u32) -> Self {
        let mut camera = Self { ..Self::default() };
        camera.resize(width, height);
        camera
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn projection_matrix(&self) -> Matrix4<f32> {
        self.proj
    }

    pub fn set_zoom(&mut self, amount: f32) {
        self.zoom += amount;
        if self.zoom < 0.0 {
            self.zoom = 0.0;
        }
        self.compute_projection_matrix()
    }

    pub fn reset_zoom(&mut self) {
        self.zoom = self.zoom_base;
        self.compute_projection_matrix()
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.aspect_ratio = width as f32 / height as f32;
        self.compute_projection_matrix()
    }

    fn compute_projection_matrix(&mut self) {
        self.proj = cgmath::ortho(
            -self.aspect_ratio * self.zoom,
            self.aspect_ratio * self.zoom,
            -self.zoom,
            self.zoom,
            self.near,
            self.far,
        )
    }
}

impl Default for CameraOrthographic {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            aspect_ratio: 0.0,
            zoom_base: 1.0,
            zoom: 1.0,
            near: 0.1,
            far: 10.0,
            proj: Matrix4::identity(),
        }
    }
}
