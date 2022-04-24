use std::{error::Error, result, sync::Arc};

use gameloop::GameLoop;
use log::debug;
use winit::{
    dpi::{LogicalSize, Size},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, Icon, Window, WindowBuilder},
};

use crate::render::Renderer;

type Result<T> = result::Result<T, Box<dyn Error>>;

#[derive(Debug, Clone)]
pub struct EngineBuilder {
    window_size: Option<Size>,
    window_title: Option<String>,
    window_resizable: bool,
    window_fullscreen: Option<Fullscreen>,
    window_maximized: bool,
    window_visible: bool,
    window_icon: Option<Icon>,
    renderer_debug: bool,
}

impl EngineBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_window_size(mut self, s: Size) -> Self {
        self.window_size = Some(s);
        self
    }

    pub fn with_window_title(mut self, s: String) -> Self {
        self.window_title = Some(s);
        self
    }

    pub fn with_window_resizable(mut self, b: bool) -> Self {
        self.window_resizable = b;
        self
    }

    pub fn with_window_fullscreen(mut self, f: Fullscreen) -> Self {
        self.window_fullscreen = Some(f);
        self
    }

    pub fn with_window_maximized(mut self, b: bool) -> Self {
        self.window_maximized = b;
        self
    }

    pub fn with_window_visible(mut self, b: bool) -> Self {
        self.window_visible = b;
        self
    }

    pub fn with_window_icon(mut self, i: Icon) -> Self {
        self.window_icon = Some(i);
        self
    }

    pub fn with_renderer_debug(mut self, b: bool) -> Self {
        self.renderer_debug = b;
        self
    }

    pub fn build(&mut self) -> Engine {
        let mut wb = WindowBuilder::new()
            .with_min_inner_size(Size::Logical(LogicalSize::new(320.0, 240.0)))
            .with_resizable(self.window_resizable)
            .with_fullscreen(self.window_fullscreen.take())
            .with_maximized(self.window_maximized)
            .with_visible(self.window_visible)
            .with_window_icon(self.window_icon.take());

        if let Some(window_size) = self.window_size {
            wb = wb.with_inner_size(window_size);
        }
        if let Some(window_title) = &self.window_title {
            wb = wb.with_title(window_title);
        }

        Engine::new(wb, self.renderer_debug)
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self {
            window_size: None,
            window_title: Some("Vulkan Engine".to_owned()),
            window_resizable: true,
            window_fullscreen: None,
            window_maximized: false,
            window_visible: true,
            window_icon: None,
            renderer_debug: false,
        }
    }
}

pub struct Engine {
    window_builder: Option<WindowBuilder>,
    renderer: Option<Renderer>,
    renderer_debug: bool,
}

impl Engine {
    fn new(wb: WindowBuilder, renderer_debug: bool) -> Self {
        Engine {
            window_builder: Some(wb),
            renderer: None,
            renderer_debug,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        // window
        let (event_loop, window) = self.init_window()?;

        // renderer
        self.init_renderer(window.clone())?;
        let mut renderer = self
            .renderer
            .take()
            .ok_or("Couldnt take renderer. Did you forget to call self.init_renderer() ?")?;

        // gameloop state
        let tps = 20;
        let max_frameskip = 5;
        let game_loop = GameLoop::new(tps, max_frameskip)?;

        debug!("start event loop");
        // event_loop.run() hijacks the main thread and calls std::process::exit when
        // done anything that has not been moved in the closure will not be dropped
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
                renderer.window_resized();
            }
            Event::MainEventsCleared => {
            // NOTE: the MainEventsCleared event "will be emitted when all input events
            //       have been processed and redraw processing is about to begin".
                for action in game_loop.actions() {
                    match action {
                        gameloop::FrameAction::Tick => {
                            // // update state
                        }
                        gameloop::FrameAction::Render { interpolation: _ } => {
                            renderer.begin();
                            // gather components and submit work
                            renderer.end()
                        }
                    }
                }
            }
            _ => (),
        });
    }

    fn init_window(&mut self) -> Result<(EventLoop<()>, Arc<Window>)> {
        debug!("init_window");
        let event_loop = EventLoop::new();
        let window = self
            .window_builder
            .take()
            .ok_or("window_builder is None")?
            .build(&event_loop)?;

        Ok((event_loop, Arc::new(window)))
    }

    fn init_renderer(&mut self, window: Arc<Window>) -> Result<()> {
        debug!("init_renderer");

        let renderer = Renderer::new(window, self.renderer_debug)?;
        self.renderer = Some(renderer);

        Ok(())
    }
}
