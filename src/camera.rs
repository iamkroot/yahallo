struct Cam {
    cam: rscam::Camera,
}

#[cfg(test)]
mod tests {
    use crate::utils::Stopwatch;

    use super::*;

    #[test]
    fn stream() {
        let _sw = Stopwatch::new("capture");

        let mut cam = rscam::Camera::new("/dev/video2").expect("cam open err");
        let (format, resolution, interval) = {
            let mut res = None;
            let format = "GREY".as_bytes();
            for fmt in cam.formats() {
                let fmti = fmt.expect("fmt");
                if &fmti.format == b"GREY" {
                    res = Some(cam.resolutions(&fmti.format).expect("res"));
                    break;
                }
            }
            let resolution = match res.expect("res") {
                rscam::ResolutionInfo::Discretes(v) => v[0],
                _ => panic!("res"),
            };
            let interval = match cam.intervals(format, resolution).expect("intv") {
                rscam::IntervalInfo::Discretes(v) => v[1],
                _ => panic!("intv"),
            };
            (format, resolution, interval)
        };

        Stopwatch::time("camstart", || {
            cam.start(&rscam::Config {
                interval,
                resolution,
                format,
                ..Default::default()
            })
            .expect("cam start")
        });

        for i in 0..10 {
            println!("iter {i}");
            let img = Stopwatch::time("img",||cam.capture().expect("frame err"));
            std::thread::sleep_ms(100);
        }
    }
}
