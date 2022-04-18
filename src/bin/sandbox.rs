use core::EngineBuilder;
use std::env;

use log::LevelFilter;
use winit::{
    dpi::{LogicalSize, Size},
    window::Icon,
};

fn main() {
    // initialize logger
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    // load window icon
    let icon_bytes = include_bytes!("../../assets/engine-icon.png");
    let icon_image = image::load_from_memory(icon_bytes).expect("failed to load image from bytes");
    let icon = match Icon::from_rgba(
        icon_image.clone().into_bytes(),
        icon_image.width(),
        icon_image.height(),
    ) {
        Ok(icon) => icon,
        Err(e) => panic!("failed to load icon from image: {:?}", e),
    };

    let enable_renderer_debug = env::var("RENDERER_DEBUG").is_ok();

    // create and run engine
    // engine takes ownership of thread and will call std::process::exit for us
    EngineBuilder::new()
        .with_window_size(Size::Logical(LogicalSize::new(1024.0, 768.0)))
        .with_window_title("Sandbox (Vulkan Engine)".to_owned())
        .with_window_icon(icon)
        .with_renderer_debug(enable_renderer_debug)
        .build()
        .run()
        .expect("failed to run engine");
}
