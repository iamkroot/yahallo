use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use anyhow::{anyhow, Context, Ok, Result};
use dlib_face_recognition::FaceEncoding;
use serde_json::json;

type FaceId = u64;

#[derive(Debug)]
#[allow(dead_code)]
pub struct ModelData {
    time: u64,
    label: String,
    id: FaceId,
    data: FaceEncoding,
}

impl ModelData {
    pub fn new(time: u64, label: String, id: FaceId, data: FaceEncoding) -> Self {
        Self {
            time,
            label,
            id,
            data,
        }
    }

    fn from_json(v: &serde_json::Value) -> Result<Self> {
        Ok(ModelData {
            time: v["time"]
                .as_u64()
                .ok_or_else(|| anyhow!("invalid 'time' in {v}"))?,
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
        json!({
            "time": self.time,
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
        let f = File::open(path).context("read enc")?;
        let rdr = BufReader::new(f);
        let encs: serde_json::Value = serde_json::from_reader(rdr).context("read json")?;
        let a = encs
            .as_array()
            .ok_or_else(|| anyhow!("Failed to read json at {}", path.display()))?;

        Ok(Self(
            a.iter()
                .map(ModelData::from_json)
                .collect::<Result<Vec<_>>>()?,
        ))
    }

    /// Parse the faces.json file
    #[allow(dead_code)]
    pub(crate) fn to_file(&self, path: &Path) -> Result<()> {
        let f = File::open(path).context("writing file")?;
        let writer = BufWriter::new(f);
        let arr = self.0.iter().map(ModelData::as_json).collect::<Vec<_>>();
        serde_json::to_writer_pretty(writer, &serde_json::json!(arr))?;
        Ok(())
    }

    pub(crate) fn add_face(&mut self, data: ModelData) -> Result<()> {
        // TODO: Check if too similar
        // TODO: Check for ID conflicts
        self.0.push(data);
        Ok(())
    }

    pub(crate) fn check_match(
        &self,
        encoding: &FaceEncoding,
        threshold: f64,
    ) -> Option<&ModelData> {
        self.0
            .iter()
            .find(|known| known.encoding().distance(encoding) >= threshold)
    }
}
