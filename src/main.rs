use std::error::Error;
use std::path::PathBuf;

use clap::Parser;
use extract_model_info_json::{extract_model_info, FsPorts, IndicatifProgressReporter};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[arg(value_name = "ROOT_DIR")]
    root_dir: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    if !cli.root_dir.exists() {
        return Err(format!("root not found: {}", cli.root_dir.display()).into());
    }

    if !cli.root_dir.is_dir() {
        return Err(format!("not a directory: {}", cli.root_dir.display()).into());
    }

    let ports = FsPorts::new();
    let progress = IndicatifProgressReporter::new();
    let stats = extract_model_info(&ports, &progress, &cli.root_dir)?;

    println!(
        "directories: {} safetensors_dirs: {} zip_checked: {} extracted: {}",
        stats.directories_scanned,
        stats.safetensors_directories,
        stats.zip_files_checked,
        stats.extracted
    );

    Ok(())
}
