use log::LevelFilter;

use core::Engine;

fn main() {
    // initialize logger
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    // create and run engine
    // engine takes ownership of thread and will call std::process::exit for us
    Engine::new().run().unwrap();
}
