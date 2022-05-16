use core::{Application, EngineBuilder, TIME};
use std::{env, ops::Add};

use cgmath::{Vector2, Vector4};
use log::LevelFilter;
use winit::{
    dpi::{LogicalSize, Size},
    window::Icon,
};

const ICON_BYTES: &[u8] = include_bytes!("../../assets/engine-icon.png");

fn main() {
    // initialize logger
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    // load window icon
    let icon_image =
        image::load_from_memory(ICON_BYTES).expect("failed to load image from embeded");
    let icon = match Icon::from_rgba(
        icon_image.clone().into_bytes(),
        icon_image.width(),
        icon_image.height(),
    ) {
        Ok(icon) => icon,
        Err(e) => panic!("failed to load icon from image: {:?}", e),
    };

    let enable_renderer_debug = env::var("RENDERER_DEBUG").is_ok();

    let app = Sandbox::new();
    // create and run engine
    // engine takes ownership of thread and will call std::process::exit for us
    EngineBuilder::new(Box::new(app))
        .with_window_size(Size::Logical(LogicalSize::new(1024.0, 768.0)))
        .with_window_title("Sandbox (Vulkan Engine)".to_owned())
        .with_window_icon(icon)
        .with_renderer_debug(enable_renderer_debug)
        .build()
        .run()
        .expect("failed to run engine");
}

struct Sandbox {
    position: Vector2<f32>,
    quads: Vec<(Vector2<f32>, Vector2<f32>, Vector4<f32>)>,
}

impl Sandbox {
    fn new() -> Self {
        Self {
            position: Vector2::new(0.0, 0.0),
            quads: Vec::new(),
        }
    }
}

impl Application for Sandbox {
    fn on_init(&mut self, mut ctx: core::Context) {
        TIME!("app.on_init");
        ctx.set_background_color(&[0.0, 0.4, 1.0, 1.0]);

        // compute quads
        let size = Vector2::new(0.075, 0.075);
        let x_count = 100;
        let y_count = 100;
        for x in (-x_count / 2..x_count / 2).step_by(1) {
            for y in (-y_count / 2..y_count / 2).step_by(1) {
                // pos
                let x = x as f32 * 0.1;
                let y = y as f32 * 0.1;
                let pos = Vector2::new(x, y).add(self.position);
                // color
                let x_multiplier = (x_count / 2) as f32 * 0.1;
                let y_multiplier = (y_count / 2) as f32 * 0.1;
                let color = Vector4::new(
                    (x + x_multiplier) / (x_multiplier * 2.0),
                    0.4,
                    (y + y_multiplier) / (y_multiplier * 2.0),
                    1.0,
                );
                // push quad
                self.quads.push((pos, size, color))
            }
        }
    }

    fn on_update(&mut self, _: core::Context) {
        TIME!("app.on_update");
    }

    fn on_render(&mut self, mut ctx: core::Context) {
        TIME!("app.on_render");
        for (pos, size, color) in &self.quads {
            ctx.draw_quad(*pos, *size, *color);
        }
    }
}
