use geo::{Contains, Distance, Haversine, Intersects, MultiPolygon};
use osmpbfreader::Tags;
use rstar::AABB;
use std::collections::HashMap;

use crate::sector::{SimpleCanton, SimpleNation, SimpleRegion};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Canton {
    pub name: String,
    pub poly: MultiPolygon<f64>,
    pub id: String,
    pub index: usize,
    pub region: String,
    pub nation: String,
    pub bounding_box: AABB<[f64; 2]>,
    pub center: Option<geo::Point>,
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
    pub center: Option<geo::Point>,
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
    pub center: Option<geo::Point>,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct SectorDb {
    pub index: Vec<String>,
    pub nations: HashMap<String, Nation>,
}

impl SectorDb {
    pub fn load(path: &str) -> std::io::Result<Self> {
        let raw = std::fs::read(path)?;
        Ok(serde_json::from_slice::<Self>(&raw)?)
    }

    pub fn into_simple(self) -> HashMap<String, SimpleNation> {
        let mut out = HashMap::with_capacity(self.nations.len());

        for (nid, n) in self.nations {
            let mut regions = HashMap::with_capacity(n.regions.len());
            for (rid, r) in n.regions {
                let mut cantons = HashMap::with_capacity(r.cantons.len());
                for (cid, c) in r.cantons {
                    cantons.insert(
                        cid,
                        SimpleCanton {
                            name: c.name,
                            region: c.region,
                            id: c.id,
                            nation: c.nation,
                            bounding_box: c.bounding_box.into(),
                            center: c.center,
                        },
                    );
                }

                regions.insert(
                    rid,
                    SimpleRegion {
                        name: r.name,
                        cantons,
                        id: r.id,
                        nation: r.nation,
                        bounding_box: r.bounding_box.into(),
                        center: r.center,
                    },
                );
            }

            out.insert(
                nid,
                SimpleNation {
                    name: n.name,
                    regions,
                    id: n.id,
                    bounding_box: n.bounding_box.into(),
                    center: n.center,
                },
            );
        }

        out
    }

    // pub fn save(&self, path: &str) -> std::io::Result<()> {
    //     let jj = serde_json::to_vec(self)?;
    //     std::fs::write(path, jj)?;
    //     Ok(())
    // }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[derive(PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum LandmarkKind {
    City,
    Town,
    Suburb,
    Neighbourhood,
    RoadPrimary,
    RoadSecondary,
}

impl LandmarkKind {
    pub fn from_tags(key: &str, tags: &Tags) -> Option<Self> {
        let val = tags.get(key)?.as_str();
        Some(match (key, val) {
            ("place", "city") => Self::City,
            ("place", "town") => Self::Town,
            ("place", "suburb") => Self::Suburb,
            ("place", "neighbourhood") => Self::Neighbourhood,
            ("highway", "primary") => Self::RoadPrimary,
            ("highway", "secondary") => Self::RoadSecondary,
            _ => return None,
        })
    }

    pub fn add_stats(&self, stats: &mut Stats) {
        match self {
            Self::City => stats.city += 1,
            Self::Town => stats.town += 1,
            Self::Suburb => stats.suburb += 1,
            Self::Neighbourhood => stats.neighbourhood += 1,
            Self::RoadPrimary => stats.road_primary += 1,
            Self::RoadSecondary => stats.road_secondary += 1,
        }
    }
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[derive(Clone, Copy)]
pub struct BoundingBox {
    // min: lat,lng
    a: f64,
    b: f64,
    // max: lat,lng
    c: f64,
    d: f64,
}

impl From<AABB<[f64; 2]>> for BoundingBox {
    fn from(value: AABB<[f64; 2]>) -> Self {
        let [minx, miny] = value.lower();
        let [maxx, maxy] = value.upper();

        Self {
            a: miny.min(maxy),
            b: minx.min(maxx),

            c: miny.max(maxy),
            d: minx.max(maxx),
        }
    }
}

impl BoundingBox {
    pub fn is_empty(&self) -> bool {
        self.a == 0.0 && self.b == 0.0 && self.c == 0.0 && self.d == 0.0
    }

    pub fn update(&mut self, lat: f64, lng: f64) {
        self.a = self.a.min(lat);
        self.b = self.b.min(lng);

        self.c = self.c.max(lat);
        self.d = self.d.max(lng);

        if self.a == 0.0 {
            self.a = lat;
        }
        if self.b == 0.0 {
            self.b = lng;
        }
    }

    fn merge(&mut self, other: &Self) {
        if other.a != 0.0 {
            self.a = self.a.min(other.a);
        }
        if other.b != 0.0 {
            self.b = self.b.min(other.b);
        }

        self.c = self.c.max(other.c);
        self.d = self.d.max(other.d);
    }

    // pub fn contains(&self, lat: f64, lng: f64) -> bool {
    //     lat >= self.a && lat <= self.c && lng >= self.b && lng <= self.d
    // }

    pub fn close_to(&self, other: &Self) -> bool {
        let margin = meters_to_degrees(1_000.0, (self.a + self.c) / 2.0);
        let a = self.as_rect(margin);
        let b = other.as_rect((0.0, 0.0));

        a.intersects(&b)
    }

    fn as_rect(&self, (mx, my): (f64, f64)) -> geo::Rect {
        let (minx, miny) = (self.b, self.a);
        let (maxx, maxy) = (self.d, self.c);

        geo::Rect::new([minx - mx, miny - my], [maxx + my, maxy + my])
    }
}

fn meters_to_degrees(meters: f64, latitude: f64) -> (f64, f64) {
    let lat = meters / 111_320.0;
    let lon = meters / (111_320.0 * latitude.to_radians().cos());
    (lat, lon)
}

#[derive(Debug, Default, serde::Serialize)]
#[derive(Clone, Copy)]
pub struct Location {
    #[serde(skip_serializing_if = "BoundingBox::is_empty")]
    pub bbox: BoundingBox,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center: Option<geo::Point>,
}

impl Location {
    pub fn is_close(&self, other: &Self) -> bool {
        if let Some(ac) = self.center {
            if let Some(bc) = other.center {
                return Haversine.distance(ac, bc) < 1_000.0;
            }

            return other.bbox.as_rect((0.0, 0.0)).contains(&ac);
        }

        if self.bbox.is_empty() {
            return false;
        }

        if let Some(bc) = other.center {
            return self.bbox.as_rect((0.0, 0.0)).contains(&bc);
        }

        self.bbox.close_to(&other.bbox)
    }

    pub fn merge(&mut self, other: &Self) {
        if self.bbox.is_empty() {
            self.bbox = other.bbox;
        } else if !other.bbox.is_empty() {
            self.bbox.merge(&other.bbox);
        }

        if self.center.is_none() {
            self.center = other.center;
        }
    }
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[derive(Hash, PartialEq, Eq, Clone)]
#[serde(default)]
pub struct LandmarkParent {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub nation: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub region: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub canton: String,
}

// impl LandmarkParent {
//     pub fn priority(&self) -> i32 {
//         if !self.canton.is_empty() {
//             3
//         } else if !self.region.is_empty() {
//             2
//         } else if !self.nation.is_empty() {
//             1
//         } else {
//             0
//         }
//     }
// }

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Landmark {
    #[serde(flatten)]
    pub loc: Location,
    pub kind: LandmarkKind,
    pub parent: LandmarkParent,
}

#[derive(Debug, Default)]
pub struct Stats {
    road_primary: u32,
    road_secondary: u32,
    city: u32,
    town: u32,
    suburb: u32,
    neighbourhood: u32,
    pub bbox: u32,
    pub coord: u32,
}
