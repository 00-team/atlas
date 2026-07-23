use crate::error::AtlasError;
use geo::MultiPolygon;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Canton {
    pub name: String,
    pub poly: MultiPolygon<f64>,
    pub id: String,
    pub index: usize,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Region {
    pub name: String,
    pub poly: MultiPolygon<f64>,
    pub id: String,
    pub index: usize,
    pub cantons: HashMap<String, Canton>,
    pub canton_index: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Nation {
    pub name: String,
    pub poly: MultiPolygon<f64>,
    pub id: String,
    pub index: usize,
    pub regions: HashMap<String, Region>,
    pub regions_index: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct SectorDb {
    pub index: Vec<String>,
    pub nations: HashMap<String, Nation>,
}

impl SectorDb {
    pub fn load(path: &str) -> Result<Self, AtlasError> {
        let raw = match std::fs::read_to_string(path) {
            Ok(v) => v,
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => return Ok(Self::default()),
                _ => return Err(e)?,
            },
        };
        Ok(serde_json::from_str::<Self>(&raw)?)
    }

    pub fn save(&self, path: &str) -> Result<(), AtlasError> {
        let jj = serde_json::to_string(self)?;
        std::fs::write(path, jj)?;
        Ok(())
    }
}
