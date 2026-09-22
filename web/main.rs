use osmpbfreader::{OsmId, OsmObj, OsmPbfReader, Tags};
use rstar::AABB;
use std::fs::File;

use crate::db::{HighwayKind, Landmark, PlaceKind};
use crate::error::AtlasError;

// #[derive(serde::Serialize, Clone)]
// struct Sector {
//     level: u8,
//     name: String,
//     area: Area,
//     #[serde(skip)]
//     poly: MultiPolygon<f64>,
//     // regions: Vec<Region>,
//     id: String,
//     parent: String,
// }

mod db;
mod error;

// const DB_PATH: &str = "sector-db.json";
const LANDMARKS_PATH: &str = "landmarks.json";

fn main() -> Result<(), AtlasError> {
    let filename = std::env::args().nth(1).expect("no filename");
    println!("loading: {filename}");

    // let sdb = SectorDb::load(DB_PATH)?;

    // let filename = "data/armenia-latest.osm.pbf";
    let file = File::open(&filename).expect("Failed to open PBF");
    let mut pbf = OsmPbfReader::new(file);

    fn is_node(tags: &Tags) -> bool {
        let Some(place) = tags.get("place") else { return false };
        let place = place.as_str();
        PlaceKind::from_tag(place).is_some()
    }

    fn is_way(tags: &Tags) -> bool {
        let Some(hw) = tags.get("highway") else { return false };
        let hw = hw.as_str();
        HighwayKind::from_tag(hw).is_some()
    }

    fn is_relevant(obj: &OsmObj) -> bool {
        let tags = obj.tags();
        if tags.get("name").is_none() {
            return false;
        }

        is_node(tags) || is_way(tags)
    }

    let objs = pbf.get_objs_and_deps(is_relevant)?;

    println!("found {} objects", objs.len());

    let mut landmarks = Vec::<Landmark>::with_capacity(2_000_000);

    // fn mpbb(mp: &MultiPolygon<f64>) -> rstar::AABB<[f64; 2]> {
    //     let rect = mp.bounding_rect().unwrap();
    //     rstar::AABB::from_corners([rect.min().x, rect.min().y], [rect.max().x, rect.max().y])
    // }

    let mut stats = db::Stats::default();
    let mut ignored = db::Stats::default();

    for obj in objs.values() {
        let tags = obj.tags();
        let Some(name) = tags.get("name").map(|s| s.to_string()) else {
            continue;
        };

        match obj {
            OsmObj::Node(n) => {
                let Some(place) = tags.get("place") else { continue };
                let Some(kind) = PlaceKind::from_tag(place.as_str()) else {
                    continue;
                };
                kind.add_stats(&mut stats);

                let lng = n.decimicro_lon as f64 / 10_000_000.0;
                let lat = n.decimicro_lat as f64 / 10_000_000.0;

                landmarks.push(Landmark::Place {
                    name,
                    lat,
                    lng,
                    kind,
                    canton: String::new(),
                    region: String::new(),
                    nation: String::new(),
                });
            }
            OsmObj::Way(w) => {
                let Some(hw) = tags.get("highway").map(|s| s.as_str()) else {
                    continue;
                };

                let Some(kind) = HighwayKind::from_tag(hw) else { continue };
                kind.add_stats(&mut stats);

                let mut blat = (999.0f64, 0.0f64);
                let mut blng = (999.0f64, 0.0f64);

                for &node_id in w.nodes.iter() {
                    if let Some(OsmObj::Node(node)) =
                        objs.get(&OsmId::Node(node_id))
                    {
                        let lng = node.decimicro_lon as f64 / 10_000_000.0;
                        let lat = node.decimicro_lat as f64 / 10_000_000.0;

                        blng.0 = blng.0.min(lng);
                        blng.1 = blng.1.max(lng);

                        blat.0 = blat.0.min(lat);
                        blat.1 = blat.1.max(lat);
                    }
                }

                landmarks.push(Landmark::Highway {
                    name,
                    bbox: AABB::from_corners(
                        [blng.0, blat.0],
                        [blng.1, blat.1],
                    ),
                    kind,
                    canton: String::new(),
                    region: String::new(),
                    nation: String::new(),
                });
            }
            OsmObj::Relation(r) => {
                let Some(place) = tags.get("place") else { continue };
                let Some(kind) = PlaceKind::from_tag(place.as_str()) else {
                    continue;
                };
                kind.add_stats(&mut ignored);
            }
        }
    }

    println!("landmarks: {}", landmarks.len());
    println!("stats: {stats:#?}");
    println!("ignored: {ignored:#?}");

    let lout = serde_json::to_vec(&landmarks)?;
    std::fs::write(LANDMARKS_PATH, lout)?;

    Ok(())
}
