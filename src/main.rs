use clap::Parser;
use std::time::Instant;

mod args;
mod converter;

use args::PatchArgs;
use converter::uop_to_mul;

fn main() -> std::io::Result<()> {
    let args = PatchArgs::parse();

    println!("start patching...");
    let start = Instant::now();

    uop_to_mul(&args)?;

    let duration = start.elapsed();
    println!("Time elapsed is: {:?}", duration);

    Ok(())
}
