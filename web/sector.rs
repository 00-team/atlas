use crate::db::BoundingBox;
use geo::Point;
use std::collections::HashMap;

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SimpleCanton {
    pub name: String,
    pub id: String,
    pub region: String,
    pub nation: String,
    pub bounding_box: BoundingBox,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center: Option<Point>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SimpleRegion {
    pub name: String,
    pub id: String,
    pub cantons: HashMap<String, SimpleCanton>,
    pub nation: String,
    pub bounding_box: BoundingBox,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center: Option<Point>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SimpleNation {
    pub name: String,
    pub id: String,
    pub regions: HashMap<String, SimpleRegion>,
    pub bounding_box: BoundingBox,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center: Option<Point>,
}
