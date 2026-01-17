use dorch_common::{
    Pagination,
    types::wad::{MapStat, ReadWadMeta, TextFile},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MapReference {
    pub wad_id: Uuid,
    pub map: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolveMapThumbnailsRequest {
    pub items: Vec<MapReference>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolveWadDownloadsRequest {
    pub wad_ids: Vec<Uuid>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolveWadDownloadsResponse {
    pub items: Vec<ResolvedWadDownload>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolvedWadDownload {
    pub wad_id: Uuid,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MapThumbnail {
    pub wad_id: Uuid,
    pub map: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolveMapThumbnailsResponse {
    pub items: Vec<MapThumbnail>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WadImage {
    /// Optional on PUT; always present on GET.
    #[serde(default)]
    pub id: Option<Uuid>,

    pub url: String,

    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

/// Read-only view of a map, including image metadata.
///
/// The underlying per-map schema is stored as JSONB for speed. We add `images` at query time
/// (Postgres JSONB composition) so read endpoints can return a richer shape without changing the
/// insert/upsert payload types.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReadMapStat {
    #[serde(flatten)]
    pub map: MapStat,

    #[serde(default)]
    pub images: Vec<WadImage>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReadWadMetaWithTextFiles {
    #[serde(flatten)]
    pub meta: ReadWadMeta,

    #[serde(default)]
    pub text_files: Option<Vec<TextFile>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReadWad {
    pub meta: ReadWadMetaWithTextFiles,
    pub maps: Vec<ReadMapStat>,
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
pub struct ResolveWadURLsRequest {
    pub wad_ids: Vec<Uuid>,
}

#[derive(Serialize, Deserialize)]
pub struct ResolvedWadURL {
    pub wad_id: Uuid,
    pub url: String,
}

#[derive(Serialize, Deserialize)]
pub struct ResolveWadURLsResponse {
    pub items: Vec<ResolvedWadURL>,
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
    pub map: ReadMapStat,
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
