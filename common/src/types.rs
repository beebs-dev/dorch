use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
pub struct GameInfo {
    #[serde(default, skip_serializing_if = "Uuid::is_nil")]
    pub game_id: Uuid,
    pub private: bool,
    pub name: String,
    pub max_players: i32,
    pub player_count: i32,
    pub skill: i32,
    pub current_map: String,
    pub monster_kill_count: i32,
    pub monster_count: i32,
    pub motd: Option<String>,
    pub sv_cheats: bool,
    pub sv_allowchat: bool,
    pub sv_allowvoicechat: bool,
    pub sv_fastmonsters: bool,
    pub sv_monsters: bool,
    pub sv_nomonsters: bool,
    pub sv_itemsrespawn: bool,
    pub sv_itemrespawntime: Option<i32>,
    pub sv_coop_damagefactor: Option<f32>,
    pub sv_nojump: bool,
    pub sv_nocrouch: bool,
    pub sv_nofreelook: bool,
    pub sv_respawnonexit: bool,
    pub sv_timelimit: Option<i32>,
    pub sv_fraglimit: Option<i32>,
    pub sv_scorelimit: Option<i32>,
    pub sv_duellimit: Option<i32>,
    pub sv_roundlimit: Option<i32>,
    pub sv_allowrun: bool,
    pub sv_allowfreelook: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GameInfoUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_players: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_map: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monster_kill_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monster_count: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub motd: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_cheats: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_allowchat: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_allowvoicechat: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_fastmonsters: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_monsters: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_nomonsters: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_itemsrespawn: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_itemrespawntime: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_coop_damagefactor: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_nojump: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_nocrouch: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_nofreelook: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_respawnonexit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_timelimit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_fraglimit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_scorelimit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_duellimit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_roundlimit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_allowrun: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sv_allowfreelook: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Party {
    pub id: Uuid,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub leader_id: Uuid,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub members: Option<Vec<Uuid>>,
}

pub mod wad {
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct WadMergedOut {
        pub meta: WadMeta,
        pub maps: Vec<MapStat>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct WadMeta {
        #[serde(default)]
        pub id: Uuid,

        pub sha1: String,
        #[serde(default)]
        pub sha256: Option<String>,

        #[serde(default)]
        pub title: Option<String>,
        #[serde(default)]
        pub authors: Option<Vec<String>>,
        #[serde(default)]
        pub descriptions: Option<Vec<String>>,

        /// Combined extracted PK3 text files + idgames textfile, if any.
        #[serde(default)]
        pub text_files: Option<Vec<TextFile>>,

        pub file: FileMeta,
        pub content: ContentMeta,
        pub sources: SourcesMeta,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TextFile {
        /// "pk3" or "idgames"
        pub source: String,
        #[serde(default)]
        pub name: Option<String>,
        pub contents: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FileMeta {
        #[serde(rename = "type")]
        pub file_type: String,

        #[serde(default)]
        pub size: Option<i64>,

        #[serde(default)]
        pub url: Option<String>,

        #[serde(default)]
        pub corrupt: Option<bool>,

        #[serde(default, rename = "corruptMessage")]
        pub corrupt_message: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ContentMeta {
        /// Prefer extracted maps if present; else WAD Archive maps.
        #[serde(default)]
        pub maps: Option<Vec<String>>,

        /// From wads.json; dynamic set of counters.
        #[serde(default)]
        pub counts: Option<BTreeMap<String, i64>>,

        #[serde(default)]
        pub engines_guess: Option<Vec<String>>,

        #[serde(default)]
        pub iwads_guess: Option<Vec<String>>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SourcesMeta {
        pub wad_archive: WadArchiveSource,

        #[serde(default)]
        pub idgames: Option<IdgamesSource>,

        pub extracted: ExtractedSource,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct WadArchiveSource {
        #[serde(default)]
        pub updated: Option<String>,

        /// hashes object from wads.json (md5/sha1/sha256 strings typically)
        #[serde(default)]
        pub hashes: Option<Hashes>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Hashes {
        #[serde(default)]
        pub md5: Option<String>,
        #[serde(default)]
        pub sha1: Option<String>,
        #[serde(default)]
        pub sha256: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct IdgamesSource {
        #[serde(default)]
        pub id: Option<i64>,
        #[serde(default)]
        pub url: Option<String>,
        #[serde(default)]
        pub dir: Option<String>,
        #[serde(default)]
        pub filename: Option<String>,
        #[serde(default)]
        pub date: Option<String>,
        #[serde(default)]
        pub title: Option<String>,
        #[serde(default)]
        pub author: Option<String>,
        #[serde(default)]
        pub credits: Option<String>,
        #[serde(default)]
        pub textfile: Option<String>,
        #[serde(default)]
        pub rating: Option<f64>,
        #[serde(default)]
        pub votes: Option<i64>,
    }

    //
    // ExtractedSource: returned by extract_metadata_from_file(...)
    // - WAD: extract_from_wad_bytes
    // - ZIP/PK3: extract_from_zip_bytes (but compacted: text_files only contain {path,size})
    // - Unknown: several possible shapes
    //

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "format", rename_all = "lowercase")]
    pub enum ExtractedSource {
        /// extract_from_wad_bytes
        Wad {
            #[serde(default)]
            lump_count: Option<i64>,
            #[serde(default)]
            maps: Option<Vec<String>>,
            #[serde(default)]
            text_lumps: Option<Vec<String>>,
            #[serde(default)]
            names: Option<Vec<String>>,
            #[serde(default)]
            authors: Option<Vec<String>>,
            #[serde(default)]
            descriptions: Option<Vec<String>>,
            // note: text_payloads are commented out in script, so omitted
        },

        /// extract_from_zip_bytes (but compacted in sources.extracted: text_files are {path,size} only)
        Zip {
            #[serde(default)]
            embedded_wads: Option<Vec<EmbeddedWadMeta>>,
            #[serde(default)]
            text_files: Option<Vec<ZipTextFileCompact>>,
            #[serde(default)]
            names: Option<Vec<String>>,
            #[serde(default)]
            authors: Option<Vec<String>>,
            #[serde(default)]
            descriptions: Option<Vec<String>>,

            /// present on zip errors
            #[serde(default)]
            error: Option<String>,
        },

        /// Script may emit unknown for many reasons (not wad header, bad zip, s3 resolution failure, etc).
        Unknown {
            #[serde(default)]
            error: Option<String>,
            #[serde(default)]
            note: Option<String>,
            #[serde(default)]
            size: Option<i64>,

            // only present for "Could not resolve S3 object URL"
            #[serde(default)]
            tried_prefixes: Option<Vec<String>>,
            #[serde(default)]
            expected_ext: Option<String>,
        },
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct EmbeddedWadMeta {
        /// added in extract_from_zip_bytes: wad_meta["path"] = fname
        #[serde(default)]
        pub path: Option<String>,

        /// inner wad format (should be "wad" for these entries, but keep flexible)
        #[serde(default)]
        pub format: Option<String>,

        #[serde(default)]
        pub lump_count: Option<i64>,
        #[serde(default)]
        pub maps: Option<Vec<String>>,
        #[serde(default)]
        pub text_lumps: Option<Vec<String>>,
        #[serde(default)]
        pub names: Option<Vec<String>>,
        #[serde(default)]
        pub authors: Option<Vec<String>>,
        #[serde(default)]
        pub descriptions: Option<Vec<String>>,

        #[serde(default)]
        pub error: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ZipTextFileCompact {
        #[serde(default)]
        pub path: Option<String>,
        #[serde(default)]
        pub size: Option<i64>,
    }

    //
    // Per-map stats: output of map_summary_from_wad_bytes()
    //

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MapStat {
        pub map: String,
        pub format: String, // "doom" | "hexen" | "unknown"
        pub stats: MapStats,
        pub monsters: MonstersSummary,
        pub items: ItemsSummary,
        pub mechanics: Mechanics,
        pub difficulty: Difficulty,
        pub compatibility: String, // "vanilla_or_boom" | "hexen" | "unknown"
        pub metadata: MapMetadata,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MapStats {
        pub things: i64,
        pub linedefs: i64,
        pub sidedefs: i64,
        pub vertices: i64,
        pub sectors: i64,
        pub segs: i64,
        pub ssectors: i64,
        pub nodes: i64,

        #[serde(default)]
        pub textures: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MonstersSummary {
        pub total: i64,

        /// Monster id -> count (sorted in the producer, but JSON object order isn’t guaranteed)
        #[serde(default)]
        pub by_type: BTreeMap<String, i64>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ItemsSummary {
        pub total: i64,
        #[serde(default)]
        pub by_type: BTreeMap<String, i64>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Mechanics {
        pub teleports: bool,

        /// e.g. ["blue", "red", "yellow_skull"]
        #[serde(default)]
        pub keys: Vec<String>,

        pub secret_exit: bool,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Difficulty {
        pub uv_monsters: i64,
        pub hmp_monsters: i64,
        pub htr_monsters: i64,

        pub uv_items: i64,
        pub hmp_items: i64,
        pub htr_items: i64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MapMetadata {
        /// always null in the current script
        #[serde(default)]
        pub title: Option<String>,
        /// always null in the current script
        #[serde(default)]
        pub music: Option<String>,
        /// always "marker" in the current script
        pub source: String,
    }
}
