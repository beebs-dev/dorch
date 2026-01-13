use crate::client::{GetWadMapResponse, WadListResults, WadSearchResults};
use anyhow::{Context, Result};
use dorch_common::{
    postgres::strip_sql_comments,
    types::wad::{MapStat, WadMergedOut, WadMeta},
};
use tokio_postgres::types::Json;
use uuid::Uuid;

mod sql {
    pub const TABLES: &str = include_str!("sql/tables.sql");
    pub const INSERT_WAD: &str = include_str!("sql/insert_wad.sql");
    pub const SEARCH_WADS: &str = include_str!("sql/search_wads.sql");

    pub const LIST_WADS_ASC: &str = include_str!("sql/list_wads_asc.sql");
    pub const LIST_WADS_DESC: &str = include_str!("sql/list_wads_desc.sql");

    pub const GET_WAD: &str = include_str!("sql/get_wad.sql");
    pub const GET_WAD_MAPS: &str = include_str!("sql/get_wad_maps.sql");
    pub const GET_WAD_MAP: &str = include_str!("sql/get_wad_map.sql");

    pub const DELETE_WAD_CHILDREN: &str = include_str!("sql/delete_wad_children.sql");
    pub const INSERT_WAD_AUTHOR: &str = include_str!("sql/insert_wad_author.sql");
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
}

#[derive(Clone)]
pub struct Database {
    pool: deadpool_postgres::Pool,
}

impl Database {
    pub async fn new(pool: deadpool_postgres::Pool) -> Result<Self> {
        let mut conn = pool.get().await.context("failed to get connection")?;
        create_tables(&mut conn).await;
        _ = conn
            .prepare(sql::INSERT_WAD)
            .await
            .context("failed to prepare INSERT_WAD")?;
        _ = conn
            .prepare(sql::SEARCH_WADS)
            .await
            .context("failed to prepare SEARCH_WADS")?;

        _ = conn
            .prepare(sql::LIST_WADS_ASC)
            .await
            .context("failed to prepare LIST_WADS_ASC")?;
        _ = conn
            .prepare(sql::LIST_WADS_DESC)
            .await
            .context("failed to prepare LIST_WADS_DESC")?;

        _ = conn
            .prepare(sql::GET_WAD)
            .await
            .context("failed to prepare GET_WAD")?;
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
        Ok(Self { pool })
    }

    pub async fn get_wad(&self, wad_id: Uuid) -> Result<Option<WadMergedOut>> {
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
        let mut meta: WadMeta = serde_json::from_value(meta_json).context("deserialize WadMeta")?;
        if meta.id.is_nil() {
            meta.id = row_wad_id;
        }

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
            let map: MapStat = serde_json::from_value(map_json).context("deserialize MapStat")?;
            maps.push(map);
        }

        Ok(Some(WadMergedOut { meta, maps }))
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
        let mut wad_meta: WadMeta =
            serde_json::from_value(meta_json).context("deserialize WadMeta")?;
        if wad_meta.id.is_nil() {
            wad_meta.id = row_wad_id;
        }

        let map_json: serde_json::Value = row.try_get("map_json")?;
        let map: MapStat = serde_json::from_value(map_json).context("deserialize MapStat")?;

        Ok(Some(GetWadMapResponse { map, wad_meta }))
    }

    pub async fn search_wads(
        &self,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<WadSearchResults> {
        let mut conn = self.pool.get().await.context("failed to get connection")?;
        let tx = conn
            .transaction()
            .await
            .context("failed to begin transaction")?;
        let search_wads = tx
            .prepare_cached(sql::SEARCH_WADS)
            .await
            .context("failed to prepare SEARCH_WADS")?;
        let rows = tx
            .query(&search_wads, &[&query, &offset, &limit])
            .await
            .context("failed to execute SEARCH_WADS")?;
        let full_count = rows
            .first()
            .map(|r| r.try_get::<_, i64>("full_count"))
            .transpose()
            .context("failed to get full_count from SEARCH_WADS")?
            .unwrap_or(0);

        let items = rows
            .into_iter()
            .map(|row| {
                let row_wad_id: Uuid = row.try_get("wad_id")?;
                let meta_json: serde_json::Value = row.try_get("meta_json")?;
                let mut meta =
                    serde_json::from_value::<dorch_common::types::wad::WadMeta>(meta_json)
                        .context("deserialize WadMeta from meta_json")?;
                if meta.id.is_nil() {
                    meta.id = row_wad_id;
                }
                Ok(meta)
            })
            .collect::<Result<Vec<dorch_common::types::wad::WadMeta>>>()?;

        tx.commit().await.context("failed to commit transaction")?;
        Ok(WadSearchResults {
            query: query.to_string(),
            items,
            full_count,
            offset,
            limit,
            truncated: offset + limit < full_count,
        })
    }

    pub async fn list_wads(
        &self,
        offset: i64,
        limit: i64,
        sort_desc: bool,
    ) -> Result<WadListResults> {
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
                let mut meta = serde_json::from_value::<WadMeta>(meta_json)
                    .context("deserialize WadMeta from meta_json")?;
                if meta.id.is_nil() {
                    meta.id = row_wad_id;
                }
                Ok(meta)
            })
            .collect::<Result<Vec<WadMeta>>>()?;

        tx.commit().await.context("failed to commit transaction")?;
        Ok(WadListResults {
            items,
            full_count,
            offset,
            limit,
            truncated: offset + limit < full_count,
        })
    }

    pub async fn insert_wad(&self, merged: &WadMergedOut) -> Result<Uuid> {
        let mut conn = self.pool.get().await.context("failed to get connection")?;
        let tx = conn
            .transaction()
            .await
            .context("failed to begin transaction")?;
        let meta_json = serde_json::to_value(&merged.meta).context("serialize merged.meta")?;
        let wa_updated_ts = parse_updated_ts(&merged.meta.sources.wad_archive.updated);

        let sha1 = merged.meta.sha1.as_str();
        let sha256 = merged.meta.sha256.as_deref();
        let title = merged.meta.title.as_deref();

        let file_type = merged.meta.file.file_type.as_str();
        let file_size = merged.meta.file.size;
        let file_url = merged.meta.file.url.as_deref();
        let corrupt = merged.meta.file.corrupt;
        let corrupt_msg = merged.meta.file.corrupt_message.as_deref();

        let engines_guess = merged.meta.content.engines_guess.as_ref();
        let iwads_guess = merged.meta.content.iwads_guess.as_ref();

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
                    &title,
                    &file_type,
                    &file_size,
                    &file_url,
                    &corrupt,
                    &corrupt_msg,
                    &engines_guess,
                    &iwads_guess,
                    &wa_updated_ts,
                    &Json(meta_json),
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

        // 3) Insert authors/descriptions/maps list/text files
        if let Some(authors) = &merged.meta.authors {
            let stmt = tx
                .prepare_cached(sql::INSERT_WAD_AUTHOR)
                .await
                .context("prepare INSERT_WAD_AUTHOR")?;
            for (ord, a) in authors.iter().enumerate() {
                tx.execute(&stmt, &[&wad_id, &a, &(ord as i32)])
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
                tx.execute(&stmt, &[&wad_id, &d, &(ord as i32)])
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
                tx.execute(&stmt, &[&wad_id, &m, &(ord as i32)])
                    .await
                    .with_context(|| format!("insert map_list ord={ord} map={m}"))?;
            }
        }

        if let Some(tfs) = &merged.meta.text_files {
            let stmt = tx
                .prepare_cached(sql::INSERT_WAD_TEXT_FILE)
                .await
                .context("prepare INSERT_WAD_TEXT_FILE")?;
            for (ord, tf) in tfs.iter().enumerate() {
                tx.execute(
                    &stmt,
                    &[&wad_id, &tf.source, &tf.name, &tf.contents, &(ord as i32)],
                )
                .await
                .with_context(|| format!("insert text_file ord={ord}"))?;
            }
        }

        // 4) counts (jsonb)
        if let Some(counts) = &merged.meta.content.counts {
            let counts_json = serde_json::to_value(counts).context("serialize counts")?;
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
            let hashes_json = serde_json::to_value(&merged.meta.sources.wad_archive.hashes)
                .context("serialize wad_archive.hashes")?;
            let stmt = tx
                .prepare_cached(sql::UPSERT_WAD_SOURCE_WAD_ARCHIVE)
                .await
                .context("prepare UPSERT_WAD_SOURCE_WAD_ARCHIVE")?;
            tx.execute(&stmt, &[&wad_id, &wa_updated_ts, &Json(hashes_json)])
                .await
                .context("exec UPSERT_WAD_SOURCE_WAD_ARCHIVE")?;

            let extracted_json = serde_json::to_value(&merged.meta.sources.extracted)
                .context("serialize extracted")?;
            let stmt = tx
                .prepare_cached(sql::UPSERT_WAD_SOURCE_EXTRACTED)
                .await
                .context("prepare UPSERT_WAD_SOURCE_EXTRACTED")?;
            tx.execute(&stmt, &[&wad_id, &Json(extracted_json)])
                .await
                .context("exec UPSERT_WAD_SOURCE_EXTRACTED")?;

            if let Some(ig) = &merged.meta.sources.idgames {
                let ig_raw_json = serde_json::to_value(ig).context("serialize idgames")?;
                let stmt = tx
                    .prepare_cached(sql::UPSERT_WAD_SOURCE_IDGAMES)
                    .await
                    .context("prepare UPSERT_WAD_SOURCE_IDGAMES")?;
                tx.execute(
                    &stmt,
                    &[
                        &wad_id,
                        &ig.id,
                        &ig.url,
                        &ig.dir,
                        &ig.filename,
                        &ig.date,
                        &ig.title,
                        &ig.author,
                        &ig.credits,
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
            use std::collections::HashSet;

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
                let map_json = serde_json::to_value(m).context("serialize map stat")?;

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
                        &m.map,
                        &m.format,
                        &m.compatibility,
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
                        &m.mechanics.keys,
                        &monster_total,
                        &uv_monsters,
                        &hmp_monsters,
                        &htr_monsters,
                        &item_total,
                        &uv_items,
                        &hmp_items,
                        &htr_items,
                        &m.metadata.title,
                        &m.metadata.music,
                        &m.metadata.source,
                        &Json(map_json),
                    ],
                )
                .await
                .with_context(|| format!("insert wad_map {}", m.map))?;

                let mut textures_seen: HashSet<&str> = HashSet::new();
                for tex in &m.stats.textures {
                    if !textures_seen.insert(tex.as_str()) {
                        continue;
                    }
                    tx.execute(&insert_tex, &[&wad_id, &m.map, &tex])
                        .await
                        .with_context(|| format!("insert texture {} {}", m.map, tex))?;
                }

                for (monster, cnt) in &m.monsters.by_type {
                    let cnt: i32 = (*cnt)
                        .try_into()
                        .with_context(|| format!("monster count overflow {} {}", m.map, monster))?;
                    tx.execute(&insert_mon, &[&wad_id, &m.map, &monster, &cnt])
                        .await
                        .with_context(|| format!("insert monster {} {}", m.map, monster))?;
                }

                for (item, cnt) in &m.items.by_type {
                    let cnt: i32 = (*cnt)
                        .try_into()
                        .with_context(|| format!("item count overflow {} {}", m.map, item))?;
                    tx.execute(&insert_item, &[&wad_id, &m.map, &item, &cnt])
                        .await
                        .with_context(|| format!("insert item {} {}", m.map, item))?;
                }
            }
        }

        tx.commit().await.context("commit insert_wad tx")?;
        Ok(wad_id)
    }
}

fn parse_updated_ts(s: &Option<String>) -> Option<chrono::DateTime<chrono::Utc>> {
    let s = s.as_ref()?.trim();
    if s.is_empty() {
        return None;
    }
    chrono::DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%z")
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .ok()
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
