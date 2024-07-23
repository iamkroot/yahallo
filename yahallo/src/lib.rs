use std::path::Path;

use anyhow::Result;
use data::{Faces, ModelData};
use dlib_face_recognition::{
    FaceDetector, FaceDetectorTrait, FaceEncoderNetwork, FaceEncoderTrait, FaceEncoding,
    FaceEncodings, FaceLocations, LandmarkPredictor, LandmarkPredictorTrait,
};
pub use dlib_face_recognition::{ImageMatrix, Rectangle};
use image::buffer::ConvertBuffer;
use image::RgbImage;
use log::warn;
use rscam::Frame;

pub mod camera;
pub mod config;
pub mod data;
mod error;
mod utils;

use crate::config::Config;
pub use crate::utils::Stopwatch;
pub use crate::error::{DbusResult, Error, YahalloResult};

struct FaceDet(Box<dyn FaceDetectorTrait>);

impl FaceDetectorTrait for FaceDet {
    fn face_locations(&self, image: &dlib_face_recognition::ImageMatrix) -> FaceLocations {
        self.0.face_locations(image)
    }
}

unsafe impl Send for FaceDet {}

pub struct FaceRecognizer {
    fdet: FaceDet,
    lm_pred: LandmarkPredictor,
    encoder: FaceEncoderNetwork,
    known_faces: Faces,
}

impl FaceRecognizer {
    pub fn new(config: &Config) -> Result<Self> {
        let fdt = std::thread::spawn(FaceDetector::new);
        let lm_path = config.dlib_model_dat("shape_predictor_5_face_landmarks.dat")?;
        let lmt = std::thread::spawn(move || LandmarkPredictor::open(lm_path));
        let enc_path = config.dlib_model_dat("dlib_face_recognition_resnet_model_v1.dat")?;
        let ent = std::thread::spawn(move || FaceEncoderNetwork::open(enc_path));
        let fdet = fdt
            .join()
            // TODO: Print the panics properly instead of ignoring them
            .map_err(|_| anyhow::format_err!("FDet init failed!"))?;
        let lm_pred = lmt
            .join()
            .map_err(|_| anyhow::format_err!("LMPred init failed!"))?
            .map_err(|e| anyhow::anyhow!(e))?;
        let encoder = ent
            .join()
            .map_err(|_| anyhow::format_err!("Enc init failed!"))?
            .map_err(|e| anyhow::anyhow!(e))?;

        let faces_file = config.faces_file();
        let encs = Faces::from_file(faces_file)?;
        Ok(Self {
            fdet: FaceDet(Box::new(fdet)),
            lm_pred,
            encoder,
            known_faces: encs,
        })
    }

    /// Returns largest face rect on image, if it is available
    pub fn get_face_rect(&self, matrix: &ImageMatrix) -> YahalloResult<Option<Rectangle>> {
        // TODO: Actually return the largest :P
        let locs = self.fdet.face_locations(matrix);
        if locs.len() > 1 {
            warn!("Expected just one face, found {}", locs.len());
            return Err(Error::MultipleFaces);
        }
        Ok(locs.first().cloned())
    }

    pub fn gen_encodings(&self, matrix: &ImageMatrix) -> YahalloResult<FaceEncodings> {
        let rect = &self.get_face_rect(matrix)?.ok_or(Error::NoFace)?;
        let landmarks = self.lm_pred.face_landmarks(matrix, rect);
        let encodings = self.encoder.get_face_encodings(matrix, &[landmarks], 0);
        Ok(encodings)
    }

    pub fn gen_encodings_with_rect(&self, matrix: &ImageMatrix, rect: &Rectangle) -> FaceEncodings {
        let landmarks = self.lm_pred.face_landmarks(matrix, rect);
        self.encoder.get_face_encodings(matrix, &[landmarks], 0)
    }

    pub fn check_match(&self, matrix: &ImageMatrix, config: &Config) -> YahalloResult<Option<&ModelData>> {
        // TODO: Check staleness of self.known_faces
        let Some(rect) = self.get_face_rect(matrix)? else {
            return Ok(None);
        };
        let encodings = self.gen_encodings_with_rect(matrix, &rect);
        let encoding = encodings.first().unwrap();
        // TODO: Return more info about the match
        Ok(self
            .known_faces
            .check_match(encoding, config.match_threshold))
    }

    pub fn add_face(&mut self, enc: FaceEncoding, label: Option<String>) -> Result<()> {
        self.known_faces.add_face(enc, label)
    }

    pub fn has_faces(&self) -> bool {
        !self.known_faces.is_empty()
    }

    pub fn dump_faces_file(&self, path: &Path) -> Result<()> {
        self.known_faces.to_file(path)
    }
}

// pub fn convert_image(frame: Frame) -> Result<ImageMatrix> {
//     let img = image::ImageBuffer::<image::Luma<u8>, _>::from_raw(
//         frame.resolution.0,
//         frame.resolution.1,
//         frame,
//     )
//     .ok_or(anyhow::anyhow!("no img from cam frame"))?;
//     let img = image::imageops::resize(&img, 320, 180, image::imageops::FilterType::Nearest);
//     let img = img.convert();
//     Ok(ImageMatrix::from_image(&img))
// }

type GrayFrameImage = image::ImageBuffer<image::Luma<u8>, Frame>;

/// Convert the frame into an rgb image
pub fn process_image(frame: Frame) -> Result<GrayFrameImage> {
    image::ImageBuffer::<image::Luma<u8>, _>::from_raw(
        frame.resolution.0,
        frame.resolution.1,
        frame,
    )
    .ok_or(anyhow::anyhow!("no img from cam frame"))
}

pub fn to_rgb(img: &GrayFrameImage) -> RgbImage {
    img.convert()
}

pub fn is_dark(img: &GrayFrameImage, threshold_percent: u32) -> bool {
    let hist = gen_hist::<8>(img);
    let total: u32 = hist.iter().sum();
    let dark_percent = (hist[0] * 100) / total;
    dark_percent >= threshold_percent
}

/// Resize to target width preserving the aspect ratio
pub fn resize_to_width(img: &RgbImage, target_width: u32) -> RgbImage {
    let w = img.width();
    let aspect_ratio = w as f64 / img.height() as f64;
    let target_height = (target_width as f64 / aspect_ratio).round() as u32;
    // TODO: Need to make sure height is divisible by x??
    image::imageops::resize(
        img,
        target_width,
        target_height,
        image::imageops::FilterType::Nearest,
    )
}

const fn bin<const BINS: usize>(val: u8) -> usize {
    (val / (((u8::MAX as usize + 1) / BINS) as u8)) as usize
}

/// Get the histogram from a grayscale image
fn gen_hist<const BINS: usize>(img: &GrayFrameImage) -> [u32; BINS] {
    let mut hist = [0; BINS];

    for p in img.pixels() {
        let val = p.0[0];
        hist[bin::<BINS>(val)] += 1;
    }
    hist
}

pub fn img_to_dlib(img: &RgbImage) -> Result<ImageMatrix> {
    let img = resize_to_width(img, 320);
    Ok(ImageMatrix::from_image(&img))
}
