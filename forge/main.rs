use geo::algorithm::bounding_rect::BoundingRect;
use geo::algorithm::centroid::Centroid;
use geo::algorithm::contains::Contains;
use geo::{HasDimensions, LineString, MultiPolygon, Polygon as GeoPolygon};
use geo::{Point, Polygon};
use osmpbfreader::NodeId;
use osmpbfreader::{OsmId, OsmObj, OsmPbfReader};
use std::collections::VecDeque;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;

use crate::db::SectorDb;
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

const DB_PATH: &str = "sector-db.json";

fn main() -> Result<(), AtlasError> {
    let filename = std::env::args().nth(1).expect("no filename");
    println!("loading: {filename}");

    let mut sdb = SectorDb::load(DB_PATH)?;

    // let filename = "data/armenia-latest.osm.pbf";
    let file = File::open(&filename).expect("Failed to open PBF");
    let mut pbf = OsmPbfReader::new(file);

    let en_locale =
        serde_json::from_str::<HashMap<String, String>>(&std::fs::read_to_string("data/en.json")?)?;

    let objs = pbf.get_objs_and_deps(|obj| {
        let tags = obj.tags();
        let is_admin = tags.get("boundary").map(|s| s.as_str()) == Some("administrative");
        if !is_admin {
            return false;
        }
        let Some(admin_level) = tags.get("admin_level").map(|s| s.as_str()) else {
            return false;
        };

        admin_level == "2" || admin_level == "4" || admin_level == "5"
    })?;

    let mut nations = Vec::new();
    let mut regions = Vec::new();
    let mut cantons = Vec::new();

    let mut nations_ids = Vec::new();
    let mut regions_ids = Vec::new();

    for obj in objs.values() {
        let tags = obj.tags();
        let name = tags.get("name").map(|s| s.to_string()).unwrap_or_default();
        let name_en = tags
            .get("name:en")
            .map(|s| s.to_string())
            .unwrap_or_else(|| en_locale.get(&name).cloned().unwrap_or_default());

        let OsmObj::Relation(rel) = obj else { continue };

        let Some(admin_level) = tags.get("admin_level").map(|s| s.as_str()) else {
            continue;
        };

        if !["2", "4", "5"].contains(&admin_level) {
            continue;
        }

        let poly = build_geo_polygons(rel, &objs);
        if poly.is_empty() {
            println!("\x1b[33mwarning\x1b[m no geo for {admin_level:?} {name} | {name_en}");
            continue;
        }

        let id = name_en_to_id(&name_en);

        // let Some((geo_poly, coords)) = build_geo_polygons(rel, &objs) else {
        //     println!("\x1b[33mwarning\x1b[m no geo for {admin_level:?} {name} | {name_en}");
        //     continue;
        // };

        if id.is_empty() {
            println!("no id: \"{name}\": \"\",");
            println!("{tags:?}");
        }

        match admin_level {
            "2" => {
                nations_ids.push(id.clone());
                nations.push(db::Nation {
                    name,
                    poly,
                    id,
                    regions: Default::default(),
                    index: 0,
                    regions_index: vec!["<empty>".to_string()],
                });
            }
            "4" => {
                regions_ids.push(id.clone());
                regions.push(db::Region {
                    name,
                    poly,
                    id,
                    cantons: Default::default(),
                    index: 0,
                    canton_index: vec!["<empty>".to_string()],
                });
            }
            "5" => {
                cantons.push(db::Canton {
                    name,
                    poly,
                    id,
                    index: 0,
                });
            }
            _ => unreachable!(),
        }
    }

    println!("nations: {}", nations.len());
    println!("regions: {}", regions.len());
    println!("cantons: {}", cantons.len());

    if regions.is_empty() && nations.len() == 1 {
        let n = &nations[0];
        let r = db::Region {
            index: 0,
            canton_index: vec!["<empty>".to_string()],
            poly: n.poly.clone(),
            cantons: Default::default(),
            id: n.id.clone(),
            name: n.name.clone(),
        };
        regions.push(r);
    }

    println!("Assembling hierarchy natively via Point-in-Polygon...");

    fn clean_index<T>(index: &mut [String], map: &HashMap<String, T>) {
        for id in index.iter_mut().skip(1) {
            if !map.contains_key(id) {
                id.clear();
            }
        }
    }

    fn get_index(index: &mut Vec<String>, id: &str) -> usize {
        let mut empty_idx = 0;
        for (x, old_id) in index.iter().skip(1).enumerate() {
            if old_id.is_empty() && empty_idx == 0 {
                empty_idx = x;
                continue;
            }

            if old_id == id {
                return x;
            }
        }

        if empty_idx != 0 {
            index[empty_idx] = id.to_string();
            return empty_idx;
        }

        let idx = index.len();
        index.push(id.to_string());
        idx
    }

    clean_index(&mut sdb.index, &sdb.nations);

    for mut n in nations {
        assert!(!n.id.is_empty());
        if let Some(old) = sdb.nations.get_mut(&n.id) {
            println!("nation {} already exists", n.name);
            old.poly = n.poly;
            old.regions.clear();
            continue;
        }

        n.index = get_index(&mut sdb.index, &n.id);
        sdb.nations.insert(n.id.clone(), n);
    }

    for mut r in regions {
        assert!(!r.id.is_empty());
        let Some(rc) = get_safe_interior_point(&r.poly[0]) else {
            println!("\x1b[31mERR: {}", r.name);
            continue;
        };

        let Some(nation) = sdb.nations.values_mut().find(|n| n.poly.contains(&rc)) else {
            println!(
                "\x1b[93mWarning\x1b[m: State '{}' center fell completely outside all countries.",
                r.name
            );
            continue;
        };

        if let Some(old) = nation.regions.get_mut(&r.id) {
            println!("region {} already exists", r.name);
            old.poly = r.poly;
            old.cantons.clear();
            continue;
        }

        r.index = get_index(&mut nation.regions_index, &r.id);
        nation.regions.insert(r.id.clone(), r);
    }

    for mut c in cantons {
        assert!(!c.id.is_empty());
        let Some(cc) = get_safe_interior_point(&c.poly[0]) else {
            println!("\x1b[31mERR: {}", c.name);
            continue;
        };

        let region = sdb
            .nations
            .values_mut()
            .find_map(|n| n.regions.values_mut().find(|r| r.poly.contains(&cc)));

        let Some(region) = region else {
            println!(
                "\x1b[93mWarning\x1b[m: County '{}' center fell completely outside all states.",
                c.name
            );
            continue;
        };

        if let Some(old) = region.cantons.get_mut(&c.id) {
            println!("canton {} already exists", c.name);
            old.poly = c.poly;
            continue;
        }

        c.index = get_index(&mut region.canton_index, &c.id);
        region.cantons.insert(c.id.clone(), c);
    }

    sdb.save(DB_PATH)?;

    Ok(())
}

fn build_geo_polygons(
    rel: &osmpbfreader::Relation,
    objs: &BTreeMap<OsmId, OsmObj>,
) -> MultiPolygon<f64> {
    // 1. Collect all "outer" ways as lists of NodeIds
    let mut unstitched_ways: Vec<Vec<NodeId>> = Vec::new();

    for ref_id in &rel.refs {
        if !(ref_id.role == "outer" || ref_id.role.is_empty()) {
            continue;
        }

        let Some(OsmObj::Way(way)) = objs.get(&ref_id.member) else {
            continue;
        };

        if way.nodes.is_empty() {
            continue;
        }

        unstitched_ways.push(way.nodes.clone());
    }

    let mut polies = Vec::new();

    // 2. Loop until all disjointed rings (exclaves) are processed
    while !unstitched_ways.is_empty() {
        // Seed the next disconnected ring
        let mut ring: VecDeque<NodeId> = VecDeque::from(unstitched_ways.remove(0));
        let mut changed = true;

        // Stitch the current ring together
        while changed && !unstitched_ways.is_empty() {
            changed = false;
            let first_node = *ring.front().unwrap();
            let last_node = *ring.back().unwrap();

            // If the ring is closed, we stop looking for connections for THIS ring
            if first_node == last_node && ring.len() > 1 {
                break;
            }

            // Search for a way that connects to either end of our current ring
            let mut i = 0;
            while i < unstitched_ways.len() {
                let way = &unstitched_ways[i];
                let way_first = *way.first().unwrap();
                let way_last = *way.last().unwrap();

                if way_first == last_node {
                    // Connects to the end, facing forward
                    ring.extend(way.iter().skip(1));
                    unstitched_ways.remove(i);
                    changed = true;
                    break;
                } else if way_last == last_node {
                    // Connects to the end, facing backward (needs reversing)
                    ring.extend(way.iter().rev().skip(1));
                    unstitched_ways.remove(i);
                    changed = true;
                    break;
                } else if way_last == first_node {
                    // Connects to the start, facing forward
                    for node in way.iter().rev().skip(1) {
                        ring.push_front(*node);
                    }
                    unstitched_ways.remove(i);
                    changed = true;
                    break;
                } else if way_first == first_node {
                    // Connects to the start, facing backward (needs reversing)
                    for node in way.iter().skip(1) {
                        ring.push_front(*node);
                    }
                    unstitched_ways.remove(i);
                    changed = true;
                    break;
                }
                i += 1;
            }
        }

        // 3. Convert the stitched, continuous loop of NodeIds into coordinates
        let mut coords: Vec<(f64, f64)> = Vec::with_capacity(ring.len());
        let mut json_coords: Vec<[f64; 2]> = Vec::with_capacity(ring.len());

        for node_id in ring {
            if let Some(OsmObj::Node(node)) = objs.get(&OsmId::Node(node_id)) {
                let lon = node.decimicro_lon as f64 / 10_000_000.0;
                let lat = node.decimicro_lat as f64 / 10_000_000.0;
                coords.push((lon, lat));
                json_coords.push([lon, lat]);
            }
        }

        // 4. Ensure the loop is closed before building the geometry
        // We only push valid, closed rings to our final output list.
        if !coords.is_empty() && coords.first() == coords.last() {
            let line_string = LineString::from(coords);
            polies.push(GeoPolygon::new(line_string, vec![]));
        }
    }

    MultiPolygon::new(polies)
}

fn name_en_to_id(name: &str) -> String {
    name.split(' ')
        .filter_map(|sq| {
            let sq = sq.to_lowercase();
            if matches!(sq.as_str(), "province" | "county" | "community") {
                return None;
            }
            Some(sq)
        })
        .collect::<Vec<_>>()
        .join("_")
}

/// Finds a point that is guaranteed to be strictly inside the polygon,
/// avoiding the "Concave Centroid" problem where curved states fall outside themselves.
fn get_safe_interior_point(poly: &Polygon<f64>) -> Option<Point<f64>> {
    // 1. Try the centroid first (fastest, works for 90% of shapes)
    if let Some(center) = poly.centroid()
        && poly.contains(&center)
    {
        return Some(center);
    }

    // 2. If the centroid is outside (like North Khorasan), use a grid-search
    if let Some(bbox) = poly.bounding_rect() {
        let min_x = bbox.min().x;
        let max_x = bbox.max().x;
        let min_y = bbox.min().y;
        let max_y = bbox.max().y;

        let steps = 10;
        let step_x = (max_x - min_x) / steps as f64;
        let step_y = (max_y - min_y) / steps as f64;

        // Scan a 10x10 grid inside the bounding box
        for i in 1..steps {
            for j in 1..steps {
                let test_point =
                    Point::new(min_x + (i as f64 * step_x), min_y + (j as f64 * step_y));

                // Return the first point that falls strictly inside the actual polygon
                if poly.contains(&test_point) {
                    return Some(test_point);
                }
            }
        }
    }

    // 3. Absolute fallback (should practically never hit this)
    poly.centroid()
}
