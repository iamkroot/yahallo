use std::path::PathBuf;

use anyhow::{Result, anyhow};

#[derive(Debug)]
pub(crate) struct Config {
    camera_path: PathBuf,
    // Could use the "embed-nn" feature of dlib to avoid this.
    // To get a completely independent binary, we would also have to enable the "build-native" flag of dlib
    dlib_model_dir: PathBuf,
    data_dir: PathBuf,
    /// Euclidean distance
    pub(crate) match_threshold: f64,
}

impl Config {
    pub(crate) fn dlib_model_dat(&self, filename: &str) -> Result<PathBuf> {
        let file = self.dlib_model_dir.join(filename);
        if !file.exists() {
            Err(anyhow!("Dlib file not found {}", file.display()))
        } else {
            Ok(file)
        }
    }
}
