use std::fs::File;
use std::io::{BufReader, BufWriter, ErrorKind};
use std::path::Path;
use std::time::SystemTime;

use anyhow::{anyhow, Context, Result};
use dlib_face_recognition::FaceEncoding;
use serde_json::json;

type FaceId = u64;

#[derive(Debug)]
#[allow(dead_code)]
pub struct ModelData {
    time: SystemTime,
    label: String,
    id: FaceId,
    data: FaceEncoding,
}

impl ModelData {
    pub fn new(time: SystemTime, label: String, id: FaceId, data: FaceEncoding) -> Self {
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
                let mut arr = v["data"]
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
                let v = arr
                    .iter()
                    .map(|f| f.as_f64().ok_or_else(|| anyhow!("Invalid f64 {f}")))
                    .collect::<Result<Vec<f64>>>()?;
                FaceEncoding::from_vec(&v).map_err(|e| anyhow!("Invalid face encoding: {e}"))?
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
            "data": self.data.as_ref()
        })
    }

    pub fn encoding(&self) -> &FaceEncoding {
        &self.data
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

    pub(crate) fn add_face(&mut self, enc: FaceEncoding, label: Option<String>) -> Result<()> {
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

    pub(crate) fn check_match(
        &self,
        encoding: &FaceEncoding,
        threshold: f64,
    ) -> Option<&ModelData> {
        log::info!("Checking against {} known faces", self.0.len());
        self.0
            .iter()
            .find(|known| known.encoding().distance(encoding) <= threshold)
    }
}
