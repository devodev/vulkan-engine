use log::info;
use std::error::Error;

use core::engine::Engine;

fn main() {
    // initialize logger
    env_logger::init();

    // run app
    std::process::exit(match run_app() {
        Ok(_) => 0,
        Err(err) => {
            eprintln!("error: {:?}", err);
            1
        }
    });
}

fn run_app() -> Result<(), Box<dyn Error>> {
    info!("start");

    let mut engine = Engine::new();
    engine.run()?;

    info!("end");

    Ok(())
}
