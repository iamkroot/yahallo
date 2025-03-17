use std::fs::File;
use std::io::{BufReader, BufWriter, ErrorKind};
use std::path::Path;
use std::time::SystemTime;

use crate::config::FRcgMode;
use anyhow::{anyhow, Context, Result};
use serde_json::json;

type FaceId = u64;

#[derive(Debug, Clone)]
pub struct FaceEnc(FRcgMode, Vec<f64>);

// TODO: This should be a trait rather than a struct
//  even an enum would be fine.
pub struct FaceEncs(Vec<FaceEnc>);

impl std::ops::Deref for FaceEncs {
    type Target = Vec<FaceEnc>;
    fn deref(&self) -> &Vec<FaceEnc> {
        &self.0
    }
}

impl From<FaceEnc> for FaceEncs {
    fn from(f: FaceEnc) -> Self {
        Self(vec![f])
    }
}

impl From<dlib_face_recognition::FaceEncodings> for FaceEncs {
    fn from(f: dlib_face_recognition::FaceEncodings) -> Self {
        Self(f.as_ref().iter().map(|e| e.into()).collect())
    }
}

impl From<dlib_face_recognition::FaceEncoding> for FaceEnc {
    fn from(e: dlib_face_recognition::FaceEncoding) -> Self {
        Self(FRcgMode::Dlib, e.as_ref().into())
    }
}

impl From<&dlib_face_recognition::FaceEncoding> for FaceEnc {
    fn from(e: &dlib_face_recognition::FaceEncoding) -> Self {
        Self(FRcgMode::Dlib, e.as_ref().into())
    }
}

impl FaceEnc {
    pub(crate) fn from_sface(v: &ort::value::Tensor<f32>) -> Result<Self> {
        assert_eq!(v.shape()?, &[1, 128]);
        Ok(Self(
            FRcgMode::SFace,
            (v.extract_raw_tensor().1)
                .iter()
                .map(|&v| f64::from(v))
                .collect(),
        ))
    }

    pub(crate) fn distance(&self, other: &Self) -> f64 {
        // cosine distance
        // TODO: check that they are the same model!
        const METRIC: &str = "euclidean";
        if METRIC == "cosine" {
            let n: f64 = self
                .1
                .iter()
                .zip(other.1.iter())
                .fold(0.0, |p, (x, y)| x * y + p);
            let a: f64 = self.1.iter().fold(0.0, |p, x| x * x + p);
            let b: f64 = other.1.iter().fold(0.0, |p, x| x * x + p);
            1.0 - (n / (a.sqrt() * b.sqrt()))
        } else {
            self.1
                .iter()
                .zip(other.1.iter())
                .fold(0.0, |p, (x, y)| (x - y).powi(2) + p)
                .sqrt()
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ModelData {
    time: SystemTime,
    label: String,
    id: FaceId,
    data: FaceEnc,
}

impl ModelData {
    pub fn new(time: SystemTime, label: String, id: FaceId, data: FaceEnc) -> Self {
        Self {
            time,
            label,
            id,
            data,
        }
    }

    fn from_json(v: &serde_json::Value) -> Result<Self> {
        Ok(ModelData {
            time: {
                let secs = v["time"]
                    .as_u64()
                    .ok_or_else(|| anyhow!("invalid 'time' in {v}"))?;
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs)
            },
            label: v["label"]
                .as_str()
                .ok_or_else(|| anyhow!("invalid 'label' in {v}"))?
                .to_string(),
            id: v["id"]
                .as_u64()
                .ok_or_else(|| anyhow!("invalid 'id' in {v}"))?,
            data: {
                let mut v = v;
                let frcg_mode = if v["data"].is_object() {
                    v = &v["data"]["emb"];
                    if v["data"]["model"] == "dlib" {
                        FRcgMode::Dlib
                    } else {
                        FRcgMode::SFace
                    }
                } else {
                    v = &v["data"];
                    // dlib by default
                    FRcgMode::Dlib
                };
                let mut arr = v
                    .as_array()
                    .ok_or_else(|| anyhow!("invalid 'data' in {v}"))?;
                // in case it is a nested array- extract first value
                // (needed to maintain compat with howdy)
                if let Some(inner) = arr
                    .first()
                    .ok_or_else(|| anyhow!("empty 'data' in {v}"))?
                    .as_array()
                {
                    arr = inner;
                }
                let emb = arr
                    .iter()
                    .map(|f| f.as_f64().ok_or_else(|| anyhow!("Invalid f64 {f}")))
                    .collect::<Result<Vec<f64>>>()?;
                FaceEnc(frcg_mode, emb)
            },
        })
    }

    pub(crate) fn as_json(&self) -> serde_json::Value {
        let time = self
            .time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        json!({
            "time": time,
            "label": self.label,
            "id": self.id,
            "data": {
                "model": if self.data.0 == FRcgMode::Dlib {"dlib"} else {"sface"},
                "emb": self.data.1
            }
        })
    }

    pub fn encoding(&self) -> &FaceEnc {
        &self.data
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Debug)]
pub(crate) struct Faces(Vec<ModelData>);

impl Faces {
    /// Parse the faces.json file
    pub(crate) fn from_file(path: &Path) -> Result<Self> {
        let f = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                // make new file
                std::fs::write(path, "[]")
                    .with_context(|| format!("couldn't create {}", path.display()))?;
                return Ok(Self(vec![]));
            }
            r => r.with_context(|| format!("{} not found", path.display()))?,
        };
        let rdr = BufReader::new(f);
        let encs: Vec<serde_json::Value> = serde_json::from_reader(rdr)
            .with_context(|| anyhow!("Failed to read json at {}", path.display()))?;

        Ok(Self(
            encs.iter()
                .map(ModelData::from_json)
                .collect::<Result<Vec<_>>>()?,
        ))
    }

    /// Parse the faces.json file
    pub(crate) fn to_file(&self, path: &Path) -> Result<()> {
        let f = File::options()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .with_context(|| format!("file {}", path.display()))?;
        let writer = BufWriter::new(f);
        let arr = self.0.iter().map(ModelData::as_json).collect::<Vec<_>>();
        serde_json::to_writer_pretty(writer, &serde_json::json!(arr))?;
        println!("written {} faces to {}", self.0.len(), path.display());
        Ok(())
    }

    pub(crate) fn is_empty(&self) -> bool {
        // In the future, we can check for a particular user
        self.0.is_empty()
    }

    pub(crate) fn add_face(&mut self, enc: FaceEnc, label: Option<String>) -> Result<()> {
        // TODO: Check if too similar
        let new_id = self.0.last().map_or(1, |d| d.id + 1);
        let data = ModelData {
            time: SystemTime::now(),
            label: label.unwrap_or_else(|| format!("Model #{new_id}")),
            id: new_id,
            data: enc,
        };
        self.0.push(data);
        Ok(())
    }

    pub(crate) fn check_match(&self, encoding: &FaceEnc, threshold: f64) -> Option<&ModelData> {
        log::info!("Checking against {} known faces", self.0.len());
        self.0
            .iter()
            .find(|known| known.encoding().distance(encoding) <= threshold)
            .inspect(|v| {
                log::debug!(target: "enc_match",
                    "Matched: {} with distance {}",
                    v.label,
                    v.encoding().distance(encoding)
                )
            })
    }
}
