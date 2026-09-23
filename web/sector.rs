use std::collections::HashMap;

use crate::db::BoundingBox;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SimpleCanton {
    pub name: String,
    pub id: String,
    pub region: String,
    pub nation: String,
    pub bounding_box: BoundingBox,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SimpleRegion {
    pub name: String,
    pub id: String,
    pub cantons: HashMap<String, SimpleCanton>,
    pub nation: String,
    pub bounding_box: BoundingBox,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SimpleNation {
    pub name: String,
    pub id: String,
    pub regions: HashMap<String, SimpleRegion>,
    pub bounding_box: BoundingBox,
}
