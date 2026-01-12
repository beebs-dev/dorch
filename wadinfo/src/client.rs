use dorch_common::types::wad::WadMeta;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Pagination {
    #[serde(rename = "o", default)]
    pub offset: i64,

    #[serde(rename = "l", default)]
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct SearchOptions {
    #[serde(flatten)]
    pub pagination: Pagination,

    #[serde(rename = "q")]
    pub query: String,
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

#[derive(Deserialize)]
pub struct WadSearchResults {
    pub query: String,
    pub items: Vec<WadMeta>,
    pub full_count: i64,
    pub offset: i64,
    pub limit: i64,
    pub truncated: bool,
}
