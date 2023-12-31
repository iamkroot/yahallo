use std::time::Duration;
use std::path::Path;
use std::num::NonZeroU32;

use anyhow::{anyhow, bail, Context, Result};

pub struct Cam {
    cam: rscam::Camera,
    config: rscam::Config<'static>,
}

impl Cam {
    pub fn start(camera_path: impl AsRef<Path>) -> Result<Self> {
        let device = camera_path
            .as_ref()
            .to_str()
            .ok_or_else(|| anyhow!("Invalid camera path {}", camera_path.as_ref().display()))?;
        let mut cam = rscam::Camera::new(device).context("cam open err")?;
        let rscam_config = Self::configure(&cam)?;
        cam.start(&rscam_config)?;
        Ok(Self {
            cam,
            config: rscam_config,
        })
    }

    pub fn interval(&self) -> Duration {
        Duration::from_secs_f64(self.config.interval.0 as f64 / self.config.interval.1 as f64)
    }

    pub fn resolution(&self) -> Result<(NonZeroU32, NonZeroU32)> {
        let (w, h) = self.config.resolution;
        Ok((w.try_into()?, h.try_into()?))
    }

    #[allow(dead_code)]
    fn dump_resolutions(cam: &rscam::Camera) {
        for fmt in cam.formats() {
            let Ok(fmti) = fmt else {
                continue;
            };
            dbg!(&fmti);
            let _ = dbg!(cam.resolutions(&fmti.format));
        }
    }

    fn configure(cam: &rscam::Camera) -> Result<rscam::Config<'static>> {
        let format = b"GREY";
        let res = {
            let mut res = None;
            for fmt in cam.formats() {
                let Ok(fmti) = fmt else {
                    continue;
                };
                if &fmti.format == format {
                    res =
                        Some(cam.resolutions(format).with_context(|| {
                            format!("format {}", String::from_utf8_lossy(format))
                        })?);
                }
            }
            res.ok_or_else(|| anyhow!("No suitable resolution found"))?
        };
        let resolution = match res {
            rscam::ResolutionInfo::Discretes(v) => v
                .first()
                .ok_or_else(|| anyhow::anyhow!("No resolutions! {v:#?}"))
                .cloned()?,
            _ => anyhow::bail!("Only support discrete resolutions"),
        };
        let interval = match cam
            .intervals(format, resolution)
            .context("camera interval")?
        {
            rscam::IntervalInfo::Discretes(v) => v
                .first()
                .ok_or_else(|| anyhow!("no intervals! {v:?}"))
                .cloned()?,
            _ => bail!("Only support discrete inervals"),
        };
        Ok(rscam::Config {
            interval,
            resolution,
            format,
            ..Default::default()
        })
    }

    pub fn capture(&mut self) -> Result<rscam::Frame> {
        Ok(self.cam.capture()?)
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::Stopwatch;

    use super::*;

    #[test]
    fn stream() {
        let _sw = Stopwatch::new("capture");

        let mut cam = Cam::start("/dev/video2").expect("failed to start cam");

        for i in 0..10 {
            println!("iter {i}");
            let _img = Stopwatch::time("img", || cam.capture().expect("frame err"));
            std::thread::sleep_ms(100);
        }
    }
}
