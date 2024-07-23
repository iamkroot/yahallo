use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{bail, Ok};
use clap::Parser;
use log::{debug, info, warn};
use winit::event::{Event, KeyEvent, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::WindowBuilder;
use yahallo::camera::Cam;
use yahallo::config::Config;
use yahallo::{
    img_to_dlib, is_dark, process_image, resize_to_width, to_rgb, FaceRecognizer, ImageMatrix,
    Rectangle,
};

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
        timeout: Option<humantime::Duration>,
    },
}

fn main() -> anyhow::Result<()> {
    pretty_env_logger::formatted_timed_builder()
        .filter_level(log::LevelFilter::Debug)
        .init();
    let args = Cli::parse();
    let config = Config::new(
        PathBuf::from("/dev/video2"),
        PathBuf::from("data"),
        PathBuf::from("data/faces.json"),
        0.6,
        30,
    )?;
    match args.command {
        Commands::Add { label, timeout } => handle_add(config, timeout.into(), label)?,
        Commands::Test {
            exit_on_match: _,
            timeout,
        } => handle_test(config, timeout.map(|t| t.into()))?,
    }
    Ok(())
}
const RED: u32 = u32::from_be_bytes([0, 255, 0, 0]);

fn redraw(
    buffer: &mut [u32],
    fr: &FaceRecognizer,
    cam: &mut Cam,
    config: &Config,
) -> anyhow::Result<Instant> {
    let frame = cam.capture()?;
    let start = Instant::now();
    let next_frame_at = start + cam.interval();
    info!("New frame");
    const WIDTH: f64 = 320.0;
    let scale = frame.resolution.0 as f64 / WIDTH;
    let img = process_image(frame)?;
    if is_dark(&img, config.dark_threshold()) {
        info!("frame too dark!");
        // TODO: Do we want to skip this?
        for (i, p) in img.pixels().enumerate() {
            buffer[i] = u32::from_be_bytes([0, p.0[0], p.0[0], p.0[0]]);
        }
        return Ok(next_frame_at);
    }
    let img = to_rgb(&img);
    let resized = resize_to_width(&img, 320);
    let matrix = ImageMatrix::from_image(&resized);
    let Some(rect) = fr.get_face_rect(&matrix)? else {
        println!("No face in frame");
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

fn handle_add(config: Config, timeout: Duration, label: Option<String>) -> anyhow::Result<()> {
    let mut fr = FaceRecognizer::new(&config)?;
    let mut cam = Cam::start(config.camera_path())?;
    let start = Instant::now();
    loop {
        if start.elapsed() >= timeout {
            warn!("Timeout trying to detect face!");
            bail!("No face detected!");
        }
        let frame = cam.capture()?;
        let img = process_image(frame)?;
        if is_dark(&img, config.dark_threshold()) {
            info!("frame too dark!");
            continue;
        }
        let img = to_rgb(&img);
        let matrix = img_to_dlib(&img)?;
        let Some(rect) = fr.get_face_rect(&matrix)? else {
            info!("No face in frame");
            continue;
        };
        let encodings = fr.gen_encodings_with_rect(&matrix, &rect);
        let encoding = encodings.first().unwrap();
        fr.add_face(encoding.clone(), label)?;
        fr.dump_faces_file(config.faces_file())?;
        break;
    }
    Ok(())
}

fn handle_test(config: Config, timeout: Option<Duration>) -> anyhow::Result<()> {
    let fr = FaceRecognizer::new(&config)?;
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
        if let Some(timeout) = timeout {
            if start.elapsed() >= timeout {
                warn!("Timeout trying to detect face!");
                elwt.exit();
                return;
            }
        }
        match evt {
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                // | Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                let mut buffer = surface.buffer_mut().unwrap();
                // the redraw call is blocking- will be limited by the cam fps
                let next_frame_at =
                    redraw(&mut buffer, &fr, &mut cam, &config).expect("failed to draw");
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
    Ok(())
}
