use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use image::{buffer::ConvertBuffer, Luma};

use dlib_face_recognition::{
    FaceDetector, FaceDetectorCnn, FaceDetectorTrait, FaceEncoderNetwork, FaceEncoderTrait,
    FaceEncoding, ImageMatrix, LandmarkPredictor, LandmarkPredictorTrait,
};


#[derive(Debug)]
struct ModelData {
    #[allow(dead_code)]
    time: u64,
    label: String,
    id: u64,
    // data: Array1<f64>,
    data: Vec<f64>,
}

struct FDet(Box<dyn FaceDetectorTrait>);

impl std::ops::Deref for FDet {
    type Target = Box<dyn FaceDetectorTrait>;

    fn deref(&self) -> &Self::Target {
        // todo!()
        &self.0
    }
}
unsafe impl Send for FDet {}

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
    #[clap(long)]
    use_cnn: bool,
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
fn write_enc(enc: &FaceEncoding, label: &str, path: impl AsRef<Path>) {
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

fn capture_img() -> image::ImageBuffer<image::Rgb<u8>, Vec<u8>> {
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
            rscam::IntervalInfo::Discretes(v) => v[0],
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
    let img = {
        let _sw = Stopwatch::new("img");
        // let img = image::open(&args.).expect("Unable to open");
        let img = cam.capture().expect("frame err");
        dbg!(&img.resolution);
        dbg!(&img.len());
        // img.save_with_format("/home/kroot/Pictures/f1.png", image::ImageFormat::Png).expect("unable to save");
        let img =
            image::ImageBuffer::<Luma<u8>, _>::from_raw(img.resolution.0, img.resolution.1, img)
                .expect("img");
        let img = image::imageops::resize(&img, 320, 180, image::imageops::FilterType::Nearest);
        img.convert()
        // img
    };
    std::thread::spawn(move || cam.stop().expect("cam stop"));
    img
}

fn make_fdet(args: &Args) -> FDet {
    let _sw = Stopwatch::new("initfd");
    if args.use_cnn {
        FDet(Box::new(
            FaceDetectorCnn::open(args.dlib_model_dat("mmod_human_face_detector.dat"))
                .expect("cnn open failure"),
        ))
    } else {
        FDet(Box::new(FaceDetector::new()))
    }
}

fn main() -> ExitCode {
    let args = Arc::new(Args::parse());
    let _sw = Stopwatch::new("full");

    const MAX_DIST: f64 = 0.6;
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

    let args0 = args.clone();
    let fdet = std::thread::spawn(move || make_fdet(&args0));

    let img = capture_img();
    let matrix = Stopwatch::time("mat", || ImageMatrix::from_image(&img));

    let args2 = args.clone();
    let args3 = args.clone();
    let pred = move || {
        let _sw = Stopwatch::new("initpred");
        let file = args2.dlib_model_dat("shape_predictor_5_face_landmarks.dat");
        LandmarkPredictor::open(file).expect("landmark init")
    };
    let enc = move || {
        let _sw = Stopwatch::new("initenc");
        let file = args3.dlib_model_dat("dlib_face_recognition_resnet_model_v1.dat");
        FaceEncoderNetwork::open(file).expect("initenc")
    };
    let pred_t = std::thread::spawn(pred);
    let enc_t = std::thread::spawn(enc);

    let fdet = fdet.join().expect("fdet join");
    let locs = {
        let _sw = Stopwatch::new("FDet");
        fdet.face_locations(&matrix)
    };

    let Some(l) = locs.first() else {
        return ExitCode::FAILURE;
    };
    let pred = pred_t.join().expect("pred join");

    let lm = {
        let _sw = Stopwatch::new("lm");
        pred.face_landmarks(&matrix, l)
    };
    let enc = enc_t.join().expect("enc join");

    let encs = {
        let _sw = Stopwatch::new("enc");
        enc.get_face_encodings(&matrix, &[lm], 0)
    };

    dbg!(encs.len());
    let idx = {
        let _sw = Stopwatch::new("find");
        let e = encs.first().expect("no face found");
        // write_enc(e, "kroot1", "enc2.data");
        known_encs
            .iter()
            .position(|enc| enc.distance(e) <= MAX_DIST)
    };
    if let Some(idx) = idx {
        println!("Match found: {}!", data[idx].0);
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}