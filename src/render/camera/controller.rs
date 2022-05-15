use std::ops::{Add, Mul, Sub};

use cgmath::{EuclideanSpace, InnerSpace, Matrix4, Point3, SquareMatrix, Vector3};
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};

use super::ortho::CameraOrthographic;

const HORIZONTAL_VEC: Vector3<f32> = Vector3::new(1.0, 0.0, 0.0);
const VERTICAL_VEC: Vector3<f32> = Vector3::new(0.0, 1.0, 0.0);

// Vulkan clip space has inverted Y and half Z.
#[rustfmt::skip]
const VULKAN_TO_GL_PROJ: Matrix4<f32> = Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, -1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

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

    pub fn on_update(&mut self, event: &Event<()>) {
        self.compute_view_matrix();
        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => self.camera.resize(size.width, size.height),
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state,
                                virtual_keycode,
                                ..
                            },
                        ..
                    },
                ..
            } => match state {
                ElementState::Pressed => match virtual_keycode {
                    Some(VirtualKeyCode::Q) => self.move_backward(self.speed_base),
                    Some(VirtualKeyCode::E) => self.move_forward(self.speed_base),
                    Some(VirtualKeyCode::W) => self.move_up(self.speed_base),
                    Some(VirtualKeyCode::S) => self.move_down(self.speed_base),
                    Some(VirtualKeyCode::A) => self.move_left(self.speed_base),
                    Some(VirtualKeyCode::D) => self.move_right(self.speed_base),
                    _ => (),
                },
                ElementState::Released => {}
            },
            _ => (),
        }
    }

    pub fn view_projection_matrix(&self) -> Matrix4<f32> {
        // Pre-multiply projection matrix with this magix matrix
        // to adapt to Vulkan coordinate system.
        //
        // It involves flipping Y to point downwards and moving
        // depth range from 0 <-> 1 to -1 <-> 1.
        //
        // This avoids doing it on the GPU with:
        //   account for vulkan Y pointing downwards
        //   gl_Position.y = -gl_Position.y;
        //   account for vulkan depth range being 0.0<->1.0
        //   gl_Position.z = (gl_Position.z + gl_Position.w) / 2.0;
        //
        // ref: https://matthewwellings.com/blog/the-new-vulkan-coordinate-system/
        let proj = VULKAN_TO_GL_PROJ.mul(self.camera.projection_matrix());
        proj.mul(self.view)
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
