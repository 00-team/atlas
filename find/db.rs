use geo::MultiPolygon;
use rstar::AABB;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Canton {
    pub name: String,
    pub poly: MultiPolygon<f64>,
    pub id: String,
    pub index: usize,
    pub region: String,
    pub nation: String,
    pub bounding_box: AABB<[f64; 2]>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Region {
    pub name: String,
    pub poly: MultiPolygon<f64>,
    pub id: String,
    pub index: usize,
    pub cantons: HashMap<String, Canton>,
    pub canton_index: Vec<String>,
    pub nation: String,
    pub bounding_box: AABB<[f64; 2]>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Nation {
    pub name: String,
    pub poly: MultiPolygon<f64>,
    pub id: String,
    pub index: usize,
    pub regions: HashMap<String, Region>,
    pub regions_index: Vec<String>,
    pub bounding_box: AABB<[f64; 2]>,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct SectorDb {
    pub index: Vec<String>,
    pub nations: HashMap<String, Nation>,
}

impl SectorDb {
    pub fn load(path: &str) -> std::io::Result<Self> {
        let raw = match std::fs::read_to_string(path) {
            Ok(v) => v,
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => return Ok(Self::default()),
                _ => return Err(e)?,
            },
        };
        Ok(serde_json::from_str::<Self>(&raw)?)
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        let jj = serde_json::to_string(self)?;
        std::fs::write(path, jj)?;
        Ok(())
    }
}
