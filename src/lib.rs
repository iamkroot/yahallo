use anyhow::{Ok, Result};
use dlib_face_recognition::{
    FaceDetectorTrait, FaceEncoderNetwork, FaceEncoderTrait, FaceEncodings, FaceLocations,
    ImageMatrix, LandmarkPredictor, LandmarkPredictorTrait,
};
mod utils;
pub mod pam_handler;
mod utils;

use crate::config::Config;

struct FaceDetector(Box<dyn FaceDetectorTrait>);

impl FaceDetectorTrait for FaceDetector {
    fn face_locations(&self, image: &dlib_face_recognition::ImageMatrix) -> FaceLocations {
        self.0.face_locations(image)
    }
}

unsafe impl Send for FaceDetector {}

struct FaceRecognizer {
    fdet: FaceDetector,
    lm_pred: LandmarkPredictor,
    encoder: FaceEncoderNetwork,
    // TODO: Also store metadata about the known faces
    known_faces: FaceEncodings,
}

impl FaceRecognizer {
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
            .iter()
            .any(|known| known.distance(encoding) >= config.match_threshold))
    }
}
