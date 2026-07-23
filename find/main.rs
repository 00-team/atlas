use geo::{BoundingRect, Contains, LineString, MultiPolygon, Point, Polygon};
use rstar::{AABB, RTree, RTreeObject};
use rstar::{Envelope, PointDistance};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Sector {
    pub geometry: MultiPolygon<f64>,
    pub level: u8,
    pub name: String,
    pub id: String,
    pub parent: String,
    pub bounding_box: AABB<[f64; 2]>,
}

impl From<SectorData> for Sector {
    fn from(value: SectorData) -> Self {
        let mut polies = Vec::with_capacity(value.area.len());
        for p in &value.area {
            let ls = LineString::from(p[0].clone());
            polies.push(Polygon::new(ls, vec![]));
        }

        let mp = MultiPolygon::new(polies);

        let rect = mp.bounding_rect().unwrap();
        let bounding_box =
            AABB::from_corners([rect.min().x, rect.min().y], [rect.max().x, rect.max().y]);

        Self {
            geometry: mp,
            bounding_box,
            level: value.level,
            parent: value.parent,
            id: value.id,
            name: value.name,
        }
    }
}

impl PointDistance for Sector {
    // 1. How to calculate distance (used for nearest-neighbor searches)
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        // Defer to the bounding box's highly optimized distance calculation
        self.bounding_box.distance_2(point)
    }

    // 2. Exact containment check (used by locate_all_at_point)
    fn contains_point(&self, point: &[f64; 2]) -> bool {
        // First, verify it is inside the bounding box (super fast)
        if !self.bounding_box.contains_point(point) {
            return false;
        }

        // Second, perform the exact ray-casting on the polygon (computationally heavy)
        let geo_point = Point::new(point[0], point[1]);
        self.geometry.contains(&geo_point)
    }
}

#[derive(serde::Deserialize, Clone)]
struct SectorData {
    level: u8,
    name: String,
    area: Vec<Vec<Vec<[f64; 2]>>>,
    id: String,
    parent: String,
}

// 2. Implement RTreeObject so rstar knows how to index it
impl RTreeObject for Sector {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        // Extract the bounding box of the MultiPolygon
        let bounding_rect = self
            .geometry
            .bounding_rect()
            .expect("Geometry must have a bounding rect");

        AABB::from_corners(
            [bounding_rect.min().x, bounding_rect.min().y],
            [bounding_rect.max().x, bounding_rect.max().y],
        )
    }
}

pub struct GeoIndex {
    cantons: RTree<Sector>,
    regions: RTree<Sector>,
    nations: RTree<Sector>,
}

impl GeoIndex {
    pub fn load() -> Self {
        let raw = std::fs::read_to_string("sectors.json").unwrap();
        let sd = serde_json::from_str::<HashMap<String, SectorData>>(&raw).unwrap();

        let mut cantons = Vec::<Sector>::with_capacity(sd.len());
        let mut regions = Vec::<Sector>::with_capacity(sd.len());
        let mut nations = Vec::<Sector>::with_capacity(sd.len());

        for (_, s) in sd {
            match s.level {
                1 => nations.push(s.into()),
                2 => regions.push(s.into()),
                3 => cantons.push(s.into()),
                _ => unreachable!(),
            }
        }

        Self {
            nations: RTree::bulk_load(nations),
            regions: RTree::bulk_load(regions),
            cantons: RTree::bulk_load(cantons),
        }
    }

    /// Super fast lookup: Returns (Country, State, County)
    pub fn find_location(&self, lat: f64, lon: f64) -> Option<Sector> {
        // Note: geo/rstar typically uses (x, y) which maps to (Longitude, Latitude)

        fn find(sectors: &RTree<Sector>, lat: f64, lon: f64) -> Option<Sector> {
            let search_point = Point::new(lon, lat);
            let point_coords = [lon, lat];

            let candidates = sectors.locate_all_at_point(point_coords);

            for candidate in candidates {
                if candidate.geometry.contains(&search_point) {
                    return Some(candidate.clone());
                }
            }

            None
        }

        if let Some(v) = find(&self.cantons, lat, lon) {
            return Some(v);
        }

        if let Some(v) = find(&self.regions, lat, lon) {
            return Some(v);
        }

        if let Some(v) = find(&self.nations, lat, lon) {
            return Some(v);
        }

        None // Point is in the ocean, or unmapped territory
    }
}

#[derive(serde::Deserialize, Debug)]
struct Eatery {
    gene: String,
    // name: String,
    latitude: f64,
    longitude: f64,
    // address: String,
}

fn main() {
    let gix = GeoIndex::load();

    let eats = serde_json::from_str::<Vec<Eatery>>(&std::fs::read_to_string("eats.json").unwrap())
        .unwrap();

    for eat in eats {
        if eat.longitude == 0.0 || eat.latitude == 0.0 {
            continue;
        }

        if let Some(s) = gix.find_location(eat.latitude, eat.longitude) {
            println!("{} | {} | {}", s.level, eat.gene, s.id);
        } else {
            println!("\x1b[31mno sector\x1b[m for {eat:?}");
        }
    }
}
