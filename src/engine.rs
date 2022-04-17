use log::debug;
use std::error::Error;

use gameloop::GameLoop;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

use crate::render::Renderer;

type MyResult<T> = Result<T, Box<dyn Error>>;

pub struct Engine {
    renderer: Option<Renderer>,
}

impl Engine {
    pub fn new() -> Self {
        Engine { renderer: None }
    }

    pub fn run(&mut self) -> MyResult<()> {
        let (event_loop, window) = self.init_window()?;
        self.init_renderer(window)?;

        // flags
        let mut window_resized = false;
        let mut recreate_swapchain = false;

        let mut renderer = self
            .renderer
            .take()
            .ok_or("Couldnt take renderer. Did you call self.init_renderer?")?;

        // run gameloop at 20 tps with a max frameskip of 5
        let tps = 20;
        let max_frameskip = 5;
        let game_loop = GameLoop::new(tps, max_frameskip)?;

        debug!("start event loop");
        event_loop.run(move |event, _, control_flow| match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                window_resized = true;
            }
            Event::MainEventsCleared => {
                // NOTE: the MainEventsCleared event "will be emitted when all input events
                //       have been processed and redraw processing is about to begin".
                for action in game_loop.actions() {
                    match action {
                        gameloop::FrameAction::Tick => {
                            // todo!("update state")
                        }
                        gameloop::FrameAction::Render { interpolation: _ } => {
                            renderer.render(&mut window_resized, &mut recreate_swapchain)
                        }
                    }
                }
            }
            _ => (),
        });
    }

    fn init_window(&mut self) -> MyResult<(EventLoop<()>, Window)> {
        debug!("init_window");
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new().build(&event_loop)?;

        Ok((event_loop, window))
    }

    fn init_renderer(&mut self, window: Window) -> MyResult<()> {
        debug!("init_vulkan");

        let renderer = Renderer::new(window)?;
        self.renderer = Some(renderer);

        Ok(())
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
