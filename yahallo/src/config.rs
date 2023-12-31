use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

#[derive(Debug)]
pub struct Config {
    camera_path: PathBuf,
    // Could use the "embed-nn" feature of dlib to avoid this.
    // To get a completely independent binary, we would also have to enable the "build-native" flag of dlib
    dlib_model_dir: PathBuf,
    faces_file: PathBuf,
    /// Euclidean distance
    pub(crate) match_threshold: f64,
    /// maximum percent of dark pixels in frame to allow face recog
    dark_threshold: u32,
}

impl Config {
    pub fn new(
        camera_path: PathBuf,
        dlib_model_dir: PathBuf,
        faces_file: PathBuf,
        match_threshold: f64,
        dark_threshold: u32,
    ) -> anyhow::Result<Self> {
        if faces_file.is_dir() {
            bail!("Faces file should not be a dir!");
        }
        if dark_threshold > 100 {
            bail!("Dark threshold percent should be 0..=100");
        }
        Ok(Self {
            camera_path,
            dlib_model_dir,
            faces_file,
            match_threshold,
            dark_threshold,
        })
    }

    pub(crate) fn dlib_model_dat(&self, filename: &str) -> Result<PathBuf> {
        let file = self.dlib_model_dir.join(filename);
        if !file.exists() {
            bail!("Dlib file not found {}", file.display())
        } else {
            Ok(file)
        }
    }

    pub fn camera_path(&self) -> &Path {
        &self.camera_path
    }

    pub fn faces_file(&self) -> &Path {
        &self.faces_file
    }

    pub fn dark_threshold(&self) -> u32 {
        self.dark_threshold
    }
}
