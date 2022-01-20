use std::{fs::File, io::BufReader, path::Path};

use dlib_face_recognition::{
    FaceDetector, FaceDetectorTrait, FaceEncoderNetwork, FaceEncoderTrait, ImageMatrix,
    LandmarkPredictor, LandmarkPredictorTrait,
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

fn euc_dist(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .fold(0.0, |acc, (x, y)| acc + (*x - *y).powi(2))
        .sqrt()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let data_dir = std::path::PathBuf::from(
        args.next()
            .expect("First arg should be data dir containing dlib models"),
    );
    let models_path = std::path::PathBuf::from(
        args.next()
            .expect("Second arg should be path to data file containing encodings"),
    );
    let img_path =
        std::path::PathBuf::from(args.next().expect("Third arg should be path to image file"));
    let _sw = Stopwatch::new("full");

    const MAX_DIST: f64 = 0.4;
    let (data, known_encs) = {
        let _sw = Stopwatch::new("read");

        let models = read_encodings(&models_path);
        let (data, known_encs): (Vec<_>, Vec<_>) = models
            .into_iter()
            .map(|m| ((m.label, m.id), m.data))
            .unzip();
        (data, known_encs)
    };
    // let det = {
    //     let _sw = Stopwatch::new("initcnn");
    //     let mut file = data_dir.clone();
    //     file.push("mmod_human_face_detector.dat");
    //     FaceDetectorCnn::new(file).expect("cnn erro")
    // };
    let fdet = {
        let _sw = Stopwatch::new("initfd");
        FaceDetector::new()
    };
    let img = {
        let _sw = Stopwatch::new("img");
        let img = image::open(img_path).expect("Unable to open");

        let img = img.resize(1000, 320, image::imageops::FilterType::Nearest);

        img.to_rgb8()
    };

    let matrix = ImageMatrix::from_image(&img);
    let locs = {
        let _sw = Stopwatch::new("FDet");
        fdet.face_locations(&matrix)
    };

    let pred = {
        let _sw = Stopwatch::new("initpred");
        let mut file = data_dir.clone();
        file.push("shape_predictor_5_face_landmarks.dat");
        LandmarkPredictor::new(file).expect("landmark init")
    };
    let enc = {
        let _sw = Stopwatch::new("initenc");
        let mut file = data_dir.clone();
        file.push("dlib_face_recognition_resnet_model_v1.dat");
        FaceEncoderNetwork::new(file).expect("initenc")
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

        let idx = {
            let _sw = Stopwatch::new("find");
            let e = encs.first().expect("no face found");
            known_encs
                .iter()
                .position(|enc| euc_dist(enc, e) <= MAX_DIST)
        };
        if let Some(idx) = idx {
            println!("Match found: {}!", data[idx].0);
        }
    }
}
