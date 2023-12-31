use std::path::Path;

use anyhow::Result;
use data::Faces;
use dlib_face_recognition::{
    FaceDetector, FaceDetectorTrait, FaceEncoderNetwork, FaceEncoderTrait, FaceEncoding,
    FaceEncodings, FaceLocations, ImageMatrix, LandmarkPredictor, LandmarkPredictorTrait,
    Rectangle,
};
use image::buffer::ConvertBuffer;
use image::RgbImage;
use rscam::Frame;

pub mod camera;
pub mod config;
pub mod data;
pub mod pam_handler;
mod utils;

use crate::config::Config;
pub use crate::utils::Stopwatch;

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
    pub fn get_face_rect(&self, matrix: &ImageMatrix) -> Result<Option<Rectangle>> {
        // TODO: Actually return the largest :P
        let locs = self.fdet.face_locations(matrix);
        if locs.len() > 1 {
            anyhow::bail!("Expected just one face, found {}", locs.len());
        }
        Ok(locs.first().cloned())
    }

    pub fn gen_encodings(&self, matrix: &ImageMatrix) -> Result<FaceEncodings> {
        let rect = &self
            .get_face_rect(matrix)?
            .ok_or_else(|| anyhow::anyhow!("No faces detected!"))?;
        let landmarks = self.lm_pred.face_landmarks(matrix, rect);
        let encodings = self.encoder.get_face_encodings(matrix, &[landmarks], 0);
        Ok(encodings)
    }

    pub fn gen_encodings_with_rect(&self, matrix: &ImageMatrix, rect: &Rectangle) -> FaceEncodings {
        let landmarks = self.lm_pred.face_landmarks(matrix, rect);
        self.encoder.get_face_encodings(matrix, &[landmarks], 0)
    }

    pub fn check_match(&self, matrix: &ImageMatrix, config: &Config) -> Result<bool> {
        // TODO: Check staleness of self.known_faces
        let Some(rect) = self.get_face_rect(matrix)? else {
            return Ok(false);
        };
        let encodings = self.gen_encodings_with_rect(matrix, &rect);
        let Some(encoding) = encodings.first() else {
            anyhow::bail!("Encoder failed to process landmarks");
        };
        // TODO: Return more info about the match
        Ok(self
            .known_faces
            .check_match(encoding, config.match_threshold)
            .is_some())
    }

    pub fn add_face(&mut self, enc: FaceEncoding, label: Option<String>) -> Result<()> {
        self.known_faces.add_face(enc, label)
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

/// Convert the frame into an rgb image
pub fn process_image(frame: Frame) -> Result<RgbImage> {
    let img = image::ImageBuffer::<image::Luma<u8>, _>::from_raw(
        frame.resolution.0,
        frame.resolution.1,
        frame,
    )
    .ok_or(anyhow::anyhow!("no img from cam frame"))?;
    Ok(img.convert())
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

pub fn img_to_dlib(img: &RgbImage) -> Result<ImageMatrix> {
    let img = resize_to_width(img, 320);
    Ok(ImageMatrix::from_image(&img))
}
