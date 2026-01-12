use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

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

#[derive(Serialize, Deserialize)]
pub struct InsertWadRequest {
    #[serde(flatten)]
    pub meta: WadMeta,

    pub maps: Vec<WadMap>,
}

#[derive(Serialize, Deserialize)]
pub struct ListWadsResponse {
    pub offset: i64,
    pub limit: i64,
    pub full_count: i64,
    pub items: Vec<WadMeta>,
    pub truncated: bool,
}

#[derive(Serialize, Deserialize)]
pub struct WadSearchResult {
    #[serde(flatten)]
    pub meta: WadMeta,
    pub rank: f32,
}

#[derive(Serialize, Deserialize)]
pub struct SearchWadsResponse {
    pub offset: i64,
    pub limit: i64,
    pub items: Vec<WadSearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WadMeta {
    pub wad_id: Uuid,
    pub sha1: String,
    pub filename: Option<String>,
    /// 'IWAD' or 'PWAD'
    pub wad_type: Option<String>,
    pub byte_size: Option<i64>,
    pub uploaded_at: i64,
    pub map_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wad {
    #[serde(flatten)]
    pub meta: WadMeta,
    pub map_names: Vec<String>,
}

impl TryFrom<tokio_postgres::Row> for WadMeta {
    type Error = anyhow::Error;

    fn try_from(row: tokio_postgres::Row) -> std::result::Result<Self, Self::Error> {
        Ok(WadMeta {
            wad_id: row.try_get("wad_id")?,
            sha1: row.try_get("sha1")?,
            filename: row.try_get("filename")?,
            wad_type: row.try_get("wad_type")?,
            byte_size: row.try_get("byte_size")?,
            uploaded_at: row.try_get("uploaded_at")?,
            map_count: row.try_get("map_count")?,
        })
    }
}

/// Row in `wad_maps`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WadMap {
    pub wad_id: Uuid,
    pub map_name: String,      // MAP01 / E1M1
    pub format: String,        // doom / hexen / unknown
    pub compatibility: String, // vanilla_or_boom / hexen / unknown

    // core stats
    pub things: i32,
    pub linedefs: i32,
    pub sidedefs: i32,
    pub vertices: i32,
    pub sectors: i32,
    pub segs: i32,
    pub ssectors: i32,
    pub nodes: i32,

    // computed flags
    pub teleports: bool,
    pub secret_exit: bool,

    // monsters summary
    pub monster_total: i32,
    pub uv_monsters: i32,
    pub hmp_monsters: i32,
    pub htr_monsters: i32,

    // per-monster breakdown
    pub zombieman_count: i32,
    pub shotgun_guy_count: i32,
    pub chaingun_guy_count: i32,
    pub imp_count: i32,
    pub demon_count: i32,
    pub spectre_count: i32,
    pub cacodemon_count: i32,
    pub lost_soul_count: i32,
    pub pain_elemental_count: i32,
    pub revenant_count: i32,
    pub mancubus_count: i32,
    pub arachnotron_count: i32,
    pub hell_knight_count: i32,
    pub baron_count: i32,
    pub archvile_count: i32,
    pub cyberdemon_count: i32,
    pub spider_mastermind_count: i32,

    /// `text[]` in Postgres
    pub keys: Vec<String>,

    /// `jsonb` in Postgres
    pub doc: Value,
}

impl TryFrom<tokio_postgres::Row> for WadMap {
    type Error = anyhow::Error;

    fn try_from(row: tokio_postgres::Row) -> std::result::Result<Self, Self::Error> {
        Ok(WadMap {
            wad_id: row.try_get("wad_id")?,
            map_name: row.try_get("map_name")?,
            format: row.try_get("format")?,
            compatibility: row.try_get("compatibility")?,
            things: row.try_get("things")?,
            linedefs: row.try_get("linedefs")?,
            sidedefs: row.try_get("sidedefs")?,
            vertices: row.try_get("vertices")?,
            sectors: row.try_get("sectors")?,
            segs: row.try_get("segs")?,
            ssectors: row.try_get("ssectors")?,
            nodes: row.try_get("nodes")?,
            teleports: row.try_get("teleports")?,
            secret_exit: row.try_get("secret_exit")?,
            monster_total: row.try_get("monster_total")?,
            uv_monsters: row.try_get("uv_monsters")?,
            hmp_monsters: row.try_get("hmp_monsters")?,
            htr_monsters: row.try_get("htr_monsters")?,
            zombieman_count: row.try_get("zombieman_count")?,
            shotgun_guy_count: row.try_get("shotgun_guy_count")?,
            chaingun_guy_count: row.try_get("chaingun_guy_count")?,
            imp_count: row.try_get("imp_count")?,
            demon_count: row.try_get("demon_count")?,
            spectre_count: row.try_get("spectre_count")?,
            cacodemon_count: row.try_get("cacodemon_count")?,
            lost_soul_count: row.try_get("lost_soul_count")?,
            pain_elemental_count: row.try_get("pain_elemental_count")?,
            revenant_count: row.try_get("revenant_count")?,
            mancubus_count: row.try_get("mancubus_count")?,
            arachnotron_count: row.try_get("arachnotron_count")?,
            hell_knight_count: row.try_get("hell_knight_count")?,
            baron_count: row.try_get("baron_count")?,
            archvile_count: row.try_get("archvile_count")?,
            cyberdemon_count: row.try_get("cyberdemon_count")?,
            spider_mastermind_count: row.try_get("spider_mastermind_count")?,
            keys: row.try_get("keys")?,
            doc: row.try_get("doc")?,
        })
    }
}
