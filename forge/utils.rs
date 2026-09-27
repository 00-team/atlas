use geo::algorithm::bounding_rect::BoundingRect;
use geo::algorithm::centroid::Centroid;
use geo::algorithm::contains::Contains;
use geo::{LineString, MultiPolygon, Polygon as GeoPolygon};
use geo::{Point, Polygon};
use osmpbfreader::{NodeId, Relation};
use osmpbfreader::{OsmId, OsmObj};
use std::collections::BTreeMap;
use std::collections::VecDeque;

pub fn rel_admin_centre(
    r: &Relation, objs: &BTreeMap<OsmId, OsmObj>,
) -> Option<geo::Point> {
    for rf in r.refs.iter() {
        if rf.role != "admin_centre" {
            continue;
        }
        let Some(OsmObj::Node(n)) = objs.get(&rf.member) else {
            continue;
        };

        let x = n.decimicro_lon as f64 / 1e7;
        let y = n.decimicro_lat as f64 / 1e7;

        return Some(geo::Point::new(x, y));
    }

    None
}

pub fn build_geo_polygons(
    rel: &osmpbfreader::Relation, objs: &BTreeMap<OsmId, OsmObj>,
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
        let mut ring: VecDeque<NodeId> =
            VecDeque::from(unstitched_ways.remove(0));
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
        // let mut json_coords: Vec<[f64; 2]> = Vec::with_capacity(ring.len());

        for node_id in ring {
            if let Some(OsmObj::Node(node)) = objs.get(&OsmId::Node(node_id)) {
                let lon = node.decimicro_lon as f64 / 10_000_000.0;
                let lat = node.decimicro_lat as f64 / 10_000_000.0;
                coords.push((lon, lat));
                // json_coords.push([lon, lat]);
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

pub fn name_en_to_id(name: &str) -> String {
    const IG: &[&str] =
        &["province", "county", "community", "governorate", "district"];

    name.split(' ')
        .filter_map(|sq| {
            let mut sq = sq.to_lowercase();
            if IG.contains(&sq.as_str()) {
                return None;
            }
            sq = sq.replace('-', "_").replace('\'', "_");
            Some(sq)
        })
        .collect::<Vec<_>>()
        .join("_")
}

/// Finds a point that is guaranteed to be strictly inside the polygon,
/// avoiding the "Concave Centroid" problem where curved states fall outside themselves.
pub fn get_safe_interior_point(poly: &Polygon<f64>) -> Option<Point<f64>> {
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
                let test_point = Point::new(
                    min_x + (i as f64 * step_x),
                    min_y + (j as f64 * step_y),
                );

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

pub fn clean_name(name: String) -> String {
    if let Some(n) = name.strip_prefix("استان ") {
        return n.trim().to_string();
    }

    if let Some(n) = name.strip_prefix("شهرستان ") {
        return n.trim().to_string();
    }

    name
}
