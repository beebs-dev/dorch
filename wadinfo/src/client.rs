use dorch_common::{
    Pagination,
    types::wad::{InsertWadMeta, MapStat, ReadWadMeta},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WadImage {
    /// Optional on PUT; always present on GET.
    #[serde(default)]
    pub id: Option<Uuid>,

    pub url: String,

    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WadMapImages {
    pub map: String,
    pub items: Vec<WadImage>,
}

#[derive(Deserialize)]
pub struct SearchOptions {
    #[serde(flatten)]
    pub pagination: Pagination,

    #[serde(rename = "q")]
    pub query: String,
}

#[derive(Deserialize)]
pub struct ListWadsRequest {
    #[serde(flatten)]
    pub pagination: Pagination,

    /// If true, sort descending. Otherwise, sort ascending.
    #[serde(rename = "d", default)]
    pub sort_desc: bool,
}

#[derive(Serialize)]
pub struct GetWadMapResponse {
    #[serde(flatten)]
    pub map: MapStat,
    pub wad_meta: ReadWadMeta,
}

#[derive(Deserialize)]
pub struct WadSearchRequest {
    pub query: String,

    #[serde(default)]
    pub offset: i64,

    #[serde(default)]
    pub limit: Option<i64>,
    // TODO: add filters, sorting, etc.
}

#[derive(Serialize, Deserialize)]
pub struct WadSearchResults {
    pub request_id: Uuid,
    pub query: String,
    pub items: Vec<ReadWadMeta>,
    pub full_count: i64,
    pub offset: i64,
    pub limit: i64,
    pub truncated: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ListWadsResponse {
    pub items: Vec<ReadWadMeta>,
    pub full_count: i64,
    pub offset: i64,
    pub limit: i64,
    pub truncated: bool,
}
