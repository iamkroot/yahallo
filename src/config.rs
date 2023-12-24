use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

#[derive(Debug)]
pub struct Config {
    camera_path: PathBuf,
    // Could use the "embed-nn" feature of dlib to avoid this.
    // To get a completely independent binary, we would also have to enable the "build-native" flag of dlib
    dlib_model_dir: PathBuf,
    data_dir: PathBuf,
    /// Euclidean distance
    pub(crate) match_threshold: f64,
}

impl Config {
    pub fn new(
        camera_path: PathBuf,
        dlib_model_dir: PathBuf,
        data_dir: PathBuf,
        match_threshold: f64,
    ) -> Self {
        Self {
            camera_path,
            dlib_model_dir,
            data_dir,
            match_threshold,
        }
    }

    pub(crate) fn dlib_model_dat(&self, filename: &str) -> Result<PathBuf> {
        let file = self.dlib_model_dir.join(filename);
        if !file.exists() {
            bail!("Dlib file not found {}", file.display())
        } else {
            Ok(file)
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn camera_path(&self) -> &Path {
        &self.camera_path
    }

    pub(crate) fn faces_file(&self) -> PathBuf {
        // TODO: Can allow user to specify this directly instead of assuming it is inside data dir
        self.data_dir().join("faces.json")
    }
}
