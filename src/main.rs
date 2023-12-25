use std::path::PathBuf;
use std::time::Instant;

use anyhow::Ok;
use clap::Parser;
use log::{info, warn};
use yahallo::camera::Cam;
use yahallo::config::Config;
use yahallo::data::ModelData;
use yahallo::{convert_image, FaceRecognizer};

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
        #[arg(long)]
        /// When to exit. Runs indefinitely unless specified.
        duration: Option<humantime::Duration>,
    },
    #[command(arg_required_else_help = true)]
    Test {
        /// Whether to exit after first match
        #[arg(long)]
        exit_on_match: bool,
        /// When to exit. Runs indefinitely unless specified.
        #[arg(long)]
        duration: Option<humantime::Duration>,
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
        Commands::Add { label, duration } => {
            let mut cam = Cam::start(config.camera_path())?;
            let start = Instant::now();
            let duration = duration.map(|d| d.into());
            loop {
                if let Some(dur) = duration {
                    if start.elapsed() >= dur {
                        // timed out
                        warn!("Timeout trying to detect face!");
                        break;
                    }
                }
                let frame = cam.capture()?;
                let matrix = convert_image(frame)?;
                let Some(rect) = fr.get_face_rect(&matrix)? else {
                    info!("No face in frame");
                    continue;
                };
                let encodings = fr.gen_encodings_with_rect(&matrix, &rect);
                if let Some(encoding) = encodings.first() {
                    let model = ModelData::new(
                        0,
                        label.unwrap_or_else(|| String::from("model")),
                        0,
                        encoding.clone(),
                    );
                    break;
                }
            }
        }
        Commands::Test {
            exit_on_match,
            duration,
        } => todo!(),
    }
    Ok(())
}
