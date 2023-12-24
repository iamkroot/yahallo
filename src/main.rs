use std::path::PathBuf;

use anyhow::Ok;
use clap::Parser;
use yahallo::{convert_image, FaceRecognizer};
use yahallo::data::ModelData;
use yahallo::config::Config;
use yahallo::camera::Cam;

#[derive(Debug, Parser, Clone)]
#[command(name = "yahallo")]
#[command(about = "Facial Recognition CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug, Clone)]
enum Commands {
    #[command(arg_required_else_help = true)]
    Add {
        #[arg(long)]
        label: Option<String>,
    },
    #[command(arg_required_else_help = true)]
    Test {
        /// Whether to exit after first match
        #[arg(long)]
        exit_on_match: bool,
        /// When to exit. Runs indefinitely unless specified.
        #[arg(long)]
        duration: Option<u32>,
    },
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    let config = Config::new(
        PathBuf::from("/dev/video2"),
        PathBuf::from("data"),
        PathBuf::from("data"),
        0.8,
    );
    let fr = FaceRecognizer::new(&config)?;
    match args.command {
        Commands::Add { label } => {
            let mut cam = Cam::start(config.camera_path())?;
            let frame = cam.capture()?;
            let matrix = convert_image(frame)?;
            let encodings = fr.gen_encodings(&matrix)?;
            if let Some(encoding) = encodings.first() {
                let model = ModelData::new(
                    0,
                    label.unwrap_or_else(|| String::from("model")),
                    0,
                    encoding.clone(),
                );
            }
        }
        Commands::Test {
            exit_on_match,
            duration,
        } => todo!(),
    }
    Ok(())
}
