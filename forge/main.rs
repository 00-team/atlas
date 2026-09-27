use geo::algorithm::bounding_rect::BoundingRect;
use geo::algorithm::contains::Contains;
use geo::{Area, HasDimensions, MultiPolygon};
use osmpbfreader::Tags;
use osmpbfreader::{OsmObj, OsmPbfReader};
use std::collections::HashMap;
use std::fs::File;

use crate::db::SectorDb;
use crate::error::AtlasError;
use crate::utils::rel_admin_centre;
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
mod utils;

const DB_PATH: &str = "sector-db.json";

const NATION_LVL: &str = "2";
const REGION_LVL: &str = "4";
const CANTON_LVL: &str = "5";

fn main() -> Result<(), AtlasError> {
    let filename = std::env::args().nth(1).expect("no filename");
    println!("loading: {filename}");

    let mut sdb = SectorDb::load(DB_PATH)?;

    // let filename = "data/armenia-latest.osm.pbf";
    let file = File::open(&filename).expect("Failed to open PBF");
    let mut pbf = OsmPbfReader::new(file);

    let en_locale = serde_json::from_str::<HashMap<String, String>>(
        &std::fs::read_to_string("data/en.json")?,
    )?;

    fn get_admin_level(tags: &Tags) -> Option<&str> {
        let bd = tags.get("boundary")?;
        if bd != "administrative" {
            return None;
        }

        let al = tags.get("admin_level")?.as_str();

        if !matches!(al, NATION_LVL | REGION_LVL | CANTON_LVL) {
            return None;
        }

        Some(al)
    }

    let objs = pbf.get_objs_and_deps(|obj| {
        let tags = obj.tags();

        let is_city = tags
            .get("place")
            .map(|s| matches!(s.as_str(), "city" | "town"))
            .unwrap_or_default();

        is_city || get_admin_level(tags).is_some()
    })?;

    let mut nations = Vec::new();
    let mut regions = Vec::new();
    let mut cantons = Vec::new();

    let mut nations_ids = Vec::new();
    let mut regions_ids = Vec::new();

    let mut towns = HashMap::with_capacity(10_000);

    fn mpbb(mp: &MultiPolygon<f64>) -> rstar::AABB<[f64; 2]> {
        let rect = mp.bounding_rect().unwrap();
        rstar::AABB::from_corners(
            [rect.min().x, rect.min().y],
            [rect.max().x, rect.max().y],
        )
    }

    for obj in objs.values() {
        let tags = obj.tags();
        let Some(name) = tags.get("name") else { continue };
        let Some(place) = tags.get("place") else { continue };
        if !matches!(place.as_str(), "city" | "town") {
            continue;
        }
        let name = utils::clean_name(tools::text_normalize(name.as_str()));

        let p = match obj {
            OsmObj::Node(node) => {
                let x = node.decimicro_lon as f64 / 1e7;
                let y = node.decimicro_lat as f64 / 1e7;
                geo::Point::new(x, y)
            }
            OsmObj::Relation(r) => {
                let Some(p) = rel_admin_centre(r, &objs) else { continue };
                p
            }
            _ => continue,
        };

        towns.insert(name, p);
    }

    for obj in objs.values() {
        let tags = obj.tags();
        let Some(name) = tags.get("name") else { continue };
        let name = utils::clean_name(tools::text_normalize(name.as_str()));
        let name_en =
            tags.get("name:en").map(|s| s.to_string()).unwrap_or_else(|| {
                en_locale.get(&name).cloned().unwrap_or_default()
            });

        let OsmObj::Relation(rel) = obj else { continue };

        let Some(admin_level) = tags.get("admin_level").map(|s| s.as_str())
        else {
            continue;
        };

        if ![NATION_LVL, REGION_LVL, CANTON_LVL].contains(&admin_level) {
            continue;
        }

        let mut center = rel_admin_centre(rel, &objs);

        let poly = utils::build_geo_polygons(rel, &objs);
        if poly.is_empty() {
            println!(
                "\x1b[33mwarning\x1b[m no geo for {admin_level:?} {name} | {name_en}"
            );
            continue;
        }

        if center.is_none() {
            center = towns.get(&name).cloned();
        }

        // if center.is_none() {
        //     println!("\x1b[93mstill no center for\x1b[m {name}: {id:?}");
        // }

        let id = utils::name_en_to_id(&name_en);

        // let Some((geo_poly, coords)) = build_geo_polygons(rel, &objs) else {
        //     println!("\x1b[33mwarning\x1b[m no geo for {admin_level:?} {name} | {name_en}");
        //     continue;
        // };

        if id.is_empty() {
            println!("no id: \"{name}\": \"\",");
            println!("{tags:?}");
        }

        match admin_level {
            NATION_LVL => {
                nations_ids.push(id.clone());
                nations.push(db::Nation {
                    name,
                    bounding_box: mpbb(&poly),
                    poly,
                    id,
                    regions: Default::default(),
                    index: 0,
                    regions_index: vec!["<empty>".to_string()],
                    center,
                });
            }
            REGION_LVL => {
                regions_ids.push(id.clone());
                regions.push(db::Region {
                    name,
                    bounding_box: mpbb(&poly),
                    poly,
                    id,
                    cantons: Default::default(),
                    index: 0,
                    canton_index: vec!["<empty>".to_string()],
                    nation: String::new(),
                    center,
                });
            }
            CANTON_LVL => {
                cantons.push(db::Canton {
                    name,
                    bounding_box: mpbb(&poly),
                    poly,
                    id,
                    index: 0,
                    nation: String::new(),
                    region: String::new(),
                    center,
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
            nation: n.id.clone(),
            bounding_box: mpbb(&n.poly),
            center: n.center,
        };
        regions.push(r);
    }

    println!("Assembling hierarchy natively via Point-in-Polygon...");

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

        if index.is_empty() {
            index.push("<empty>".to_string());
        }

        let idx = index.len();
        index.push(id.to_string());
        idx
    }

    fn poly_cmp(a: &MultiPolygon<f64>, b: &MultiPolygon<f64>) -> bool {
        a.unsigned_area() >= b.unsigned_area()
    }

    for mut n in nations {
        assert!(!n.id.is_empty());
        if let Some(old) = sdb.nations.get_mut(&n.id) {
            if !poly_cmp(&n.poly, &old.poly) {
                println!("\x1b[33mnation\x1b[m {} already exists", n.name);
            } else {
                println!("\x1b[32mupdating nation\x1b[m {}", n.name);
                old.poly = n.poly;
                old.regions.clear();
            }
            continue;
        }

        n.index = get_index(&mut sdb.index, &n.id);
        sdb.nations.insert(n.id.clone(), n);
    }

    for mut r in regions {
        assert!(!r.id.is_empty());
        let Some(rc) = utils::get_safe_interior_point(&r.poly[0]) else {
            println!("\x1b[31mERR: {}", r.name);
            continue;
        };

        let nation_id =
            nations_ids.iter().find(|&id| sdb.nations[id].poly.contains(&rc));

        let Some(nation_id) = nation_id else {
            println!(
                "\x1b[93mWarning\x1b[m: Region '{}' center fell completely outside all Nations.",
                r.name
            );
            continue;
        };

        let nation = sdb.nations.get_mut(nation_id).unwrap();

        if let Some(old) = nation.regions.get_mut(&r.id) {
            if !poly_cmp(&r.poly, &old.poly) {
                println!("\x1b[33mregion\x1b[m {} already exists", r.name);
            } else {
                println!("\x1b[32mupdating region\x1b[m {}", r.name);
                old.poly = r.poly;
                old.cantons.clear();
            }
            continue;
        }

        r.index = get_index(&mut nation.regions_index, &r.id);
        r.nation = nation.id.clone();
        nation.regions.insert(r.id.clone(), r);
    }

    for mut c in cantons {
        assert!(!c.id.is_empty());
        let Some(cc) = utils::get_safe_interior_point(&c.poly[0]) else {
            println!("\x1b[31mERR: {}", c.name);
            continue;
        };

        let nid =
            nations_ids.iter().find(|&id| sdb.nations[id].poly.contains(&cc));

        let Some(nation_id) = nid else {
            println!(
                "\x1b[93mWarning\x1b[m: Canton '{}' center fell completely outside all Nations.",
                c.name
            );
            continue;
        };

        let nation = sdb.nations.get_mut(nation_id).unwrap();
        let region = nation.regions.values_mut().find(|r| r.poly.contains(&cc));

        let Some(region) = region else {
            println!(
                "\x1b[93mWarning\x1b[m: canton '{}' center fell completely outside all regions.",
                c.name
            );
            continue;
        };

        if let Some(old) = region.cantons.get_mut(&c.id) {
            if !poly_cmp(&c.poly, &old.poly) {
                println!("\x1b[33mcanton\x1b[m {} already exists", c.name);
            } else {
                println!("\x1b[32mupdating canton\x1b[m {}", c.name);
                old.poly = c.poly;
            }
            continue;
        }

        c.index = get_index(&mut region.canton_index, &c.id);
        c.nation = region.nation.clone();
        c.region = region.id.clone();
        region.cantons.insert(c.id.clone(), c);
    }

    sdb.save(DB_PATH)?;

    Ok(())
}
