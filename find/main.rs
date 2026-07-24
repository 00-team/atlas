use std::time::{Duration, Instant};

use geo::{Contains, MultiPolygon, Point};
use rstar::{AABB, RTree, RTreeObject};
use rstar::{Envelope, PointDistance};

mod db;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Sector {
    pub name: String,
    pub poly: MultiPolygon<f64>,
    pub level: u8,
    pub id: String,
    pub index: usize,
    pub region: String,
    pub nation: String,
    pub bounding_box: AABB<[f64; 2]>,
}

impl From<&db::Nation> for Sector {
    fn from(value: &db::Nation) -> Self {
        Self {
            name: value.name.clone(),
            poly: value.poly.clone(),
            level: 1,
            id: value.id.clone(),
            index: value.index,
            region: String::new(),
            nation: String::new(),
            bounding_box: value.bounding_box,
        }
    }
}

impl From<&db::Region> for Sector {
    fn from(value: &db::Region) -> Self {
        Self {
            name: value.name.clone(),
            poly: value.poly.clone(),
            level: 2,
            id: value.id.clone(),
            index: value.index,
            region: String::new(),
            nation: value.nation.clone(),
            bounding_box: value.bounding_box,
        }
    }
}

impl From<&db::Canton> for Sector {
    fn from(value: &db::Canton) -> Self {
        Self {
            name: value.name.clone(),
            poly: value.poly.clone(),
            level: 3,
            id: value.id.clone(),
            index: value.index,
            region: value.region.clone(),
            nation: value.nation.clone(),
            bounding_box: value.bounding_box,
        }
    }
}

impl RTreeObject for Sector {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.bounding_box
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
        self.poly.contains(&geo_point)
    }
}

pub struct GeoIndex {
    cantons: RTree<Sector>,
    regions: RTree<Sector>,
    nations: RTree<Sector>,
    sdb: db::SectorDb,
}

impl GeoIndex {
    pub fn load() -> Self {
        let sdb = db::SectorDb::load("sector-db.json").unwrap();

        let nc = sdb.nations.len();

        let mut nations = Vec::<Sector>::with_capacity(nc);
        let mut regions = Vec::<Sector>::with_capacity(nc * 50);
        let mut cantons = Vec::<Sector>::with_capacity(nc * 500);

        for n in sdb.nations.values() {
            nations.push(n.into());
            for r in n.regions.values() {
                regions.push(r.into());
                for c in r.cantons.values() {
                    cantons.push(c.into());
                }
            }
        }

        Self {
            sdb,
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
                if candidate.poly.contains(&search_point) {
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
    name: String,
    latitude: f64,
    longitude: f64,
    address: String,
}

fn main() {
    let gix = GeoIndex::load();

    let eats = serde_json::from_str::<Vec<Eatery>>(&std::fs::read_to_string("eats.json").unwrap())
        .unwrap();

    let mut count = 0;
    let mut dur = Duration::default();
    let mut found = 0;
    let mut lv3 = 0;
    let mut not_iran = 0;

    for eat in eats {
        if eat.longitude == 0.0 || eat.latitude == 0.0 {
            continue;
        }

        let start = Instant::now();
        if let Some(s) = gix.find_location(eat.latitude, eat.longitude) {
            found += 1;
            if s.level == 3 {
                lv3 += 1;
            }

            if s.nation != "iran" {
                println!("not iran: {} - {} - {}", s.nation, s.region, eat.gene);
                not_iran += 1;
            }
        } else {
            println!("\x1b[31mno sector\x1b[m for {eat:?}");
        }

        dur += start.elapsed();
        count += 1;
    }

    println!(
        "took: {:?} | {count} | {found} | {} | {} | {not_iran}",
        dur / count,
        count - found,
        found - lv3
    );
}
