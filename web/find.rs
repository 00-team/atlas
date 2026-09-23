use geo::{Contains, MultiPolygon, Point};
use rstar::{AABB, RTree, RTreeObject};
use rstar::{Envelope, PointDistance};

use crate::db::LandmarkParent;

use super::db;

#[derive(Clone)]
pub struct Sector {
    // pub name: String,
    pub poly: MultiPolygon<f64>,
    pub level: LandmarkParent,
    // pub index: usize,
    pub region: String,
    // pub nation: String,
    pub bounding_box: AABB<[f64; 2]>,
}

impl From<&db::Nation> for Sector {
    fn from(value: &db::Nation) -> Self {
        Self {
            // name: value.name.clone(),
            poly: value.poly.clone(),
            level: LandmarkParent::Nation(value.id.clone()),
            // index: value.index,
            region: String::new(),
            // nation: String::new(),
            bounding_box: value.bounding_box,
        }
    }
}

impl From<&db::Region> for Sector {
    fn from(value: &db::Region) -> Self {
        Self {
            // name: value.name.clone(),
            poly: value.poly.clone(),
            level: LandmarkParent::Region(value.id.clone()),
            // index: value.index,
            region: value.id.clone(),
            // nation: value.nation.clone(),
            bounding_box: value.bounding_box,
        }
    }
}

impl From<&db::Canton> for Sector {
    fn from(value: &db::Canton) -> Self {
        Self {
            // name: value.name.clone(),
            poly: value.poly.clone(),
            level: LandmarkParent::Canton(value.id.clone()),
            // index: value.index,
            region: value.region.clone(),
            // nation: value.nation.clone(),
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
}

impl GeoIndex {
    pub fn load(sdb: &db::SectorDb) -> Self {
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

        println!("nations: {}", nations.len());
        println!("regions: {}", regions.len());
        println!("cantons: {}", cantons.len());

        Self {
            nations: RTree::bulk_load(nations),
            regions: RTree::bulk_load(regions),
            cantons: RTree::bulk_load(cantons),
        }
    }

    pub fn find_location(&self, lat: f64, lon: f64) -> Option<Sector> {
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
