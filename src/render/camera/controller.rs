use std::{
    ops::{Add, Mul, Sub},
    time::Duration,
};

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

    zoom_target: f32,
    zoom_min: f32,
    zoom_max: f32,
    zoom_sensitivity: f32,
}

impl CameraController {
    pub fn new(camera: CameraOrthographic) -> Self {
        let mut controller = Self {
            speed_base: 1.0,
            pos: Vector3::new(0.0, 0.0, 2.0),
            target: Vector3::new(0.0, 0.0, -1.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            camera,
            view: Matrix4::identity(),
            zoom_target: camera.zoom(),
            zoom_min: 0.1,
            zoom_max: 5.0,
            zoom_sensitivity: 0.08,
        };
        controller.compute_view_matrix();
        controller
    }

    pub fn on_update(&mut self, ctx: Context, delta: Duration) {
        self.compute_view_matrix();

        let speed = self.speed_base * delta.as_secs_f32();

        // move camera => Z
        if ctx.is_key_pressed(VirtualKeyCode::Q) {
            self.move_backward(speed)
        }
        if ctx.is_key_pressed(VirtualKeyCode::E) {
            self.move_forward(speed)
        }
        // move camera => Y
        if ctx.is_key_pressed(VirtualKeyCode::W) {
            self.move_up(speed)
        }
        if ctx.is_key_pressed(VirtualKeyCode::S) {
            self.move_down(speed)
        }
        // move camera => X
        if ctx.is_key_pressed(VirtualKeyCode::A) {
            self.move_left(speed)
        }
        if ctx.is_key_pressed(VirtualKeyCode::D) {
            self.move_right(speed)
        }

        // on scroll, update zoom_target
        self.zoom_target -= ctx.mouse_scoll_y() * self.zoom_sensitivity;

        // clamp zoom target between min and max
        self.zoom_target = clamp(self.zoom_target, self.zoom_min, self.zoom_max);

        // move camera (lerp) towards zoom_target
        if (self.camera.zoom() - self.zoom_target).abs() > 0.1 {
            let zoom_amount = lerp(self.camera.zoom(), self.zoom_target, speed * 3.0);
            self.camera.set_zoom(zoom_amount);
        }

        // reset zoom
        if ctx.is_key_pressed(VirtualKeyCode::Z) {
            self.camera.reset_zoom();
            self.zoom_target = self.camera.zoom();
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

fn clamp(v: f32, min: f32, max: f32) -> f32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

// fn lerp(start: f32, end: f32, amount: f32) -> f32 {
//     start * (1.0 - amount) + end * amount
// }

fn lerp(start: f32, end: f32, amount: f32) -> f32 {
    start + (end - start) * amount
}

fn ease_in(amount: f32) -> f32 {
    amount * amount
}

fn flip(amount: f32) -> f32 {
    1.0 - amount
}

fn smoothstep(amount: f32) -> f32 {
    amount * amount * (3.0 - 2.0 * amount)
}
