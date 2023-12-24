use anyhow::Result;
use data::Faces;
use dlib_face_recognition::{
    FaceDetector, FaceDetectorTrait, FaceEncoderNetwork, FaceEncoderTrait, FaceEncodings,
    FaceLocations, ImageMatrix, LandmarkPredictor, LandmarkPredictorTrait,
};

pub mod camera;
mod config;
mod data;
pub mod pam_handler;
mod utils;

use crate::config::Config;

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
        let encs = Faces::from_file(&faces_file)?;
        Ok(Self {
            fdet: FaceDet(Box::new(fdet)),
            lm_pred,
            encoder,
            known_faces: encs,
        })
    }

    fn gen_encodings(&self, matrix: &ImageMatrix) -> Result<FaceEncodings> {
        let locs = self.fdet.face_locations(matrix);
        if locs.len() > 1 {
            anyhow::bail!("Expected just one face, found {}", locs.len());
        }
        let Some(rect) = locs.first() else {
            anyhow::bail!("No faces detected!");
        };

        let landmarks = self.lm_pred.face_landmarks(matrix, rect);
        let encodings = self.encoder.get_face_encodings(matrix, &[landmarks], 0);
        Ok(encodings)
    }

    pub fn check_match(&self, matrix: &ImageMatrix, config: &Config) -> Result<bool> {
        // TODO: Check staleness of self.known_faces
        let encodings = self.gen_encodings(matrix)?;
        let Some(encoding) = encodings.first() else {
            anyhow::bail!("Encoder failed to process landmarks");
        };
        // TODO: Return more info about the match
        Ok(self
            .known_faces
            .check_match(encoding, config.match_threshold)
            .is_some())
    }
}
