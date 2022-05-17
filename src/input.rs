use std::collections::HashMap;

use winit::{
    dpi::PhysicalPosition,
    event::{
        DeviceEvent, ElementState, Event, KeyboardInput, MouseScrollDelta, VirtualKeyCode,
        WindowEvent,
    },
};

#[derive(Default)]
struct ScrollState {
    x: f32,
    y: f32,
}

pub struct InputSystem {
    keyboard: HashMap<VirtualKeyCode, ElementState>,
    scroll_state: ScrollState,
}

impl InputSystem {
    pub fn new() -> Self {
        Self {
            keyboard: HashMap::new(),
            scroll_state: ScrollState::default(),
        }
    }

    pub fn reset(&mut self) {
        self.scroll_state = ScrollState::default();
    }

    pub fn on_event(&mut self, event: &Event<()>) {
        #[allow(clippy::single_match)]
        #[allow(clippy::collapsible_match)]
        match event {
            Event::WindowEvent { ref event, .. } => match *event {
                // handle keys
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state,
                            virtual_keycode,
                            ..
                        },
                    ..
                } => {
                    if let Some(keycode) = virtual_keycode {
                        self.keyboard.insert(keycode, state);
                    }
                }
                // reset state when losing focus
                WindowEvent::Focused(false) => {
                    self.keyboard.clear();
                    self.scroll_state = ScrollState::default();
                }
                _ => {}
            },
            Event::DeviceEvent { ref event, .. } => match *event {
                // handle mouse scroll
                DeviceEvent::MouseWheel {
                    delta: MouseScrollDelta::LineDelta(delta_x, delta_y),
                } => {
                    if delta_x != 0.0 {
                        self.scroll_state.x = delta_x.signum();
                    }
                    if delta_y != 0.0 {
                        self.scroll_state.y = delta_y.signum();
                    }
                }
                DeviceEvent::MouseWheel {
                    delta: MouseScrollDelta::PixelDelta(PhysicalPosition { x, y }),
                } => {
                    if x != 0.0 {
                        self.scroll_state.x = x.signum() as f32;
                    }
                    if y != 0.0 {
                        self.scroll_state.y = y.signum() as f32;
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    pub fn is_key_pressed(&self, key: VirtualKeyCode) -> bool {
        match self.keyboard.get(&key) {
            Some(state) => state == &ElementState::Pressed,
            None => false,
        }
    }
    pub fn is_key_released(&self, key: VirtualKeyCode) -> bool {
        match self.keyboard.get(&key) {
            Some(state) => state == &ElementState::Released,
            None => true,
        }
    }

    pub fn mouse_scoll_x(&self) -> f32 {
        self.scroll_state.x
    }
    pub fn mouse_scoll_y(&self) -> f32 {
        self.scroll_state.y
    }
}
