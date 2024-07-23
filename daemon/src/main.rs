//! DBus daemon

use std::path::PathBuf;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use dbus::blocking::{stdintf::org_freedesktop_dbus::RequestNameReply, Connection};
use dbus_crossroads::{Context, Crossroads};

use anyhow::bail;
use log::{info, warn};
use yahallo::{camera::Cam, config::Config, img_to_dlib, process_image, FaceRecognizer};
use yahallo::{is_dark, to_rgb, DbusResult, Error, YahalloResult};

struct State {
    fr: FaceRecognizer,
    config: Config,
    cam_drop: Option<JoinHandle<()>>,
}

impl State {
    fn myconfig() -> anyhow::Result<Self> {
        let config = Config::new(
            PathBuf::from("/dev/video2"),
            PathBuf::from("data"),
            PathBuf::from("data/faces.json"),
            0.6,
            30,
        )?;
        let fr = FaceRecognizer::new(&config)?;
        Ok(Self {
            fr,
            config,
            cam_drop: None,
        })
    }
}

fn check_match(
    _ctx: &mut Context,
    State {
        fr,
        config,
        cam_drop,
    }: &mut State,
    (_username,): (String,),
) -> YahalloResult<()> {
    if !fr.has_faces() {
        // In the future, we should check for this particular user
        warn!("No faces in the database!");
        return Err(Error::NoData);
    }
    if let Some(cam_drop) = cam_drop.take() {
        let _ = cam_drop
            .join()
            .map_err(|_| warn!("Error joining camera drop thread"));
    }
    let mut cam = Cam::start(config.camera_path())?;
    let start = Instant::now();
    let timeout = Duration::from_secs(2);
    loop {
        if start.elapsed() >= timeout {
            warn!("Timeout trying to detect face!");
            *cam_drop = Some(std::thread::spawn(move || {
                let _ = cam.stop().map_err(|e| warn!("Error stopping camera: {e}"));
            }));
            return Err(Error::Timeout);
        }
        let frame = cam.capture()?;
        let img = process_image(frame)?;
        if is_dark(&img, config.dark_threshold()) {
            info!("frame too dark!");
            continue;
        } else {
            info!("looking for matches");
        }
        let img = to_rgb(&img);
        let matrix = img_to_dlib(&img)?;
        if let Some(model) = fr.check_match(&matrix, config)? {
            println!("{}", model.label());
            // TODO: Check username!!
            break;
        } else {
            println!("No match");
        }
    }
    *cam_drop = Some(std::thread::spawn(move || {
        let _ = cam.stop().map_err(|e| warn!("Error stopping camera: {e}"));
    }));
    Ok(())
}

fn main() -> anyhow::Result<()> {
    pretty_env_logger::formatted_timed_builder()
        .filter_level(log::LevelFilter::Trace)
        .init();

    let c = Connection::new_system()?;
    const NAME: &str = "com.iamkroot.yahallo";
    let reply = c.request_name(NAME, false, false, true)?;
    if reply != RequestNameReply::PrimaryOwner {
        bail!("Could not become owner of service - {reply:?}");
    };
    let mut cr = Crossroads::new();
    let iface_token = cr.register(NAME, |b| {
        b.method(
            "CheckMatch",
            ("username",),
            ("result",),
            |ctx, state, input| {
                let m = check_match(ctx, state, input);
                let res = match m {
                    Ok(_) => DbusResult::Success,
                    Err(e) => DbusResult::Error(e),
                };
                Ok((res,))
            },
        );
        // TODO: Add a reload faces method?
    });
    cr.insert("/", &[iface_token], State::myconfig()?);
    cr.serve(&c)?;
    Ok(())
}
