use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Ok;
use clap::Parser;
use dlib_face_recognition::{ImageMatrix, Rectangle};
use image::buffer::ConvertBuffer;
use log::{debug, info, warn};
use winit::event::{Event, KeyEvent, StartCause, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::WindowBuilder;
use yahallo::camera::Cam;
use yahallo::config::Config;
use yahallo::data::ModelData;
use yahallo::{process_image, FaceRecognizer};

#[derive(Debug, Parser, Clone)]
#[command(name = "yahallo")]
#[command(about = "Facial Recognition CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug, Clone)]
enum Commands {
    // #[command(arg_required_else_help = true)]
    Add {
        #[arg(long)]
        label: Option<String>,
        /// How long to wait for face
        #[arg(long, default_value = "30s")]
        timeout: humantime::Duration,
    },
    // #[command(arg_required_else_help = true)]
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
        Commands::Add { label, timeout } => handle_add(config, timeout.into(), fr, label)?,
        Commands::Test {
            exit_on_match,
            duration,
        } => todo!(),
    }
    Ok(())
}
const RED: u32 = u32::from_be_bytes([0, 255, 0, 0]);

fn redraw(buffer: &mut [u32], fr: &FaceRecognizer, cam: &mut Cam) -> anyhow::Result<Instant> {
    let frame = cam.capture()?;
    let start = Instant::now();
    let next_frame_at = start + cam.interval();
    let aspect_ratio = frame.resolution.0 as f64 / frame.resolution.1 as f64;

    const WIDTH: f64 = 320.0;
    let scale = frame.resolution.0 as f64 / WIDTH;
    let img = process_image(frame)?;
    // let matrix = img_to_dlib(&img)?;
    let resized = image::imageops::resize(
        &img,
        WIDTH as u32,
        (WIDTH / aspect_ratio) as u32,
        image::imageops::FilterType::Nearest,
    );
    let matrix = ImageMatrix::from_image(&resized);
    let Some(rect) = fr.get_face_rect(&matrix)? else {
        info!("No face in frame");
        // TODO: Do we want to skip this?
        for (i, p) in img.pixels().enumerate() {
            buffer[i] = u32::from_be_bytes([0, p.0[0], p.0[1], p.0[2]]);
        }
        return Ok(next_frame_at);
    };
    // upscale the rect to orig image size
    let rect = Rectangle {
        left: (rect.left as f64 * scale) as i64,
        top: (rect.top as f64 * scale) as i64,
        right: (rect.right as f64 * scale) as i64,
        bottom: (rect.bottom as f64 * scale) as i64,
    };
    debug!("writing pixels!");
    for (i, (c, r, p)) in img.enumerate_pixels().enumerate() {
        let r = r as i64;
        let c = c as i64;
        let v = if ((r == rect.top || r == rect.bottom) && (c >= rect.left && c <= rect.right))
            || ((c == rect.left || c == rect.right) && (r >= rect.top && r <= rect.bottom))
        {
            // Draw rect boundary in red
            RED
        } else {
            u32::from_be_bytes([0, p.0[0], p.0[1], p.0[2]])
        };
        buffer[i] = v;
        // let v: u32 = p.into();
    }
    Ok(next_frame_at)
}

fn handle_add(
    config: Config,
    timeout: Duration,
    fr: FaceRecognizer,
    label: Option<String>,
) -> anyhow::Result<()> {
    let mut cam = Cam::start(config.camera_path())?;
    let (width, height) = cam.resolution()?;
    let start = Instant::now();
    let event_loop = EventLoop::new().unwrap();
    let window = Rc::new(
        WindowBuilder::new()
            .with_inner_size(winit::dpi::PhysicalSize::new(
                u32::from(width),
                u32::from(height),
            ))
            .with_resizable(false)
            .with_title("yahallo")
            .build(&event_loop)
            .unwrap(),
    );
    let context = softbuffer::Context::new(window.clone()).unwrap();
    let mut surface = softbuffer::Surface::new(&context, window.clone()).unwrap();
    surface.resize(width, height).unwrap();
    event_loop.listen_device_events(winit::event_loop::DeviceEvents::Never);
    event_loop.run(move |evt, elwt| {
        if start.elapsed() >= timeout {
            warn!("Timeout trying to detect face!");
            elwt.exit();
            return;
        }
        match evt {
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                // | Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                let mut buffer = surface.buffer_mut().unwrap();
                // the redraw call is blocking- will be limited by the cam fps
                let next_frame_at = redraw(&mut buffer, &fr, &mut cam).expect("failed to draw");
                buffer.present().unwrap();
                window.request_redraw();
                elwt.set_control_flow(ControlFlow::wait_duration(next_frame_at - Instant::now()));
            }
            Event::WindowEvent {
                event:
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                logical_key:
                                    Key::Named(NamedKey::Escape) | Key::Named(NamedKey::Exit),
                                ..
                            },
                        ..
                    },
                window_id,
            } if window_id == window.id() => {
                elwt.exit();
            }
            _ => {
                debug!("other event {evt:?}")
            }
        }
    })?;
    // loop {
    // if let Some(dur) = duration {
    //     if start.elapsed() >= dur {
    //         // timed out
    //         warn!("Timeout trying to detect face!");
    //         break;
    //     }
    // }
    // let frame = cam.capture()?;
    // let matrix: dlib_face_recognition::ImageMatrix = convert_image(frame)?;
    // let Some(rect) = fr.get_face_rect(&matrix)? else {
    //     info!("No face in frame");
    //     continue;
    // };
    // would sure be nice to get some generators here
    // let encodings = fr.gen_encodings_with_rect(&matrix, &rect);
    // if let Some(encoding) = encodings.first() {
    //     let model = ModelData::new(
    //         0,
    //         label.unwrap_or_else(|| String::from("model")),
    //         0,
    //         encoding.clone(),
    //     );
    //     break;
    // }
    // }
    Ok(())
}
