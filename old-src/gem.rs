use geo::algorithm::bounding_rect::BoundingRect;
use geo::algorithm::centroid::Centroid;
use geo::algorithm::contains::Contains;
use geo::{LineString, MultiPolygon, Polygon as GeoPolygon};
use geo::{Point, Polygon};
use osmpbfreader::NodeId;
use osmpbfreader::{OsmId, OsmObj, OsmPbfReader};
use std::collections::VecDeque;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;



type Area = Vec<Vec<Vec<[f64; 2]>>>;

#[derive(serde::Serialize, Clone)]
struct Sector {
    level: u8,
    name: String,
    area: Area,
    #[serde(skip)]
    poly: MultiPolygon<f64>,
    // regions: Vec<Region>,
    id: String,
    parent: String,
}


// #[derive(serde::Serialize, Clone)]
// struct Canton {
//     name: String,
//     area: Vec<GeoArea>,
//     id: String,
// }
//
// #[derive(serde::Serialize, Clone)]
// struct Region {
//     name: String,
//     area: Vec<GeoArea>,
//     // cantons: Vec<Canton>,
//     id: String,
// }
//
// #[derive(serde::Serialize)]
// struct Nation {
//     name: String,
//     area: Vec<GeoArea>,
//     // regions: Vec<Region>,
//     id: String,
// }

#[derive(serde::Serialize, serde::Deserialize)]
struct SectorDb {
    index: Vec<String>,
    nations: HashMap<String, Nation>,
}

fn main() {
    let filename = std::env::args().skip(1).next().expect("no filename");
    println!("loading: {filename}");

    // let filename = "data/armenia-latest.osm.pbf";
    let file = File::open(&filename).expect("Failed to open PBF");
    let mut pbf = OsmPbfReader::new(file);

    let en_locale = serde_json::from_str::<HashMap<String, String>>(
        &std::fs::read_to_string("data/en.json").unwrap(),
    )
    .unwrap();

    let objs = pbf
        .get_objs_and_deps(|obj| {
            let tags = obj.tags();
            let is_admin = tags.get("boundary").map(|s| s.as_str()) == Some("administrative");
            if !is_admin {
                return false;
            }
            let Some(admin_level) = tags.get("admin_level").map(|s| s.as_str()) else {
                return false;
            };

            admin_level == "2" || admin_level == "4" || admin_level == "5"
            // let is_target_admin =
            //     is_admin && (admin_level == Some("2") || admin_level == Some("4"));
            // // let is_city = obj.is_node()
            // //     && (tags.get("place").map(|s| s.as_str()) == Some("city")
            // //         || tags.get("place").map(|s| s.as_str()) == Some("town"));
            //
            // is_target_admin || is_city
        })
        .unwrap();

    let mut nations = Vec::new();
    let mut regions = Vec::new();
    let mut cantons = Vec::new();

    for (_, obj) in &objs {
        let tags = obj.tags();
        let name = tags.get("name").map(|s| s.to_string()).unwrap_or_default();
        let name_en = tags
            .get("name:en")
            .map(|s| s.to_string())
            .unwrap_or_else(|| en_locale.get(&name).map(|s| s.clone()).unwrap_or_default());

        let OsmObj::Relation(rel) = obj else { continue };

        let Some(admin_level) = tags.get("admin_level").map(|s| s.as_str()) else {
            continue;
        };

        if !["2", "4", "5"].contains(&admin_level) {
            continue;
        }

        let (area, poly) = build_geo_polygons(rel, &objs);
        if area.is_empty() {
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
                nations.push(Sector {
                    level: 1,
                    name,
                    area,
                    poly,
                    // regions: Vec::new(),
                    id,
                    parent: String::new(),
                });
            }
            "4" => {
                regions.push(Sector {
                    level: 2,
                    name,
                    area,
                    poly,
                    id,
                    parent: String::new(),
                    // cantons: Vec::new(),
                });
            }
            "5" => {
                cantons.push(Sector {
                    level: 3,
                    name,
                    area,
                    poly,
                    id,
                    parent: String::new(),
                });
            }
            _ => unreachable!(),
        }
    }

    println!("nations: {}", nations.len());
    println!("regions: {}", regions.len());
    println!("cantons: {}", cantons.len());

    if regions.is_empty() && nations.len() == 1 {
        let mut r = nations[0].clone();
        r.level = 2;
        regions.push(r);
    }

    println!("Assembling hierarchy natively via Point-in-Polygon...");

    let mut sectors = HashMap::<String, Sector>::new();

    for n in &nations {
        assert!(!n.id.is_empty());
        sectors.insert(n.id.clone(), n.clone());
    }

    for region in regions.iter_mut() {
        assert!(!region.id.is_empty());
        let Some(rc) = get_safe_interior_point(&region.poly[0]) else {
            println!("\x1b[31mERR: {}", region.name);
            continue;
        };

        let Some(nation) = nations.iter_mut().find(|c| c.poly.contains(&rc)) else {
            println!(
                "\x1b[93mWarning\x1b[m: State '{}' center fell completely outside all countries.",
                region.name
            );
            continue;
        };

        region.parent = nation.id.clone();
        region.id = format!("{}.{}", nation.id, region.id);
        if sectors.contains_key(&region.id) {
            println!("id {} alreay exists", region.id);
            return;
        }
        sectors.insert(region.id.clone(), region.clone());
        // nation.regions.push(region);
    }

    for canton in cantons.iter_mut() {
        assert!(!canton.id.is_empty());
        let Some(cc) = get_safe_interior_point(&canton.poly[0]) else {
            println!("\x1b[31mERR: {}", canton.name);
            continue;
        };

        let Some(pr) = regions.iter_mut().find(|s| s.poly.contains(&cc)) else {
            println!(
                "\x1b[93mWarning\x1b[m: County '{}' center fell completely outside all states.",
                canton.name
            );
            continue;
        };

        canton.id = format!("{}.{}", pr.id, canton.id);
        canton.parent = pr.id.clone();
        if sectors.contains_key(&canton.id) {
            println!("id {} alreay exists", canton.id);
            return;
        }
        sectors.insert(canton.id.clone(), canton.clone());
        // pr.cantons.push(canton);
    }

    println!("nations: {}", nations.len());
    println!("regions: {}", regions.len());
    println!("cantons: {}", cantons.len());
    println!("sectors: {}", sectors.len());

    let json_output = serde_json::to_string_pretty(&sectors).unwrap();
    std::fs::write("sectors.json", json_output).unwrap();
}

fn build_geo_polygons(
    rel: &osmpbfreader::Relation,
    objs: &BTreeMap<OsmId, OsmObj>,
) -> (Area, MultiPolygon<f64>) {
    // 1. Collect all "outer" ways as lists of NodeIds
    let mut unstitched_ways: Vec<Vec<NodeId>> = Vec::new();

    for ref_id in &rel.refs {
        if ref_id.role == "outer" || ref_id.role == "" {
            if let Some(OsmObj::Way(way)) = objs.get(&ref_id.member) {
                if !way.nodes.is_empty() {
                    unstitched_ways.push(way.nodes.clone());
                }
            }
        }
    }

    let mut areas = Vec::new();
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
            areas.push(vec![json_coords]);
        }
    }

    (areas, MultiPolygon::new(polies))
}

fn name_en_to_id(name: &str) -> String {
    name.split(' ')
        .filter_map(|sq| {
            let sq = sq.to_lowercase();
            if matches!(sq.as_str(), "province" | "county" | "community") {
                return None;
            }
            return Some(sq);
        })
        .collect::<Vec<_>>()
        .join("_")
}

/// Finds a point that is guaranteed to be strictly inside the polygon,
/// avoiding the "Concave Centroid" problem where curved states fall outside themselves.
fn get_safe_interior_point(poly: &Polygon<f64>) -> Option<Point<f64>> {
    // 1. Try the centroid first (fastest, works for 90% of shapes)
    if let Some(center) = poly.centroid() {
        if poly.contains(&center) {
            return Some(center);
        }
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
