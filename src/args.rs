use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct PatchArgs {
    #[arg(short, long)]
    pub source_dir: String,

    #[arg(short, long)]
    pub output_dir: String,

    #[arg(short, long)]
    pub file_to_process: String,
}
