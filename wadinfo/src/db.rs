use crate::client::{
    GetWadMapResponse, ListWadsResponse, MapReference, MapThumbnail, ReadMapStat, ReadWad,
    ReadWadMetaWithTextFiles, ResolvedWadURL, WadImage, WadSearchResults,
};
use anyhow::{Context, Result};
use dorch_common::{
    postgres::strip_sql_comments,
    types::wad::{InsertWad, ReadWadMeta, TextFile},
};
use owo_colors::OwoColorize;
use serde_json::Value;
use std::collections::BTreeMap;
use tokio_postgres::types::Json;
use uuid::Uuid;

fn escape_nul_in_str(s: &str) -> Option<String> {
    if s.contains('\0') {
        // Postgres text/jsonb cannot contain a literal NUL. Preserve information by turning it
        // into the literal 6-character sequence "\\u0000".
        Some(s.replace('\0', "\\u0000"))
    } else {
        None
    }
}

fn escape_nul_in_vec(v: &[String]) -> Option<Vec<String>> {
    if v.iter().any(|s| s.contains('\0')) {
        Some(
            v.iter()
                .map(|s| escape_nul_in_str(s).unwrap_or_else(|| s.clone()))
                .collect(),
        )
    } else {
        None
    }
}

fn escape_nul_in_json(value: &mut Value) -> usize {
    match value {
        Value::String(s) => {
            if s.contains('\0') {
                *s = s.replace('\0', "\\u0000");
                1
            } else {
                0
            }
        }
        Value::Array(items) => items.iter_mut().map(escape_nul_in_json).sum(),
        Value::Object(obj) => obj.values_mut().map(escape_nul_in_json).sum(),
        _ => 0,
    }
}

mod sql {
    pub const TABLES: &str = concat!(include_str!("sql/tables.sql"));
    pub const INSERT_WAD: &str = include_str!("sql/insert_wad.sql");
    // NOTE: the legacy trigram/similarity search SQL is deprecated.
    pub const SEARCH_WADS_ILIKE: &str = include_str!("sql/search_wads_ilike.sql");
    pub const SEARCH_WADS_BY_SHA1: &str = include_str!("sql/search_wads_by_sha1.sql");

    pub const LIST_WADS_ASC: &str = include_str!("sql/list_wads_asc.sql");
    pub const LIST_WADS_DESC: &str = include_str!("sql/list_wads_desc.sql");

    pub const FEATURED_WADS: &str = include_str!("sql/featured_wads.sql");

    pub const GET_WAD: &str = include_str!("sql/get_wad.sql");
    pub const GET_WAD_PUBLIC: &str = include_str!("sql/get_wad_public.sql");
    pub const RESOLVE_WAD_URLS: &str = include_str!("sql/resolve_wad_urls.sql");
    pub const RESOLVE_MAP_THUMBNAILS: &str = include_str!("sql/resolve_map_thumbnails.sql");
    pub const GET_WAD_MAPS: &str = include_str!("sql/get_wad_maps.sql");
    pub const GET_WAD_MAP: &str = include_str!("sql/get_wad_map.sql");

    pub const DELETE_WAD_CHILDREN: &str = include_str!("sql/delete_wad_children.sql");
    pub const INSERT_WAD_AUTHOR: &str = include_str!("sql/insert_wad_author.sql");
    pub const INSERT_WAD_FILENAME: &str = include_str!("sql/insert_wad_filename.sql");
    pub const INSERT_WAD_DESCRIPTION: &str = include_str!("sql/insert_wad_description.sql");
    pub const INSERT_WAD_MAP_LIST: &str = include_str!("sql/insert_wad_map_list.sql");
    pub const INSERT_WAD_TEXT_FILE: &str = include_str!("sql/insert_wad_text_file.sql");

    pub const UPSERT_WAD_COUNTS: &str = include_str!("sql/upsert_wad_counts.sql");

    pub const UPSERT_WAD_SOURCE_WAD_ARCHIVE: &str =
        include_str!("sql/upsert_wad_source_wad_archive.sql");
    pub const UPSERT_WAD_SOURCE_EXTRACTED: &str =
        include_str!("sql/upsert_wad_source_extracted.sql");
    pub const UPSERT_WAD_SOURCE_IDGAMES: &str = include_str!("sql/upsert_wad_source_idgames.sql");
    pub const DELETE_WAD_SOURCE_IDGAMES: &str = include_str!("sql/delete_wad_source_idgames.sql");

    pub const INSERT_WAD_MAP: &str = include_str!("sql/insert_wad_map.sql");
    pub const INSERT_WAD_MAP_TEXTURE: &str = include_str!("sql/insert_wad_map_texture.sql");
    pub const INSERT_WAD_MAP_MONSTER: &str = include_str!("sql/insert_wad_map_monster.sql");
    pub const INSERT_WAD_MAP_ITEM: &str = include_str!("sql/insert_wad_map_item.sql");

    pub const LIST_WAD_MAP_IMAGES: &str = include_str!("sql/list_wad_map_images.sql");
    pub const LIST_WAD_TEXT_FILES: &str = include_str!("sql/list_wad_text_files.sql");
    pub const DELETE_WAD_MAP_IMAGES: &str = include_str!("sql/delete_wad_map_images.sql");
    pub const INSERT_WAD_MAP_IMAGE: &str = include_str!("sql/insert_wad_map_image.sql");
}

#[derive(Clone)]
pub struct Database {
    pool: deadpool_postgres::Pool,
}

impl Database {
    pub async fn new(pool: deadpool_postgres::Pool) -> Result<Self> {
        let mut conn = pool.get().await.context("failed to get connection")?;
        create_tables(&mut conn).await;
        println!("{}", "Database tables ensured.".green());
        _ = conn
            .prepare(sql::INSERT_WAD)
            .await
            .context("failed to prepare INSERT_WAD")?;
        _ = conn
            .prepare(sql::SEARCH_WADS_ILIKE)
            .await
            .context("failed to prepare SEARCH_WADS_ILIKE")?;
        _ = conn
            .prepare(sql::SEARCH_WADS_BY_SHA1)
            .await
            .context("failed to prepare SEARCH_WADS_BY_SHA1")?;

        _ = conn
            .prepare(sql::LIST_WADS_ASC)
            .await
            .context("failed to prepare LIST_WADS_ASC")?;
        _ = conn
            .prepare(sql::LIST_WADS_DESC)
            .await
            .context("failed to prepare LIST_WADS_DESC")?;

        _ = conn
            .prepare(sql::FEATURED_WADS)
            .await
            .context("failed to prepare FEATURED_WADS")?;

        _ = conn
            .prepare(sql::RESOLVE_WAD_URLS)
            .await
            .context("failed to prepare RESOLVE_WAD_URLS")?;

        _ = conn
            .prepare(sql::RESOLVE_MAP_THUMBNAILS)
            .await
            .context("failed to prepare RESOLVE_MAP_THUMBNAILS")?;

        _ = conn
            .prepare(sql::GET_WAD)
            .await
            .context("failed to prepare GET_WAD")?;
        _ = conn
            .prepare(sql::GET_WAD_PUBLIC)
            .await
            .context("failed to prepare GET_WAD_PUBLIC")?;
        _ = conn
            .prepare(sql::GET_WAD_MAPS)
            .await
            .context("failed to prepare GET_WAD_MAPS")?;
        _ = conn
            .prepare(sql::GET_WAD_MAP)
            .await
            .context("failed to prepare GET_WAD_MAP")?;

        _ = conn
            .prepare(sql::DELETE_WAD_CHILDREN)
            .await
            .context("failed to prepare DELETE_WAD_CHILDREN")?;
        _ = conn
            .prepare(sql::INSERT_WAD_AUTHOR)
            .await
            .context("failed to prepare INSERT_WAD_AUTHOR")?;
        _ = conn
            .prepare(sql::INSERT_WAD_DESCRIPTION)
            .await
            .context("failed to prepare INSERT_WAD_DESCRIPTION")?;
        _ = conn
            .prepare(sql::INSERT_WAD_MAP_LIST)
            .await
            .context("failed to prepare INSERT_WAD_MAP_LIST")?;
        _ = conn
            .prepare(sql::INSERT_WAD_TEXT_FILE)
            .await
            .context("failed to prepare INSERT_WAD_TEXT_FILE")?;
        _ = conn
            .prepare(sql::UPSERT_WAD_COUNTS)
            .await
            .context("failed to prepare UPSERT_WAD_COUNTS")?;
        _ = conn
            .prepare(sql::UPSERT_WAD_SOURCE_WAD_ARCHIVE)
            .await
            .context("failed to prepare UPSERT_WAD_SOURCE_WAD_ARCHIVE")?;
        _ = conn
            .prepare(sql::UPSERT_WAD_SOURCE_EXTRACTED)
            .await
            .context("failed to prepare UPSERT_WAD_SOURCE_EXTRACTED")?;
        _ = conn
            .prepare(sql::UPSERT_WAD_SOURCE_IDGAMES)
            .await
            .context("failed to prepare UPSERT_WAD_SOURCE_IDGAMES")?;
        _ = conn
            .prepare(sql::DELETE_WAD_SOURCE_IDGAMES)
            .await
            .context("failed to prepare DELETE_WAD_SOURCE_IDGAMES")?;
        _ = conn
            .prepare(sql::INSERT_WAD_MAP)
            .await
            .context("failed to prepare INSERT_WAD_MAP")?;
        _ = conn
            .prepare(sql::INSERT_WAD_MAP_TEXTURE)
            .await
            .context("failed to prepare INSERT_WAD_MAP_TEXTURE")?;
        _ = conn
            .prepare(sql::INSERT_WAD_MAP_MONSTER)
            .await
            .context("failed to prepare INSERT_WAD_MAP_MONSTER")?;
        _ = conn
            .prepare(sql::INSERT_WAD_MAP_ITEM)
            .await
            .context("failed to prepare INSERT_WAD_MAP_ITEM")?;

        _ = conn
            .prepare(sql::LIST_WAD_MAP_IMAGES)
            .await
            .context("failed to prepare LIST_WAD_MAP_IMAGES")?;

        _ = conn
            .prepare(sql::LIST_WAD_TEXT_FILES)
            .await
            .context("failed to prepare LIST_WAD_TEXT_FILES")?;
        _ = conn
            .prepare(sql::DELETE_WAD_MAP_IMAGES)
            .await
            .context("failed to prepare DELETE_WAD_MAP_IMAGES")?;
        _ = conn
            .prepare(sql::INSERT_WAD_MAP_IMAGE)
            .await
            .context("failed to prepare INSERT_WAD_MAP_IMAGE")?;

        _ = conn;
        Ok(Self { pool })
    }

    pub async fn get_wad(&self, wad_id: Uuid) -> Result<Option<ReadWad>> {
        let conn = self.pool.get().await.context("failed to get connection")?;

        let get_wad = conn
            .prepare_cached(sql::GET_WAD)
            .await
            .context("failed to prepare GET_WAD")?;
        let row = conn
            .query_opt(&get_wad, &[&wad_id])
            .await
            .context("failed to execute GET_WAD")?;

        let Some(row) = row else {
            return Ok(None);
        };

        let row_wad_id: Uuid = row.try_get("wad_id")?;
        let meta_json: serde_json::Value = row.try_get("meta_json")?;
        let mut meta: ReadWadMeta =
            serde_json::from_value(meta_json).context("deserialize ReadWadMeta")?;
        if meta.id.is_nil() {
            meta.id = row_wad_id;
        }

        // Only GET /wad/{id} returns `text_files`.
        let list_text_files = conn
            .prepare_cached(sql::LIST_WAD_TEXT_FILES)
            .await
            .context("failed to prepare LIST_WAD_TEXT_FILES")?;
        let text_file_rows = conn
            .query(&list_text_files, &[&wad_id])
            .await
            .context("failed to execute LIST_WAD_TEXT_FILES")?;
        let mut text_files: Vec<TextFile> = Vec::with_capacity(text_file_rows.len());
        for row in text_file_rows {
            let source: String = row.try_get("source")?;
            let name: Option<String> = row.try_get("name")?;
            let contents: String = row.try_get("contents")?;
            text_files.push(TextFile {
                source,
                name,
                contents,
            });
        }

        let meta = ReadWadMetaWithTextFiles {
            meta,
            text_files: if text_files.is_empty() {
                None
            } else {
                Some(text_files)
            },
        };

        let get_maps = conn
            .prepare_cached(sql::GET_WAD_MAPS)
            .await
            .context("failed to prepare GET_WAD_MAPS")?;
        let map_rows = conn
            .query(&get_maps, &[&wad_id])
            .await
            .context("failed to execute GET_WAD_MAPS")?;

        let mut maps = Vec::with_capacity(map_rows.len());
        for row in map_rows {
            let map_json: serde_json::Value = row.try_get("map_json")?;
            let map: ReadMapStat =
                serde_json::from_value(map_json).context("deserialize ReadMapStat")?;
            maps.push(map);
        }

        Ok(Some(ReadWad { meta, maps }))
    }

    pub async fn get_wad_map(
        &self,
        wad_id: Uuid,
        map_name: &str,
    ) -> Result<Option<GetWadMapResponse>> {
        let conn = self.pool.get().await.context("failed to get connection")?;

        let stmt = conn
            .prepare_cached(sql::GET_WAD_MAP)
            .await
            .context("failed to prepare GET_WAD_MAP")?;
        let row = conn
            .query_opt(&stmt, &[&wad_id, &map_name])
            .await
            .context("failed to execute GET_WAD_MAP")?;

        let Some(row) = row else {
            return Ok(None);
        };

        let row_wad_id: Uuid = row.try_get("wad_id")?;
        let meta_json: serde_json::Value = row.try_get("meta_json")?;
        let mut wad_meta: ReadWadMeta =
            serde_json::from_value(meta_json).context("deserialize ReadWadMeta")?;
        if wad_meta.id.is_nil() {
            wad_meta.id = row_wad_id;
        }

        let map_json: serde_json::Value = row.try_get("map_json")?;
        let map: ReadMapStat =
            serde_json::from_value(map_json).context("deserialize ReadMapStat")?;

        Ok(Some(GetWadMapResponse { map, wad_meta }))
    }

    pub async fn list_wad_map_images(&self, wad_id: Uuid, map_name: &str) -> Result<Vec<WadImage>> {
        let conn = self.pool.get().await.context("failed to get connection")?;
        let stmt = conn
            .prepare_cached(sql::LIST_WAD_MAP_IMAGES)
            .await
            .context("failed to prepare LIST_WAD_MAP_IMAGES")?;
        let rows = conn
            .query(&stmt, &[&wad_id, &map_name])
            .await
            .context("failed to execute LIST_WAD_MAP_IMAGES")?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let id: Uuid = row.try_get("id")?;
            let url: String = row.try_get("url")?;
            let kind: Option<String> = row.try_get("type")?;
            out.push(WadImage {
                id: Some(id),
                url,
                kind,
            });
        }
        Ok(out)
    }

    pub async fn resolve_map_thumbnails(
        &self,
        items: &[MapReference],
    ) -> Result<Vec<MapThumbnail>> {
        if items.is_empty() {
            return Ok(Vec::new());
        }

        let mut wad_ids = Vec::with_capacity(items.len());
        let mut map_names = Vec::with_capacity(items.len());
        for item in items {
            wad_ids.push(item.wad_id);
            map_names.push(item.map.clone());
        }
        let map_names = escape_nul_in_vec(&map_names).unwrap_or(map_names);

        let mut conn = self.pool.get().await.context("failed to get connection")?;
        let tx = conn
            .transaction()
            .await
            .context("failed to begin transaction")?;
        let stmt = tx
            .prepare_cached(sql::RESOLVE_MAP_THUMBNAILS)
            .await
            .context("failed to prepare RESOLVE_MAP_THUMBNAILS")?;

        let rows = tx
            .query(&stmt, &[&wad_ids, &map_names])
            .await
            .context("failed to execute RESOLVE_MAP_THUMBNAILS")?;

        let out = rows
            .into_iter()
            .map(|row| {
                Ok(MapThumbnail {
                    wad_id: row.try_get("wad_id")?,
                    map: row.try_get("map_name")?,
                    url: row.try_get("url")?,
                })
            })
            .collect::<Result<Vec<MapThumbnail>>>()?;

        Ok(out)
    }

    pub async fn replace_wad_map_images(
        &self,
        wad_id: Uuid,
        map_name: &str,
        images: &[WadImage],
    ) -> Result<()> {
        let mut conn = self.pool.get().await.context("failed to get connection")?;
        let tx = conn
            .transaction()
            .await
            .context("begin replace_wad_map_images tx")?;

        let delete_stmt = tx
            .prepare_cached(sql::DELETE_WAD_MAP_IMAGES)
            .await
            .context("failed to prepare DELETE_WAD_MAP_IMAGES")?;
        tx.execute(&delete_stmt, &[&wad_id, &map_name])
            .await
            .context("failed to execute DELETE_WAD_MAP_IMAGES")?;

        let insert_stmt = tx
            .prepare_cached(sql::INSERT_WAD_MAP_IMAGE)
            .await
            .context("failed to prepare INSERT_WAD_MAP_IMAGE")?;
        for image in images {
            tx.execute(&insert_stmt, &[&wad_id, &map_name, &image.url, &image.kind])
                .await
                .with_context(|| format!("insert wad_map_image {wad_id} {map_name}"))?;
        }

        tx.commit()
            .await
            .context("commit replace_wad_map_images tx")?;
        Ok(())
    }

    pub async fn search_wads(
        &self,
        request_id: Uuid,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<WadSearchResults> {
        fn is_sha1_hex(s: &str) -> bool {
            s.len() == 40 && s.as_bytes().iter().all(|b| b.is_ascii_hexdigit())
        }

        let mut conn = self.pool.get().await.context("failed to get connection")?;
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(WadSearchResults {
                request_id,
                query: trimmed.to_string(),
                items: Vec::new(),
                full_count: 0,
                offset,
                limit,
                truncated: false,
            });
        }

        // 1) If exactly 40 chars, attempt SHA1 parse and do a fast sha1 column lookup.
        if trimmed.len() == 40 && is_sha1_hex(trimmed) {
            let sha1 = trimmed.to_ascii_lowercase();
            let stmt = conn
                .prepare_cached(sql::SEARCH_WADS_BY_SHA1)
                .await
                .context("failed to prepare SEARCH_WADS_BY_SHA1")?;
            let row = conn
                .query_opt(&stmt, &[&sha1])
                .await
                .context("failed to execute SEARCH_WADS_BY_SHA1")?;

            let (full_count, items) = if let Some(row) = row {
                if offset > 0 {
                    (1, Vec::new())
                } else {
                    let row_wad_id: Uuid = row.try_get("wad_id")?;
                    let meta_json: serde_json::Value = row.try_get("meta_json")?;
                    let mut meta = serde_json::from_value::<ReadWadMeta>(meta_json)
                        .context("deserialize ReadWadMeta from meta_json")?;
                    if meta.id.is_nil() {
                        meta.id = row_wad_id;
                    }
                    (1, vec![meta])
                }
            } else {
                (0, Vec::new())
            };

            return Ok(WadSearchResults {
                request_id,
                query: trimmed.to_string(),
                items,
                full_count,
                offset,
                limit,
                truncated: false,
            });
        }

        // 2) Attempt UUID parse. If it works, return at most one result.
        if let Ok(wad_id) = Uuid::parse_str(trimmed) {
            let stmt = conn
                .prepare_cached(sql::GET_WAD_PUBLIC)
                .await
                .context("failed to prepare GET_WAD_PUBLIC")?;
            let row = conn
                .query_opt(&stmt, &[&wad_id])
                .await
                .context("failed to execute GET_WAD_PUBLIC")?;

            let (full_count, items) = if let Some(row) = row {
                if offset > 0 {
                    (1, Vec::new())
                } else {
                    let row_wad_id: Uuid = row.try_get("wad_id")?;
                    let meta_json: serde_json::Value = row.try_get("meta_json")?;
                    let mut meta = serde_json::from_value::<ReadWadMeta>(meta_json)
                        .context("deserialize ReadWadMeta from meta_json")?;
                    if meta.id.is_nil() {
                        meta.id = row_wad_id;
                    }
                    (1, vec![meta])
                }
            } else {
                (0, Vec::new())
            };

            return Ok(WadSearchResults {
                request_id,
                query: trimmed.to_string(),
                items,
                full_count,
                offset,
                limit,
                truncated: false,
            });
        }

        // 3) Fallback: ILIKE across title, preferred_filename, and wad_filenames.filename.
        let tx = conn
            .transaction()
            .await
            .context("failed to begin transaction")?;
        let stmt = tx
            .prepare_cached(sql::SEARCH_WADS_ILIKE)
            .await
            .context("failed to prepare SEARCH_WADS_ILIKE")?;
        let rows = tx
            .query(&stmt, &[&trimmed, &offset, &limit])
            .await
            .context("failed to execute SEARCH_WADS_ILIKE")?;

        let full_count = rows
            .first()
            .map(|r| r.try_get::<_, i64>("full_count"))
            .transpose()
            .context("failed to get full_count from SEARCH_WADS_ILIKE")?
            .unwrap_or(0);

        let items = rows
            .into_iter()
            .map(|row| {
                let row_wad_id: Uuid = row.try_get("wad_id")?;
                let meta_json: serde_json::Value = row.try_get("meta_json")?;
                let mut meta = serde_json::from_value::<ReadWadMeta>(meta_json)
                    .context("deserialize ReadWadMeta from meta_json")?;
                if meta.id.is_nil() {
                    meta.id = row_wad_id;
                }
                Ok(meta)
            })
            .collect::<Result<Vec<ReadWadMeta>>>()?;

        tx.commit().await.context("failed to commit transaction")?;
        Ok(WadSearchResults {
            request_id,
            query: trimmed.to_string(),
            items,
            full_count,
            offset,
            limit,
            truncated: offset + limit < full_count,
        })
    }

    pub async fn resolve_wad_urls(&self, wad_ids: &[Uuid]) -> Result<Vec<ResolvedWadURL>> {
        let mut conn = self.pool.get().await.context("failed to get connection")?;
        let tx = conn
            .transaction()
            .await
            .context("failed to begin transaction")?;
        let stmt = tx
            .prepare_cached(sql::RESOLVE_WAD_URLS)
            .await
            .context("failed to prepare RESOLVE_WAD_URLS")?;
        let items = tx
            .query(&stmt, &[&wad_ids])
            .await
            .context("failed to execute RESOLVE_WAD_URLS")?
            .into_iter()
            .map(|row| {
                Ok(ResolvedWadURL {
                    wad_id: row.try_get("wad_id")?,
                    url: row.try_get("file_url")?,
                })
            })
            .collect::<Result<Vec<ResolvedWadURL>>>()?;
        Ok(items)
    }

    pub async fn list_wads(
        &self,
        offset: i64,
        limit: i64,
        sort_desc: bool,
    ) -> Result<ListWadsResponse> {
        let mut conn = self.pool.get().await.context("failed to get connection")?;
        let tx = conn
            .transaction()
            .await
            .context("failed to begin transaction")?;

        let sql = if sort_desc {
            sql::LIST_WADS_DESC
        } else {
            sql::LIST_WADS_ASC
        };
        let stmt = tx
            .prepare_cached(sql)
            .await
            .context("failed to prepare LIST_WADS")?;

        let rows = tx
            .query(&stmt, &[&offset, &limit])
            .await
            .context("failed to execute LIST_WADS")?;

        let full_count = rows
            .first()
            .map(|r| r.try_get::<_, i64>("full_count"))
            .transpose()
            .context("failed to get full_count from LIST_WADS")?
            .unwrap_or(0);

        let items = rows
            .into_iter()
            .map(|row| {
                let row_wad_id: Uuid = row.try_get("wad_id")?;
                let meta_json: serde_json::Value = row.try_get("meta_json")?;
                let mut meta = serde_json::from_value::<ReadWadMeta>(meta_json)
                    .context("deserialize ReadWadMeta from meta_json")?;
                if meta.id.is_nil() {
                    meta.id = row_wad_id;
                }
                Ok(meta)
            })
            .collect::<Result<Vec<ReadWadMeta>>>()?;

        tx.commit().await.context("failed to commit transaction")?;
        Ok(ListWadsResponse {
            items,
            full_count,
            offset,
            limit,
            truncated: offset + limit < full_count,
        })
    }

    pub async fn featured_wads(&self, limit: i64) -> Result<ListWadsResponse> {
        let mut conn = self.pool.get().await.context("failed to get connection")?;
        let tx = conn
            .transaction()
            .await
            .context("failed to begin transaction")?;

        let limit = limit.clamp(1, 100);
        let stmt = tx
            .prepare_cached(sql::FEATURED_WADS)
            .await
            .context("failed to prepare FEATURED_WADS")?;

        let rows = tx
            .query(&stmt, &[&limit])
            .await
            .context("failed to execute FEATURED_WADS")?;

        let full_count = rows
            .first()
            .map(|r| r.try_get::<_, i64>("full_count"))
            .transpose()
            .context("failed to get full_count from FEATURED_WADS")?
            .unwrap_or(0);

        let items = rows
            .into_iter()
            .map(|row| {
                let row_wad_id: Uuid = row.try_get("wad_id")?;
                let meta_json: serde_json::Value = row.try_get("meta_json")?;
                let mut meta = serde_json::from_value::<ReadWadMeta>(meta_json)
                    .context("deserialize ReadWadMeta from meta_json")?;
                if meta.id.is_nil() {
                    meta.id = row_wad_id;
                }
                Ok(meta)
            })
            .collect::<Result<Vec<ReadWadMeta>>>()?;

        tx.commit().await.context("failed to commit transaction")?;
        Ok(ListWadsResponse {
            items,
            full_count,
            offset: 0,
            limit,
            truncated: limit < full_count,
        })
    }

    pub async fn upsert_wad(&self, merged: &InsertWad) -> Result<Uuid> {
        let mut conn = self.pool.get().await.context("failed to get connection")?;
        let tx = conn
            .transaction()
            .await
            .context("failed to begin transaction")?;
        let read_meta: ReadWadMeta = merged.meta.clone().into();
        let mut read_meta_json = serde_json::to_value(&read_meta).context("serialize read_meta")?;
        let read_meta_nul_count = escape_nul_in_json(&mut read_meta_json);
        if read_meta_nul_count > 0 {
            eprintln!(
                "{}{}{}{}",
                "⚠️  Escaped embedded NULLs in read_meta • sha1=".yellow(),
                read_meta.sha1.yellow().dimmed(),
                " • strings_touched=".yellow(),
                read_meta_nul_count.yellow().dimmed(),
            );
        }
        let wa_updated_ts = parse_ts_any(&merged.meta.sources.wad_archive.updated);

        let preferred_filename = merged.meta.filename.as_deref();
        let preferred_filename_escaped = preferred_filename.and_then(escape_nul_in_str);
        let preferred_filename_param: Option<&str> =
            preferred_filename_escaped.as_deref().or(preferred_filename);

        let added_ts = parse_ts_any(&merged.meta.added);

        let (hidden, adult, can_download, locked) = match &merged.meta.flags {
            Some(f) => (
                f.hidden.unwrap_or(false),
                f.adult.unwrap_or(false),
                f.can_download.unwrap_or(true),
                f.locked.unwrap_or(false),
            ),
            None => (false, false, true, false),
        };

        let sha1 = merged.meta.sha1.as_str();
        let sha256 = merged.meta.sha256.as_deref();

        let title = merged.meta.title.as_deref();
        let title_escaped = title.and_then(escape_nul_in_str);
        let title_param: Option<&str> = title_escaped.as_deref().or(title);

        let file_type = merged.meta.file.file_type.as_str();
        let file_size = merged.meta.file.size;
        let file_url = merged.meta.file.url.as_deref();
        let file_url_escaped = file_url.and_then(escape_nul_in_str);
        let file_url_param: Option<&str> = file_url_escaped.as_deref().or(file_url);
        let corrupt = merged.meta.file.corrupt;
        let corrupt_msg = merged.meta.file.corrupt_message.as_deref();
        let corrupt_msg_escaped = corrupt_msg.and_then(escape_nul_in_str);
        let corrupt_msg_param: Option<&str> = corrupt_msg_escaped.as_deref().or(corrupt_msg);

        let engines_guess = merged.meta.content.engines_guess.as_ref();
        let engines_guess_escaped = engines_guess.and_then(|v| escape_nul_in_vec(v));
        let engines_guess_param: Option<&Vec<String>> =
            engines_guess_escaped.as_ref().or(engines_guess);

        let iwads_guess = merged.meta.content.iwads_guess.as_ref();
        let iwads_guess_escaped = iwads_guess.and_then(|v| escape_nul_in_vec(v));
        let iwads_guess_param: Option<&Vec<String>> = iwads_guess_escaped.as_ref().or(iwads_guess);

        // 1) Upsert wads row -> wad_id
        let insert_wad = tx
            .prepare_cached(sql::INSERT_WAD)
            .await
            .context("prepare INSERT_WAD")?;
        let row = tx
            .query_one(
                &insert_wad,
                &[
                    &merged.meta.id,
                    &sha1,
                    &sha256,
                    &title_param,
                    &file_type,
                    &file_size,
                    &file_url_param,
                    &corrupt,
                    &corrupt_msg_param,
                    &engines_guess_param,
                    &iwads_guess_param,
                    &wa_updated_ts,
                    &Json(read_meta_json),
                    &preferred_filename_param,
                    &hidden,
                    &adult,
                    &can_download,
                    &locked,
                    &added_ts,
                ],
            )
            .await
            .context("exec INSERT_WAD")?;
        let wad_id: Uuid = row.get(0);

        // 2) Clear children so re-run is idempotent
        let delete_children = tx
            .prepare_cached(sql::DELETE_WAD_CHILDREN)
            .await
            .context("prepare DELETE_WAD_CHILDREN")?;
        tx.execute(&delete_children, &[&wad_id])
            .await
            .context("exec DELETE_WAD_CHILDREN")?;

        // 3) Insert filenames/authors/descriptions/maps list/text files
        if let Some(filenames) = &merged.meta.filenames {
            let stmt = tx
                .prepare_cached(sql::INSERT_WAD_FILENAME)
                .await
                .context("prepare INSERT_WAD_FILENAME")?;
            for (ord, f) in filenames.iter().enumerate() {
                let f_escaped = escape_nul_in_str(f);
                let f_param = f_escaped.as_deref().unwrap_or(f);
                tx.execute(&stmt, &[&wad_id, &f_param, &(ord as i32)])
                    .await
                    .with_context(|| format!("insert filename ord={ord}"))?;
            }
        }

        if let Some(authors) = &merged.meta.authors {
            let stmt = tx
                .prepare_cached(sql::INSERT_WAD_AUTHOR)
                .await
                .context("prepare INSERT_WAD_AUTHOR")?;
            for (ord, a) in authors.iter().enumerate() {
                let a_escaped = escape_nul_in_str(a);
                let a_param = a_escaped.as_deref().unwrap_or(a);
                tx.execute(&stmt, &[&wad_id, &a_param, &(ord as i32)])
                    .await
                    .with_context(|| format!("insert author ord={ord}"))?;
            }
        }

        if let Some(descs) = &merged.meta.descriptions {
            let stmt = tx
                .prepare_cached(sql::INSERT_WAD_DESCRIPTION)
                .await
                .context("prepare INSERT_WAD_DESCRIPTION")?;
            for (ord, d) in descs.iter().enumerate() {
                let d_escaped = escape_nul_in_str(d);
                let d_param = d_escaped.as_deref().unwrap_or(d);
                tx.execute(&stmt, &[&wad_id, &d_param, &(ord as i32)])
                    .await
                    .with_context(|| format!("insert description ord={ord}"))?;
            }
        }

        if let Some(maps) = &merged.meta.content.maps {
            let stmt = tx
                .prepare_cached(sql::INSERT_WAD_MAP_LIST)
                .await
                .context("prepare INSERT_WAD_MAP_LIST")?;
            for (ord, m) in maps.iter().enumerate() {
                let m_escaped = escape_nul_in_str(m);
                let m_param = m_escaped.as_deref().unwrap_or(m);
                tx.execute(&stmt, &[&wad_id, &m_param, &(ord as i32)])
                    .await
                    .with_context(|| format!("insert map_list ord={ord} map={m_param}"))?;
            }
        }

        if let Some(tfs) = &merged.meta.text_files {
            let stmt = tx
                .prepare_cached(sql::INSERT_WAD_TEXT_FILE)
                .await
                .context("prepare INSERT_WAD_TEXT_FILE")?;
            for (ord, tf) in tfs.iter().enumerate() {
                let source_escaped = escape_nul_in_str(&tf.source);
                let source_param = source_escaped.as_deref().unwrap_or(tf.source.as_str());

                let name = tf.name.as_deref();
                let name_escaped = name.and_then(escape_nul_in_str);
                let name_param: Option<&str> = name_escaped.as_deref().or(name);

                let contents_escaped = escape_nul_in_str(&tf.contents);
                let contents_param = contents_escaped.as_deref().unwrap_or(tf.contents.as_str());

                tx.execute(
                    &stmt,
                    &[
                        &wad_id,
                        &source_param,
                        &name_param,
                        &contents_param,
                        &(ord as i32),
                    ],
                )
                .await
                .with_context(|| format!("insert text_file ord={ord}"))?;
            }
        }

        // 4) counts (jsonb)
        if let Some(counts) = &merged.meta.content.counts {
            // Counts is a string-keyed map; sanitize keys defensively.
            let mut counts_sanitized: BTreeMap<String, i64> = BTreeMap::new();
            for (k, v) in counts.iter() {
                let k_escaped = escape_nul_in_str(k);
                counts_sanitized.insert(k_escaped.unwrap_or_else(|| k.clone()), *v);
            }
            let mut counts_json =
                serde_json::to_value(counts_sanitized).context("serialize counts")?;
            _ = escape_nul_in_json(&mut counts_json);
            let stmt = tx
                .prepare_cached(sql::UPSERT_WAD_COUNTS)
                .await
                .context("prepare UPSERT_WAD_COUNTS")?;
            tx.execute(&stmt, &[&wad_id, &Json(counts_json)])
                .await
                .context("exec UPSERT_WAD_COUNTS")?;
        }

        // 5) sources: wad_archive, extracted, idgames
        {
            let mut hashes_json = serde_json::to_value(&merged.meta.sources.wad_archive.hashes)
                .context("serialize wad_archive.hashes")?;
            _ = escape_nul_in_json(&mut hashes_json);
            let stmt = tx
                .prepare_cached(sql::UPSERT_WAD_SOURCE_WAD_ARCHIVE)
                .await
                .context("prepare UPSERT_WAD_SOURCE_WAD_ARCHIVE")?;
            tx.execute(&stmt, &[&wad_id, &wa_updated_ts, &Json(hashes_json)])
                .await
                .context("exec UPSERT_WAD_SOURCE_WAD_ARCHIVE")?;

            let mut extracted_json = serde_json::to_value(&merged.meta.sources.extracted)
                .context("serialize extracted")?;
            _ = escape_nul_in_json(&mut extracted_json);
            let stmt = tx
                .prepare_cached(sql::UPSERT_WAD_SOURCE_EXTRACTED)
                .await
                .context("prepare UPSERT_WAD_SOURCE_EXTRACTED")?;
            tx.execute(&stmt, &[&wad_id, &Json(extracted_json)])
                .await
                .context("exec UPSERT_WAD_SOURCE_EXTRACTED")?;

            if let Some(ig) = &merged.meta.sources.idgames {
                let url = ig.url.as_deref();
                let url_escaped = url.and_then(escape_nul_in_str);
                let url_param: Option<&str> = url_escaped.as_deref().or(url);

                let dir = ig.dir.as_deref();
                let dir_escaped = dir.and_then(escape_nul_in_str);
                let dir_param: Option<&str> = dir_escaped.as_deref().or(dir);

                let filename = ig.filename.as_deref();
                let filename_escaped = filename.and_then(escape_nul_in_str);
                let filename_param: Option<&str> = filename_escaped.as_deref().or(filename);

                let date = ig.date.as_deref();
                let date_escaped = date.and_then(escape_nul_in_str);
                let date_param: Option<&str> = date_escaped.as_deref().or(date);

                let ig_title = ig.title.as_deref();
                let ig_title_escaped = ig_title.and_then(escape_nul_in_str);
                let ig_title_param: Option<&str> = ig_title_escaped.as_deref().or(ig_title);

                let author = ig.author.as_deref();
                let author_escaped = author.and_then(escape_nul_in_str);
                let author_param: Option<&str> = author_escaped.as_deref().or(author);

                let credits = ig.credits.as_deref();
                let credits_escaped = credits.and_then(escape_nul_in_str);
                let credits_param: Option<&str> = credits_escaped.as_deref().or(credits);

                let mut ig_raw_json = serde_json::to_value(ig).context("serialize idgames")?;
                _ = escape_nul_in_json(&mut ig_raw_json);
                let stmt = tx
                    .prepare_cached(sql::UPSERT_WAD_SOURCE_IDGAMES)
                    .await
                    .context("prepare UPSERT_WAD_SOURCE_IDGAMES")?;
                tx.execute(
                    &stmt,
                    &[
                        &wad_id,
                        &ig.id,
                        &url_param,
                        &dir_param,
                        &filename_param,
                        &date_param,
                        &ig_title_param,
                        &author_param,
                        &credits_param,
                        &ig.rating,
                        &ig.votes.map(|v| v as i32),
                        &Json(ig_raw_json),
                    ],
                )
                .await
                .context("exec UPSERT_WAD_SOURCE_IDGAMES")?;
            } else {
                let stmt = tx
                    .prepare_cached(sql::DELETE_WAD_SOURCE_IDGAMES)
                    .await
                    .context("prepare DELETE_WAD_SOURCE_IDGAMES")?;
                tx.execute(&stmt, &[&wad_id])
                    .await
                    .context("exec DELETE_WAD_SOURCE_IDGAMES")?;
            }
        }

        // 6) Per-map stats + breakdown tables
        {
            let insert_map = tx
                .prepare_cached(sql::INSERT_WAD_MAP)
                .await
                .context("prepare INSERT_WAD_MAP")?;
            let insert_tex = tx
                .prepare_cached(sql::INSERT_WAD_MAP_TEXTURE)
                .await
                .context("prepare INSERT_WAD_MAP_TEXTURE")?;
            let insert_mon = tx
                .prepare_cached(sql::INSERT_WAD_MAP_MONSTER)
                .await
                .context("prepare INSERT_WAD_MAP_MONSTER")?;
            let insert_item = tx
                .prepare_cached(sql::INSERT_WAD_MAP_ITEM)
                .await
                .context("prepare INSERT_WAD_MAP_ITEM")?;

            for m in &merged.maps {
                let map_name_escaped = escape_nul_in_str(&m.map);
                let map_name_param = map_name_escaped.as_deref().unwrap_or(m.map.as_str());

                let format_escaped = escape_nul_in_str(&m.format);
                let format_param = format_escaped.as_deref().unwrap_or(m.format.as_str());

                let compat_escaped = escape_nul_in_str(&m.compatibility);
                let compat_param = compat_escaped
                    .as_deref()
                    .unwrap_or(m.compatibility.as_str());

                let keys_escaped = escape_nul_in_vec(&m.mechanics.keys);
                let keys_param: &Vec<String> = keys_escaped.as_ref().unwrap_or(&m.mechanics.keys);

                let meta_title = m.metadata.title.as_deref();
                let meta_title_escaped = meta_title.and_then(escape_nul_in_str);
                let meta_title_param: Option<&str> = meta_title_escaped.as_deref().or(meta_title);

                let meta_music = m.metadata.music.as_deref();
                let meta_music_escaped = meta_music.and_then(escape_nul_in_str);
                let meta_music_param: Option<&str> = meta_music_escaped.as_deref().or(meta_music);

                let meta_source_escaped = escape_nul_in_str(&m.metadata.source);
                let meta_source_param = meta_source_escaped
                    .as_deref()
                    .unwrap_or(m.metadata.source.as_str());

                let mut map_json = serde_json::to_value(m).context("serialize map stat")?;
                _ = escape_nul_in_json(&mut map_json);

                let things: i32 = m.stats.things.try_into().context("map things overflow")?;
                let linedefs: i32 = m
                    .stats
                    .linedefs
                    .try_into()
                    .context("map linedefs overflow")?;
                let sidedefs: i32 = m
                    .stats
                    .sidedefs
                    .try_into()
                    .context("map sidedefs overflow")?;
                let vertices: i32 = m
                    .stats
                    .vertices
                    .try_into()
                    .context("map vertices overflow")?;
                let sectors: i32 = m.stats.sectors.try_into().context("map sectors overflow")?;
                let segs: i32 = m.stats.segs.try_into().context("map segs overflow")?;
                let ssectors: i32 = m
                    .stats
                    .ssectors
                    .try_into()
                    .context("map ssectors overflow")?;
                let nodes: i32 = m.stats.nodes.try_into().context("map nodes overflow")?;

                let monster_total: i32 = m
                    .monsters
                    .total
                    .try_into()
                    .context("map monster_total overflow")?;
                let uv_monsters: i32 = m
                    .difficulty
                    .uv_monsters
                    .try_into()
                    .context("map uv_monsters overflow")?;
                let hmp_monsters: i32 = m
                    .difficulty
                    .hmp_monsters
                    .try_into()
                    .context("map hmp_monsters overflow")?;
                let htr_monsters: i32 = m
                    .difficulty
                    .htr_monsters
                    .try_into()
                    .context("map htr_monsters overflow")?;

                let item_total: i32 = m
                    .items
                    .total
                    .try_into()
                    .context("map item_total overflow")?;
                let uv_items: i32 = m
                    .difficulty
                    .uv_items
                    .try_into()
                    .context("map uv_items overflow")?;
                let hmp_items: i32 = m
                    .difficulty
                    .hmp_items
                    .try_into()
                    .context("map hmp_items overflow")?;
                let htr_items: i32 = m
                    .difficulty
                    .htr_items
                    .try_into()
                    .context("map htr_items overflow")?;

                tx.execute(
                    &insert_map,
                    &[
                        &wad_id,
                        &map_name_param,
                        &format_param,
                        &compat_param,
                        &things,
                        &linedefs,
                        &sidedefs,
                        &vertices,
                        &sectors,
                        &segs,
                        &ssectors,
                        &nodes,
                        &m.mechanics.teleports,
                        &m.mechanics.secret_exit,
                        keys_param,
                        &monster_total,
                        &uv_monsters,
                        &hmp_monsters,
                        &htr_monsters,
                        &item_total,
                        &uv_items,
                        &hmp_items,
                        &htr_items,
                        &meta_title_param,
                        &meta_music_param,
                        &meta_source_param,
                        &Json(map_json),
                    ],
                )
                .await
                .with_context(|| format!("insert wad_map {map_name_param}"))?;

                for (tex, cnt) in &m.stats.textures {
                    let tex_escaped = escape_nul_in_str(tex);
                    let tex_param = tex_escaped.as_deref().unwrap_or(tex);
                    let cnt_param = *cnt;
                    tx.execute(
                        &insert_tex,
                        &[&wad_id, &map_name_param, &tex_param, &cnt_param],
                    )
                    .await
                    .with_context(|| {
                        format!("insert texture {map_name_param} {tex_param} {cnt_param}")
                    })?;
                }

                for (monster, cnt) in &m.monsters.by_type {
                    let cnt: i32 = (*cnt).try_into().with_context(|| {
                        format!("monster count overflow {map_name_param} {monster}")
                    })?;
                    let monster_escaped = escape_nul_in_str(monster);
                    let monster_param = monster_escaped.as_deref().unwrap_or(monster);
                    tx.execute(
                        &insert_mon,
                        &[&wad_id, &map_name_param, &monster_param, &cnt],
                    )
                    .await
                    .with_context(|| format!("insert monster {map_name_param} {monster_param}"))?;
                }

                for (item, cnt) in &m.items.by_type {
                    let cnt: i32 = (*cnt)
                        .try_into()
                        .with_context(|| format!("item count overflow {map_name_param} {item}"))?;
                    let item_escaped = escape_nul_in_str(item);
                    let item_param = item_escaped.as_deref().unwrap_or(item);
                    tx.execute(&insert_item, &[&wad_id, &map_name_param, &item_param, &cnt])
                        .await
                        .with_context(|| format!("insert item {map_name_param} {item_param}"))?;
                }
            }
        }

        tx.commit().await.context("commit insert_wad tx")?;
        Ok(wad_id)
    }
}

fn parse_ts_any(s: &Option<String>) -> Option<chrono::DateTime<chrono::Utc>> {
    let s = s.as_ref()?.trim();
    if s.is_empty() {
        return None;
    }
    // Prefer timezone-aware timestamps, but also accept naive timestamps (assume UTC).
    if let Ok(dt) = chrono::DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%z") {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            naive,
            chrono::Utc,
        ));
    }
    None
}

async fn create_tables(conn: &mut deadpool_postgres::Client) {
    let stmts = strip_sql_comments(sql::TABLES);
    let stmts = stmts.split(';');
    let tx = conn.transaction().await.expect("begin tx");
    for stmt in stmts {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        if let Err(e) = tx.simple_query(stmt).await {
            panic!("Failed to execute statement '{}': {:?}", stmt, e);
        }
    }
    tx.commit().await.expect("commit tx");
}
