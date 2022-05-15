use core::{Application, EngineBuilder};
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

    let app = Sandbox {};
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

struct Sandbox {}

impl Application for Sandbox {
    fn on_init(&mut self, mut ctx: core::Context) {
        ctx.set_background_color(&[0.0, 0.4, 1.0, 1.0]);
    }

    fn on_update(&mut self, _: core::Context) {}

    fn on_render(&mut self, mut ctx: core::Context) {
        let position = Vector2::new(0.0, 0.0);
        let size = Vector2::new(0.1, 0.1);

        for x in (-50..50).step_by(1) {
            for y in (-50..50).step_by(1) {
                let x = x as f32 * 0.1;
                let y = y as f32 * 0.1;
                let pos = Vector2::new(x, y).add(position);
                let color = Vector4::new((x + 5.0) / 10.0, 0.4, (y + 5.0) / 10.0, 0.7);
                ctx.draw_quad(pos, size, color);
            }
        }
    }
}
