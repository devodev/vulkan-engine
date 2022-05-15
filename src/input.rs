use std::collections::HashMap;

use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};

pub struct InputSystem {
    keyboard: HashMap<VirtualKeyCode, ElementState>,
}

impl InputSystem {
    pub fn new() -> Self {
        Self {
            keyboard: HashMap::new(),
        }
    }

    pub fn on_event(&mut self, event: &Event<()>) {
        #[allow(clippy::single_match)]
        #[allow(clippy::collapsible_match)]
        match event {
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
            } => {
                if let Some(keycode) = virtual_keycode {
                    self.keyboard.insert(*keycode, *state);
                }
            }
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
}
