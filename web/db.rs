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
        let raw = match std::fs::read(path) {
            Ok(v) => v,
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => return Ok(Self::default()),
                _ => return Err(e)?,
            },
        };

        Ok(serde_json::from_slice::<Self>(&raw)?)
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        let jj = serde_json::to_vec(self)?;
        std::fs::write(path, jj)?;
        Ok(())
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighwayKind {
    // Motorway,
    // Trunk,
    Primary,
    Secondary,
}

impl HighwayKind {
    pub fn from_tag(val: &str) -> Option<Self> {
        Some(match val {
            // "motorway" => Self::Motorway,
            // "trunk" => Self::Trunk,
            "primary" => Self::Primary,
            "secondary" => Self::Secondary,
            _ => return None,
        })
    }

    pub fn add_stats(&self, stats: &mut Stats) {
        match self {
            // Self::Motorway => stats.motorway += 1,
            // Self::Trunk => stats.trunk += 1,
            Self::Primary => stats.primary += 1,
            Self::Secondary => stats.secondary += 1,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceKind {
    City,
    Town,
    Suburb,
    Neighbourhood,
}

impl PlaceKind {
    pub fn from_tag(val: &str) -> Option<Self> {
        Some(match val {
            "city" => Self::City,
            "town" => Self::Town,
            "suburb" => Self::Suburb,
            "neighbourhood" => Self::Neighbourhood,
            _ => return None,
        })
    }

    pub fn add_stats(&self, stats: &mut Stats) {
        match self {
            Self::City => stats.city += 1,
            Self::Town => stats.town += 1,
            Self::Suburb => stats.suburb += 1,
            Self::Neighbourhood => stats.neighbourhood += 1,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "mark")]
pub enum Landmark {
    Highway {
        name: String,
        bbox: AABB<[f64; 2]>,
        kind: HighwayKind,
        canton: String,
        region: String,
        nation: String,
    },
    Place {
        name: String,
        // bbox: AABB<[f64; 2]>,
        lat: f64,
        lng: f64,
        kind: PlaceKind,
        canton: String,
        region: String,
        nation: String,
    },
}

#[derive(Debug, Default)]
pub struct Stats {
    primary: u32,
    secondary: u32,
    city: u32,
    town: u32,
    suburb: u32,
    neighbourhood: u32,
}
