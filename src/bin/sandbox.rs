use core::EngineBuilder;

use log::LevelFilter;
use winit::dpi::{LogicalSize, Size};

fn main() {
    // initialize logger
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    // create and run engine
    // engine takes ownership of thread and will call std::process::exit for us
    EngineBuilder::new()
        .with_window_size(Size::Logical(LogicalSize::new(1024.0, 768.0)))
        .build()
        .run()
        .expect("failed to run engine");
}
