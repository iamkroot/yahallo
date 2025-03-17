use std::path::Path;

use anyhow::Result;
use config::FDetMode;
use data::FaceEnc;
use data::{Faces, ModelData};
use dlib_face_recognition::{
    FaceDetector, FaceDetectorTrait, FaceEncoderNetwork, FaceEncoderTrait, FaceEncoding,
    FaceEncodings, LandmarkPredictor, LandmarkPredictorTrait,
};
pub use dlib_face_recognition::{ImageMatrix, Rectangle};
use image::buffer::ConvertBuffer;
use image::math::Rect;
use image::DynamicImage;
use image::GenericImageView;
use image::Luma;
use log::warn;
use rscam::Frame;

pub mod camera;
pub mod config;
pub mod data;
mod error;
mod utils;

use crate::config::Config;
pub use crate::error::{DbusResult, Error, YahalloResult};
pub use crate::utils::Stopwatch;

struct FaceDet(Box<dyn MyFaceDet>);

trait MyFaceDet {
    fn face_locations(&self, img: &image::DynamicImage) -> Result<Vec<Rect>>;
}

impl MyFaceDet for FaceDet {
    fn face_locations(&self, img: &image::DynamicImage) -> Result<Vec<Rect>> {
        self.0.face_locations(img)
    }
}

impl MyFaceDet for dlib_face_recognition::FaceDetector {
    fn face_locations(&self, img: &image::DynamicImage) -> Result<Vec<Rect>> {
        let mat = img_to_dlib(img)?;
        let locs = FaceDetectorTrait::face_locations(self, &mat);
        Ok(locs
            .iter()
            .map(|loc| Rect {
                x: loc.left as _,
                y: loc.top as _,
                width: (loc.right - loc.left) as _,
                height: (loc.bottom - loc.top) as _,
            })
            .collect())
        // todo!()
    }
}

// TODO: Custom image struct that basically caches the different variants (dynamic image, dlib image, resized, etc.)

unsafe impl Send for FaceDet {}

use rusty_yunet::detect_faces;

struct YuNetFaceDet {}

mod image_utils;

impl MyFaceDet for YuNetFaceDet {
    fn face_locations(&self, img: &image::DynamicImage) -> Result<Vec<Rect>> {
        let ib: image_utils::BgrImage = image_utils::dyn_to_bgr(img);
        Ok(
            detect_faces(ib.as_raw(), ib.width().try_into()?, ib.height().try_into()?)?
                .into_iter()
                .map(|f| f.rectangle())
                .map(|r| Rect {
                    x: r.x as u32,
                    y: r.y as u32,
                    width: r.w as u32,
                    height: r.h as u32,
                })
                .collect(),
        )
    }
}

pub struct FaceRecognizer {
    fdet: FaceDet,
    lm_pred: LandmarkPredictor,
    encoder: FaceEncoderNetwork,
    known_faces: Faces,
}

impl FaceRecognizer {
    pub fn new(config: &Config) -> Result<Self> {
        let fdt = if config.fdet_mode == FDetMode::Dlib {
            Some(std::thread::spawn(FaceDetector::new))
        } else {
            None
        };
        let lm_path = config.dlib_model_dat("shape_predictor_5_face_landmarks.dat")?;
        let lmt = std::thread::spawn(move || LandmarkPredictor::open(lm_path));
        let enc_path = config.dlib_model_dat("dlib_face_recognition_resnet_model_v1.dat")?;
        let ent = std::thread::spawn(move || FaceEncoderNetwork::open(enc_path));

        let fdet = if config.fdet_mode == FDetMode::Dlib {
            FaceDet(Box::new(
                fdt.unwrap()
                    .join()
                    // TODO: Print the panics properly instead of ignoring them
                    .map_err(|_| anyhow::format_err!("Dlib FDet init failed!"))?,
            ))
        } else {
            FaceDet(Box::new(YuNetFaceDet {}))
        };
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
            fdet,
            lm_pred,
            encoder,
            known_faces: encs,
        })
    }

    /// Returns largest face rect on image, if it is available
    pub fn get_face_rect(&self, matrix: &image::DynamicImage) -> YahalloResult<Option<Rect>> {
        // TODO: Actually return the largest :P
        let locs = self.fdet.face_locations(matrix)?;
        if locs.len() > 1 {
            warn!("Expected just one face, found {}", locs.len());
            return Err(Error::MultipleFaces);
        }
        Ok(locs.first().cloned())
    }

    pub fn gen_encodings(&self, matrix: &image::DynamicImage) -> YahalloResult<FaceEncodings> {
        let rect = self.get_face_rect(matrix)?.ok_or(Error::NoFace)?;
        Ok(self.gen_encodings_with_rect_dlib(matrix, rect))
    }

    pub fn gen_encodings_with_rect_dlib(
        &self,
        matrix: &image::DynamicImage,
        rect: Rect,
    ) -> FaceEncodings {
        let dlib_image = img_to_dlib(matrix).unwrap();
        let dlib_rect = dlib_face_recognition::Rectangle {
            left: rect.x as _,
            top: rect.y as _,
            right: (rect.x + rect.width) as _,
            bottom: (rect.y + rect.height) as _,
        };
        let landmarks = self.lm_pred.face_landmarks(&dlib_image, &dlib_rect);
        self.encoder
            .get_face_encodings(&dlib_image, &[landmarks], 0)
    }

    /// Given an encoding, try to find the closest match
    pub fn get_enc_info(&self, encoding: &FaceEnc, config: &Config) -> Option<&ModelData> {
        // TODO: For now, we only find the first match below threshold
        self.known_faces
            .check_match(encoding, config.match_threshold)
    }

    pub fn check_match(
        &self,
        matrix: &image::DynamicImage,
        config: &Config,
    ) -> YahalloResult<Option<&ModelData>> {
        // TODO: Check staleness of self.known_faces
        let Some(rect) = self.get_face_rect(matrix)? else {
            return Ok(None);
        };
        // TODO: Switch to sface
        let encodings = self.gen_encodings_with_rect_dlib(matrix, rect);
        let encoding = encodings.first().unwrap();
        // let enc = FaceEnc::from(encoding);
        // TODO: Return more info about the match
        Ok(self.get_enc_info(&encoding.into(), config))
    }

    pub fn add_face(&mut self, enc: FaceEnc, label: Option<String>) -> Result<()> {
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

pub fn center_crop(
    img: &impl GenericImageView<Pixel = Luma<u8>>,
) -> image::SubImage<&impl GenericImageView<Pixel = Luma<u8>>> {
    let (w, h) = img.dimensions();
    assert!(w >= h, "potrait image not supported");
    let x = (w - h) / 2;
    let cropped = image::imageops::crop_imm(img, x, 0, h, h);
    cropped
}

/// Convert the frame into an image buffer
pub fn process_image(frame: Frame) -> Result<GrayFrameImage> {
    image::ImageBuffer::<image::Luma<u8>, _>::from_raw(
        frame.resolution.0,
        frame.resolution.1,
        frame,
    )
    .ok_or(anyhow::anyhow!("no img from cam frame"))
}

pub fn to_rgb(img: &GrayFrameImage) -> image::DynamicImage {
    image::DynamicImage::ImageRgb8(img.convert())
}

pub fn is_dark(img: &impl GenericImageView<Pixel = Luma<u8>>, threshold_percent: u32) -> bool {
    let cropped = center_crop(img);
    let hist = gen_hist::<12>(cropped.inner());
    let total: u32 = hist.iter().sum();
    let dark_percent = (hist[0] * 100) / total;
    dark_percent >= threshold_percent
}

/// Resize to target width preserving the aspect ratio
pub fn resize_to_width(img: &DynamicImage, target_width: u32) -> DynamicImage {
    let w = img.width();
    let aspect_ratio = w as f64 / img.height() as f64;
    let target_height = (target_width as f64 / aspect_ratio).round() as u32;
    // TODO: Need to make sure height is divisible by x??
    img.resize(
        target_width,
        target_height,
        image::imageops::FilterType::Nearest,
    )
    // todo!()
    // (image::imageops::resize(
    //     img,
    //     target_width,
    //     target_height,
    //     image::imageops::FilterType::Nearest,
    // ))
}

const fn int_ceil(a: usize, b: usize) -> usize {
    (a - 1) / b + 1
}

const fn bin<const BINS: usize>(val: u8) -> usize {
    let per_bin: u8 = int_ceil(u8::MAX as usize, BINS) as u8;
    (val / per_bin) as usize
}

/// Get the histogram from a grayscale image
fn gen_hist<const BINS: usize>(img: &impl GenericImageView<Pixel = Luma<u8>>) -> [u32; BINS] {
    let mut hist = [0; BINS];

    for (_, _, p) in img.pixels() {
        let val = p.0[0];
        hist[bin::<BINS>(val)] += 1;
    }
    hist
}

pub fn img_to_dlib(img: &DynamicImage) -> Result<ImageMatrix> {
    let img = resize_to_width(img, 320);
    Ok(ImageMatrix::from_image(&img.to_rgb8()))
}
