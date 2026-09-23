use osmpbfreader::{Node, OsmId, OsmObj, OsmPbfReader, Tags};
use std::collections::{HashMap, HashSet};
use std::fs::File;

use crate::db::{
    BoundingBox, Landmark, LandmarkKind, LandmarkParent, Location,
};
use crate::error::AtlasError;

mod db;
mod error;
mod find;
mod sector;

const SDB_PATH: &str = "sector-db.fmt.json";
// const LANDMARKS_PATH: &str = "landmarks.json";

fn main() -> Result<(), AtlasError> {
    let filename = std::env::args().nth(1).expect("no filename");
    println!("loading: {filename}");

    let sdb = db::SectorDb::load(SDB_PATH)?;
    let gdx = find::GeoIndex::load(&sdb);

    // let filename = "data/armenia-latest.osm.pbf";
    let file = File::open(&filename).expect("Failed to open PBF");
    let mut pbf = OsmPbfReader::new(file);

    fn is_node(tags: &Tags) -> bool {
        LandmarkKind::from_tags("place", tags).is_some()
    }

    fn is_way(tags: &Tags) -> bool {
        LandmarkKind::from_tags("highway", tags).is_some()
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

    let mut landmarks =
        HashMap::<String, HashMap<String, Vec<Landmark>>>::with_capacity(300);

    let mut add_lm = |name: String, region: String, lm: Landmark| {
        let r =
            landmarks.entry(region).or_insert(HashMap::with_capacity(20_000));

        let Some(v) = r.get_mut(&name) else {
            r.insert(name, vec![lm]);
            return;
        };

        for olm in v.iter_mut() {
            if olm.parent != lm.parent {
                continue;
            }

            if olm.kind != lm.kind || !olm.loc.is_close(&lm.loc) {
                continue;
            }

            olm.loc.merge(&lm.loc);
            return;
        }

        v.push(lm);
    };

    // fn mpbb(mp: &MultiPolygon<f64>) -> rstar::AABB<[f64; 2]> {
    //     let rect = mp.bounding_rect().unwrap();
    //     rstar::AABB::from_corners([rect.min().x, rect.min().y], [rect.max().x, rect.max().y])
    // }

    let mut stats = db::Stats::default();
    let mut seen_ids = HashSet::with_capacity(objs.len());

    for obj in objs.values() {
        let tags = obj.tags();
        let Some(name) = tags.get("name").map(|s| s.to_string()) else {
            continue;
        };

        let OsmObj::Relation(r) = obj else { continue };

        let Some(kind) = LandmarkKind::from_tags("place", tags) else {
            continue;
        };

        let mut center = None;
        let mut bbox = BoundingBox::default();

        let mut pf = HashMap::with_capacity(10);

        let mut bn = |n: &Node| {
            let lat = n.decimicro_lat as f64 / 1e7;
            let lng = n.decimicro_lon as f64 / 1e7;

            let Some(pp) = lat_lng_to_sector(lat, lng, &gdx) else { return };
            *pf.entry(pp).or_insert(0) += 1;

            bbox.update(lat, lng);
        };

        for rf in r.refs.iter() {
            seen_ids.insert(rf.member);

            if matches!(rf.role.as_str(), "admin_centre" | "label")
                && center.is_none()
            {
                let Some(OsmObj::Node(n)) = objs.get(&rf.member) else {
                    continue;
                };

                let x = n.decimicro_lon as f64 / 1e7;
                let y = n.decimicro_lat as f64 / 1e7;

                center = Some(Location::Point(geo::Point::new(x, y)));

                continue;
            }

            if rf.role != "outer" {
                continue;
            }

            match objs.get(&rf.member) {
                Some(OsmObj::Way(w)) => {
                    for &nid in w.nodes.iter() {
                        let Some(OsmObj::Node(n)) = objs.get(&OsmId::Node(nid))
                        else {
                            continue;
                        };

                        bn(n);
                    }
                }
                Some(OsmObj::Node(n)) => bn(n),
                _ => {}
            }
        }

        let (loc, parent, region) = if !bbox.is_empty() {
            stats.bbox += 1;
            let Some((parent, region)) = get_max_parent(&pf) else { continue };
            (Location::Bbox(bbox), parent, region)
        } else if let Some(Location::Point(p)) = center {
            stats.coord += 1;
            let Some((p, r)) = lat_lng_to_sector(p.y(), p.x(), &gdx) else {
                continue;
            };
            (center.unwrap(), p, r)
        } else {
            continue;
        };

        kind.add_stats(&mut stats);
        add_lm(name, region, Landmark { loc, kind, parent })
    }

    for (id, obj) in objs.iter() {
        let tags = obj.tags();
        let Some(name) = tags.get("name").map(|s| s.to_string()) else {
            continue;
        };

        if seen_ids.contains(id) {
            continue;
        }

        match obj {
            OsmObj::Node(n) => {
                let Some(kind) = LandmarkKind::from_tags("place", tags) else {
                    continue;
                };

                let x = n.decimicro_lon as f64 / 1e7;
                let y = n.decimicro_lat as f64 / 1e7;

                let loc = Location::Point(geo::Point::new(x, y));

                kind.add_stats(&mut stats);

                stats.coord += 1;
                let Some((parent, rg)) = lat_lng_to_sector(y, x, &gdx) else {
                    continue;
                };
                add_lm(name, rg, Landmark { loc, kind, parent });
            }
            OsmObj::Way(w) => {
                let Some(kind) = LandmarkKind::from_tags("highway", tags)
                else {
                    continue;
                };

                let mut bbox = BoundingBox::default();

                let mut pf = HashMap::with_capacity(10);

                for &nid in w.nodes.iter() {
                    if let Some(OsmObj::Node(n)) = objs.get(&OsmId::Node(nid)) {
                        let lng = n.decimicro_lon as f64 / 1e7;
                        let lat = n.decimicro_lat as f64 / 1e7;

                        let Some(p) = lat_lng_to_sector(lat, lng, &gdx) else {
                            continue;
                        };
                        *pf.entry(p).or_insert(0) += 1;

                        bbox.update(lat, lng);
                    }
                }

                let Some((parent, rg)) = get_max_parent(&pf) else { continue };

                stats.bbox += 1;
                let loc = Location::Bbox(bbox);

                kind.add_stats(&mut stats);
                add_lm(name, rg, Landmark { loc, kind, parent });
            }
            _ => {}
        }
    }

    let mut total_land = 0;
    for r in landmarks.values() {
        for lms in r.values() {
            total_land += lms.len();
        }
    }

    println!("landmarks: {} | {total_land}", landmarks.len());
    println!("stats: {stats:#?}");

    let _ = std::fs::create_dir("landmarks");
    for (r, lms) in landmarks.iter() {
        let lout = serde_json::to_vec_pretty(lms)?;
        std::fs::write(format!("landmarks/{r}.json"), lout)?;
    }
    std::fs::write(
        format!("landmarks/simple-sector.json"),
        serde_json::to_vec_pretty(&sdb.to_simple())?,
    )?;

    Ok(())
}

fn lat_lng_to_sector(
    lat: f64, lng: f64, gdx: &find::GeoIndex,
) -> Option<(LandmarkParent, String)> {
    // let (lat, lng) = (lat as f64 / 1e7, lng as f64 / 1e7);

    let Some(sector) = gdx.find_location(lat, lng) else {
        return None;
    };

    Some((sector.level, sector.region))
}

fn get_max_parent(
    map: &HashMap<(LandmarkParent, String), i32>,
) -> Option<(LandmarkParent, String)> {
    map.iter()
        .max_by(|(pa, va), (pb, vb)| {
            pa.0.priority().cmp(&pb.0.priority()).then_with(|| va.cmp(vb))
        })
        .map(|(a, _)| a.clone())
}
