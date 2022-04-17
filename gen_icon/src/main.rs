use std::{error::Error, fs::File};

use ico::{IconDir, IconDirEntry, IconImage, ResourceType};

// ref: https://docs.rs/ico/latest/ico/
fn main() {
    std::process::exit(match run() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {:?}", e);
            1
        }
    })
}

fn run() -> Result<(), Box<dyn Error>> {
    if std::env::args().len() < 3 {
        eprintln!("USAGE: gen_icon IMAGE_PATH ICON_PATH");
    }

    let input_filepath = std::env::args().nth(1).ok_or("no icon filepath given")?;
    let output_filepath = std::env::args().nth(2).ok_or("no output filepath given")?;

    if !input_filepath.ends_with(".png") {
        return Err("only supports png files".into());
    }

    // Create a new, empty icon collection:
    let mut icon_dir = IconDir::new(ResourceType::Icon);

    // Read a PNG file from disk and add it to the collection:
    let file = File::open(input_filepath)?;
    let image = IconImage::read_png(file)?;
    icon_dir.add_entry(IconDirEntry::encode(&image)?);

    // Finally, write the ICO file to disk:
    let file = File::create(output_filepath)?;
    icon_dir.write(file)?;

    Ok(())
}
