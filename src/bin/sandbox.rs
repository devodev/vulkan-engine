use core::Engine;

fn main() {
    // initialize logger
    env_logger::init();

    // create and run engine
    // engine takes ownership of thread and will call std::process::exit for us
    Engine::new().run().unwrap();
}
