//! DBus daemon

use std::path::PathBuf;
use std::time::{Duration, Instant};

use dbus::blocking::{stdintf::org_freedesktop_dbus::RequestNameReply, Connection};
use dbus_crossroads::{Context, Crossroads};

use anyhow::bail;
use log::{info, warn};
use yahallo::{camera::Cam, config::Config, img_to_dlib, process_image, FaceRecognizer};
use yahallo::{is_dark, to_rgb, YahalloResult, Error};

struct State {
    fr: FaceRecognizer,
    config: Config,
}

impl State {
    fn myconfig() -> anyhow::Result<Self> {
        let config = Config::new(
            PathBuf::from("/dev/video2"),
            PathBuf::from("data"),
            PathBuf::from("data/faces.json"),
            0.8,
            80,
        )?;
        let fr = FaceRecognizer::new(&config)?;
        Ok(Self { fr, config })
    }
}

fn check_match(
    _ctx: &mut Context,
    State { fr, config }: &mut State,
    (_username,): (String,),
) -> YahalloResult<()> {
    let mut cam = Cam::start(config.camera_path())?;
    let start = Instant::now();
    let timeout = Duration::from_secs(10);
    loop {
        if start.elapsed() >= timeout {
            warn!("Timeout trying to detect face!");
            return Err(Error::Timeout);
        }
        let frame = cam.capture()?;
        let img = process_image(frame)?;
        if is_dark(&img, config.dark_threshold()) {
            info!("frame too dark!");
            continue;
        }
        let img = to_rgb(&img);
        let matrix = img_to_dlib(&img)?;
        if fr.check_match(&matrix, config)? {
            // TODO: Check username!!
            break;
        }
    }
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
            ("success", "error"),
            |ctx, state, input| {
                let m = check_match(ctx, state, input);
                let res = match m {
                    Ok(_) => None,
                    Err(e) => Some(e)
                };
                let final_res = (res.is_none(), res.unwrap_or_else(|| unsafe { std::mem::uninitialized() }));
                Ok(final_res)
            },
        );
        // TODO: Add a reload faces method?
    });
    cr.insert("/", &[iface_token], State::myconfig()?);
    cr.serve(&c)?;
    Ok(())
}
