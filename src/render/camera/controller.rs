use std::ops::{Add, Mul, Sub};

use cgmath::{EuclideanSpace, InnerSpace, Matrix4, Point3, SquareMatrix, Vector3};
use winit::event::VirtualKeyCode;

use super::ortho::CameraOrthographic;
use crate::Context;

const HORIZONTAL_VEC: Vector3<f32> = Vector3::new(1.0, 0.0, 0.0);
const VERTICAL_VEC: Vector3<f32> = Vector3::new(0.0, 1.0, 0.0);

#[derive(Debug, Clone)]
pub struct CameraController {
    speed_base: f32,
    pos: Vector3<f32>,
    target: Vector3<f32>,
    up: Vector3<f32>,
    camera: CameraOrthographic,
    view: Matrix4<f32>,
}

impl CameraController {
    pub fn new(camera: CameraOrthographic) -> Self {
        let mut controller = Self {
            speed_base: 0.1,
            pos: Vector3::new(0.0, 0.0, 2.0),
            target: Vector3::new(0.0, 0.0, -1.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            camera,
            view: Matrix4::identity(),
        };
        controller.compute_view_matrix();
        controller
    }

    pub fn on_update(&mut self, ctx: Context) {
        self.compute_view_matrix();

        // move camera
        if ctx.is_key_pressed(VirtualKeyCode::Q) {
            self.move_backward(self.speed_base)
        }
        if ctx.is_key_pressed(VirtualKeyCode::E) {
            self.move_forward(self.speed_base)
        }
        if ctx.is_key_pressed(VirtualKeyCode::W) {
            self.move_up(self.speed_base)
        }
        if ctx.is_key_pressed(VirtualKeyCode::S) {
            self.move_down(self.speed_base)
        }
        if ctx.is_key_pressed(VirtualKeyCode::A) {
            self.move_left(self.speed_base)
        }
        if ctx.is_key_pressed(VirtualKeyCode::D) {
            self.move_right(self.speed_base)
        }
    }

    pub fn view_projection_matrix(&self) -> Matrix4<f32> {
        self.camera.projection_matrix().mul(self.view)
    }

    fn compute_view_matrix(&mut self) {
        self.view = Matrix4::look_at_rh(
            Point3::from_vec(self.pos),
            Point3::from_vec(self.pos.add(self.target)),
            self.up,
        )
    }

    fn move_forward(&mut self, speed: f32) {
        self.pos = self.pos.add(self.target.mul(speed))
    }
    fn move_backward(&mut self, speed: f32) {
        self.pos = self.pos.sub(self.target.mul(speed))
    }

    fn move_left(&mut self, speed: f32) {
        self.pos = self
            .pos
            .sub(self.target.normalize().cross(VERTICAL_VEC).mul(speed))
    }
    fn move_right(&mut self, speed: f32) {
        self.pos = self
            .pos
            .add(self.target.normalize().cross(VERTICAL_VEC).mul(speed))
    }
    fn move_up(&mut self, speed: f32) {
        self.pos = self
            .pos
            .sub(self.target.normalize().cross(HORIZONTAL_VEC).mul(speed))
    }
    fn move_down(&mut self, speed: f32) {
        self.pos = self
            .pos
            .add(self.target.normalize().cross(HORIZONTAL_VEC).mul(speed))
    }
}
