use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{bail, Ok};
use clap::Parser;
use image::math::Rect;
use image::{DynamicImage, GenericImageView};
use log::{debug, info, warn};
use text_on_image::FontBundle;
use winit::event::{Event, KeyEvent, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::WindowBuilder;
use yahallo::camera::Cam;
use yahallo::config::{Config, FDetMode, FRcgMode};
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
        FDetMode::YuNet,
        FRcgMode::Dlib,
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
const RED: image::Rgba<u8> = image::Rgba::<u8>([255, 0, 0, 0]);
const FONT_DATA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../res/CutiveMono-Regular.ttf"
));

fn name_for_face<'a>(
    fr: &'a FaceRecognizer,
    config: &Config,
    img: &DynamicImage,
    rect: Rect,
) -> &'a str {
    // return "Unknown";
    let encodings = fr.gen_encodings_with_rect_dlib(img, rect);
    if encodings.len() > 1 {
        "Too many!"
    } else if encodings.len() == 0 {
        "Unknown"
    } else {
        let enc = &encodings[0];
        if let Some(info) = fr.get_enc_info(&enc.into(), config) {
            info.label()
        } else {
            "Not found"
        }
    }
}

fn redraw(
    buffer: &mut [u32],
    fr: &FaceRecognizer,
    cam: &mut Cam,
    config: &Config,
    font_bundle: &FontBundle,
) -> anyhow::Result<Instant> {
    let frame = cam.capture()?;
    let start = Instant::now();
    let next_frame_at = start + cam.interval();
    info!("New frame");
    const WIDTH: f64 = 320.0;
    let scale = frame.resolution.0 as f64 / WIDTH;
    let img = process_image(frame)?;
    // first, write the original image to output buffer
    debug_assert_eq!(
        img.width() as usize * img.height() as usize,
        buffer.len(),
        "Why was it resized?"
    );
    for (i, p) in img.pixels().enumerate() {
        buffer[i] = u32::from_be_bytes([0, p.0[0], p.0[0], p.0[0]]);
    }
    if is_dark(&img, config.dark_threshold()) {
        info!("frame too dark!");
        return Ok(next_frame_at);
    }
    let img = to_rgb(&img);
    let resized = resize_to_width(&img, WIDTH as _);
    // let matrix = ImageMatrix::from_image(&resized.to_rgb8());
    let Some(rect) = fr.get_face_rect(&resized)? else {
        info!("No face in frame");
        return Ok(next_frame_at);
    };
    let name = name_for_face(fr, config, &resized, rect);
    // upscale the rect to orig image size
    let rect = Rectangle {
        left: (rect.x as f64 * scale) as i64,
        top: (rect.y as f64 * scale) as i64,
        right: ((rect.x + rect.width) as f64 * scale) as i64,
        bottom: ((rect.y + rect.height) as f64 * scale) as i64,
    };
    debug!("writing pixels!");
    // draw_rect(buffer, img.width() as _, rect, RED);

    let mut dyn_img = img;
    text_on_image::text_on_image_draw_debug(
        &mut dyn_img,
        name,
        font_bundle,
        rect.left.try_into()?,
        rect.top.try_into()?,
        text_on_image::TextJustify::Left,
        text_on_image::VerticalAnchor::Bottom,
        text_on_image::WrapBehavior::NoWrap,
    );
    // draw image and rect on buffer
    // TODO: Should instead have a method to draw directly on buffer
    for (i, (c, r, p)) in dyn_img.pixels().enumerate() {
        let r = r as i64;
        let c = c as i64;
        let v = if ((r == rect.top || r == rect.bottom) && (c >= rect.left && c <= rect.right))
            || ((c == rect.left || c == rect.right) && (r >= rect.top && r <= rect.bottom))
        {
            // Draw rect boundary in red
            RED.0
        } else {
            p.0
        };
        buffer[i] = u32::from_be_bytes([v[3], v[0], v[1], v[2]]);
    }
    Ok(next_frame_at)
}

/// Helper to draw some lines
#[allow(dead_code)]
fn draw_rect(buffer: &mut [u32], buffer_w: usize, rect: Rectangle, color: image::Rgba<u8>) {
    let buffer_h = buffer.len() / buffer_w;
    if rect.left >= buffer_w as _ || rect.top >= buffer_h as _ {
        warn!("Rectangle outside of frame!");
        return;
    }
    let clamped = Rectangle {
        left: rect.left.max(0),
        top: rect.top.max(0),
        right: rect.right.min(buffer_w as _),
        bottom: rect.bottom.min(buffer_h as _),
    };
    let color = u32::from_be_bytes(color.0);
    // top horizontal
    if rect.top >= 0 {
        for x in clamped.left..clamped.right {
            buffer[buffer_w * rect.top as usize + x as usize] = color;
        }
    }
    // bottom horizontal
    if rect.bottom < buffer_h as _ {
        for x in clamped.left..clamped.right {
            buffer[buffer_w * rect.bottom as usize + x as usize] = color;
        }
    }
    // left vertical
    if rect.left >= 0 {
        for y in clamped.top..clamped.bottom {
            buffer[buffer_w * y as usize + rect.left as usize] = color;
        }
    }
    // right vertical
    if rect.right < buffer_w as _ {
        for y in clamped.top..clamped.bottom {
            buffer[buffer_w * y as usize + rect.right as usize] = color;
        }
    }
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
        // let matrix = img_to_dlib(&img)?;
        let Some(rect) = fr.get_face_rect(&img)? else {
            info!("No face in frame");
            continue;
        };
        let encodings = fr.gen_encodings_with_rect_dlib(&img, rect);
        let encoding = encodings.first().unwrap();
        fr.add_face(encoding.into(), label)?;
        fr.dump_faces_file(config.faces_file())?;
        break;
    }
    Ok(())
}

fn handle_test(config: Config, timeout: Option<Duration>) -> anyhow::Result<()> {
    let font: rusttype::Font<'static> = rusttype::Font::try_from_bytes(FONT_DATA).unwrap();
    let font_bundle = text_on_image::FontBundle::new(&font, rusttype::Scale::uniform(30.0), RED);

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
                let next_frame_at = match redraw(&mut buffer, &fr, &mut cam, &config, &font_bundle)
                {
                    Result::Ok(next_frame_at) => next_frame_at,
                    Err(err) => {
                        warn!("Failed to draw: {err}");
                        Instant::now()
                    }
                };
                buffer.present().unwrap();
                window.request_redraw();
                elwt.set_control_flow(ControlFlow::wait_duration(
                    (next_frame_at - Instant::now()).max(Duration::ZERO),
                ));
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
