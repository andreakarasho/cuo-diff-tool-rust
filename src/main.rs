use clap::Parser;
use std::{path::Path, time::Instant};

mod args;
mod converter;

use args::PatchArgs;
use converter::uop_to_mul;

fn main() {
    let args = PatchArgs::parse();

    println!("start patching...");
    let start = Instant::now();

    patch(&args).unwrap();

    // let files = [
    //     "artLegacyMUL.uop",
    //     "gumpartLegacyMUL.uop",
    //     "MultiCollection.uop",
    //     "soundLegacyMUL.uop",
    //     "map0LegacyMUL.uop",
    //     "map1LegacyMUL.uop",
    //     "map2LegacyMUL.uop",
    //     "map3LegacyMUL.uop",
    //     "map4LegacyMUL.uop",
    //     "map5LegacyMUL.uop",
    // ];

    // for f in files.iter() {
    //     println!("running {}", &f);

    //     patch(PatchArgs {
    //         source_dir: String::from("D:\\Giochi\\Ultima Online Classic"),
    //         output_dir: String::from("./output"),
    //         file_to_process: String::from(*f),
    //     })
    //     .unwrap();
    // }

    let duration = start.elapsed();
    println!("Time elapsed is: {:?}", duration);
}

fn patch(args: &PatchArgs) -> std::io::Result<()> {
    let output_path = Path::new(&args.output_dir);

    if !output_path.exists() {
        std::fs::create_dir_all(&output_path)?;
    }

    uop_to_mul(&args)?;

    Ok(())
}
