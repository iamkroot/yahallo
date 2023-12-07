use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use clap::Parser;
use image::{buffer::ConvertBuffer, Luma};

use dlib_face_recognition::{
    FaceDetector, FaceDetectorTrait, FaceEncoderNetwork, FaceEncoderTrait, FaceEncoding,
    ImageMatrix, LandmarkPredictor, LandmarkPredictorTrait,
};

struct Stopwatch {
    start: std::time::Instant,
    name: &'static str,
}

impl Stopwatch {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            start: std::time::Instant::now(),
        }
    }
}

impl Drop for Stopwatch {
    fn drop(&mut self) {
        println!(
            "[{}] elapsed {}ms",
            self.name,
            self.start.elapsed().as_millis()
        );
    }
}

#[derive(Debug)]
struct ModelData {
    #[allow(dead_code)]
    time: u64,
    label: String,
    id: u64,
    // data: Array1<f64>,
    data: Vec<f64>,
}

fn read_encodings(path: &Path) -> Vec<ModelData> {
    let f = File::open(path).expect("read enc");
    let rdr = BufReader::new(f);
    let encs: serde_json::Value = serde_json::from_reader(rdr).expect("read json");
    encs.as_array()
        .unwrap()
        .iter()
        .map(|v| ModelData {
            time: v["time"].as_u64().unwrap(),
            label: v["label"].as_str().unwrap().to_string(),
            id: v["id"].as_u64().unwrap(),
            data: {
                // only get first array
                let o = v["data"].as_array().unwrap().first().unwrap();
                let v = o.as_array().unwrap().iter().map(|f| f.as_f64().unwrap());
                // Array1::from_iter(v)
                v.collect()
            },
        })
        .collect()
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    dlib_data_dir: PathBuf,
    #[clap(help = "path to data file containing encodings")]
    encodings_path: PathBuf,
    camera_num: usize,
}

impl Args {
    pub(crate) fn dlib_model_dat(&self, filename: &str) -> PathBuf {
        let mut file = self.dlib_data_dir.clone();
        file.push(filename);
        if !file.exists() {
            panic!("File not found {}", file.display());
        }
        file
    }
}


#[allow(dead_code)]
fn write_enc(enc: &FaceEncoding, label: &str, path: &Path) {
    let v: Vec<f64> = enc.as_ref().into();
    let out = serde_json::json!([{
        "time": 0,
        "label": label,
        "id": 0,
        "data": [v],
    }]);
    let out = serde_json::to_string(&out).expect("couldn't write json");
    std::fs::write(path, out).expect("failed to write");
}

fn main() {
    let args = Args::parse();
    let _sw = Stopwatch::new("full");

    const MAX_DIST: f64 = 0.4;
    let (data, known_encs) = {
        let _sw = Stopwatch::new("read");

        let models = read_encodings(&args.encodings_path);
        let (data, known_encs): (Vec<_>, Vec<_>) = models
            .into_iter()
            .map(|m| {
                (
                    (m.label, m.id),
                    FaceEncoding::from_vec(&m.data).expect("Invalid face encoding"),
                )
            })
            .unzip();
        (data, known_encs)
    };
    let mut cam = rscam::Camera::new("/dev/video2").expect("cam open err");
    let (format, resolution, interval) = {
        let mut res = None;
        let format = "GREY".as_bytes();
        for fmt in cam.formats() {
            let fmti = fmt.expect("fmt");
            let d = String::from_utf8(fmti.format.to_vec()).expect("asdf");
            if d != "GREY" {
                continue;
            }
            res = Some(cam.resolutions(&fmti.format).expect("res"));
        }
        let resolution = match res.expect("res") {
            rscam::ResolutionInfo::Discretes(v) => v[0],
            _ => panic!("res"),
        };
        let interval = match cam.intervals(format, resolution).expect("intv") {
            rscam::IntervalInfo::Discretes(v) => v[0],
            _ => panic!("intv"),
        };
        (format, resolution, interval)
    };
    // return;
    // dbg!();
    // cam.start(rscam::Config::default())

    // let det = {
    //     let _sw = Stopwatch::new("initcnn");
    //     let file = args.dlib_model_dat("mmod_human_face_detector.dat");
    //     FaceDetectorCnn::new(file).expect("cnn erro")
    // };

    let fdet = {
        let _sw = Stopwatch::new("initfd");
        FaceDetector::new()
    };
    let img = {
        let _sw = Stopwatch::new("img");
        cam.start(&rscam::Config {
            interval,
            resolution,
            format,
            ..Default::default()
        })
        .expect("cam start");
        // let img = image::open(&args.).expect("Unable to open");
        let img = cam.capture().expect("frame err");
        dbg!(&img.resolution);
        dbg!(&img.len());
        // let img = image::imageops::resize(&img, 1000, 320, image::imageops::FilterType::Nearest)
        // img.save_with_format("/home/kroot/Pictures/f1.png", image::ImageFormat::Png).expect("unable to save");
        let img =
            image::ImageBuffer::<Luma<u8>, _>::from_raw(img.resolution.0, img.resolution.1, img)
                .expect("img");
        img.convert()
        // img
    };

    let matrix = ImageMatrix::from_image(&img);
    let locs = {
        let _sw = Stopwatch::new("FDet");
        fdet.face_locations(&matrix)
    };

    let pred = {
        let _sw = Stopwatch::new("initpred");
        let file = args.dlib_model_dat("shape_predictor_5_face_landmarks.dat");
        LandmarkPredictor::open(file).expect("landmark init")
    };
    let enc = {
        let _sw = Stopwatch::new("initenc");
        let file = args.dlib_model_dat("dlib_face_recognition_resnet_model_v1.dat");
        FaceEncoderNetwork::open(file).expect("initenc")
    };

    for l in locs.iter() {
        let lm = {
            let _sw = Stopwatch::new("lm");
            pred.face_landmarks(&matrix, l)
        };

        let encs = {
            let _sw = Stopwatch::new("enc");
            enc.get_face_encodings(&matrix, &[lm], 0)
        };

        dbg!(encs.len());
        let idx = {
            let _sw = Stopwatch::new("find");
            let e = encs.first().expect("no face found");
            // write_enc(e, "enc.data");
            known_encs
                .iter()
                .position(|enc| enc.distance(e) <= MAX_DIST)
        };
        if let Some(idx) = idx {
            println!("Match found: {}!", data[idx].0);
        }
    }
}
