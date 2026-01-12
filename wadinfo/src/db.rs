use crate::client::{ListWadsResponse, SearchWadsResponse, Wad, WadMap, WadMeta, WadSearchResult};
use anyhow::{Context, Result, anyhow};
use uuid::Uuid;

mod sql {
    pub const TABLES: &str = include_str!("sql/tables.sql");
    pub const GET_WAD: &str = include_str!("sql/get_wad.sql");
    pub const GET_WAD_MAP: &str = include_str!("sql/get_wad_map.sql");
    pub const LIST_WADS: &str = include_str!("sql/list_wads.sql");
    pub const LIST_WAD_MAPS: &str = include_str!("sql/list_wad_maps.sql");
    pub const INSERT_WAD: &str = include_str!("sql/insert_wad.sql");
    pub const INSERT_WAD_MAP: &str = include_str!("sql/insert_wad_map.sql");
    pub const LIST_WAD_MAP_NAMES: &str = include_str!("sql/list_wad_map_names.sql");
    pub const SEARCH_WADS: &str = include_str!("sql/search_wads.sql");
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
            .prepare(sql::GET_WAD)
            .await
            .context("failed to prepare GET_WAD")?;
        _ = conn
            .prepare(sql::LIST_WADS)
            .await
            .context("failed to prepare LIST_WADS")?;
        _ = conn
            .prepare(sql::LIST_WAD_MAPS)
            .await
            .context("failed to prepare LIST_WAD_MAPS")?;
        _ = conn
            .prepare(sql::INSERT_WAD)
            .await
            .context("failed to prepare INSERT_WAD")?;
        _ = conn
            .prepare(sql::INSERT_WAD_MAP)
            .await
            .context("failed to prepare INSERT_WAD_MAP")?;
        _ = conn
            .prepare(sql::LIST_WAD_MAP_NAMES)
            .await
            .context("failed to prepare LIST_WAD_MAP_NAMES")?;
        _ = conn
            .prepare(sql::GET_WAD_MAP)
            .await
            .context("failed to prepare GET_WAD_MAP")?;
        _ = conn
            .prepare(sql::SEARCH_WADS)
            .await
            .context("failed to prepare SEARCH_WADS")?;
        Ok(Self { pool })
    }

    pub async fn search_wads(
        &self,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<SearchWadsResponse> {
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
            .query(&search_wads, &[&query])
            .await
            .context("failed to execute SEARCH_WADS")?;
        let items = rows
            .into_iter()
            .map(|row| {
                let rank: f32 = row
                    .try_get("rank")
                    .context("failed to get rank from SEARCH_WADS")?;
                let meta =
                    WadMeta::try_from(row).context("failed to parse WadMeta from SEARCH_WADS")?;
                Ok(WadSearchResult { meta, rank })
            })
            .collect::<Result<Vec<WadSearchResult>>>()?;
        tx.commit().await.context("failed to commit transaction")?;
        Ok(SearchWadsResponse {
            offset,
            limit,
            items,
        })
    }

    pub async fn insert_wad(&self, meta: &WadMeta, maps: &[WadMap]) -> Result<Uuid> {
        let mut conn = self.pool.get().await.context("failed to get connection")?;
        let tx = conn
            .transaction()
            .await
            .context("failed to begin transaction")?;
        let insert_wad = tx
            .prepare_cached(sql::INSERT_WAD)
            .await
            .context("failed to prepare INSERT_WAD")?;
        let insert_wad_map = tx
            .prepare_cached(sql::INSERT_WAD_MAP)
            .await
            .context("failed to prepare INSERT_WAD_MAP")?;
        let wad_id = tx
            .query_one(
                &insert_wad,
                &[
                    &meta.sha1,
                    &meta.filename,
                    &meta.wad_type,
                    &meta.byte_size,
                    &meta.map_count,
                ],
            )
            .await
            .context("failed to execute INSERT_WAD")?
            .try_get("wad_id")
            .context("failed to get wad_id from INSERT_WAD")?;
        for map in maps {
            tx.execute(
                &insert_wad_map,
                &[
                    &wad_id, // use the newly generated wad_id
                    &map.map_name,
                    &map.format,
                    &map.compatibility,
                    //
                    &map.things,
                    &map.linedefs,
                    &map.sidedefs,
                    &map.vertices,
                    &map.sectors,
                    &map.segs,
                    &map.ssectors,
                    &map.nodes,
                    //
                    &map.teleports,
                    &map.secret_exit,
                    //
                    &map.monster_total,
                    &map.uv_monsters,
                    &map.hmp_monsters,
                    &map.htr_monsters,
                    //
                    &map.zombieman_count,
                    &map.shotgun_guy_count,
                    &map.chaingun_guy_count,
                    &map.imp_count,
                    &map.demon_count,
                    &map.spectre_count,
                    &map.cacodemon_count,
                    &map.lost_soul_count,
                    &map.pain_elemental_count,
                    &map.revenant_count,
                    &map.mancubus_count,
                    &map.arachnotron_count,
                    &map.hell_knight_count,
                    &map.baron_count,
                    &map.archvile_count,
                    &map.cyberdemon_count,
                    &map.spider_mastermind_count,
                    //
                    &map.keys,
                    &map.doc,
                ],
            )
            .await
            .context("failed to execute INSERT_WAD_MAP")?;
        }
        tx.commit().await.context("failed to commit transaction")?;
        Ok(wad_id)
    }

    pub async fn list_wads(&self, offset: i64, limit: i64) -> Result<ListWadsResponse> {
        let mut conn = self.pool.get().await.context("failed to get connection")?;
        let tx = conn
            .transaction()
            .await
            .context("failed to begin transaction")?;
        let list_wads = tx
            .prepare_cached(sql::LIST_WADS)
            .await
            .context("failed to prepare LIST_WADS")?;
        let rows = tx
            .query(&list_wads, &[&offset, &limit])
            .await
            .context("failed to execute LIST_WADS")?;
        let full_count = rows
            .first()
            .map(|r| r.try_get::<_, i64>("full_count"))
            .transpose()
            .context("failed to get full_count from LIST_WADS")?
            .ok_or_else(|| anyhow!("no rows returned from LIST_WADS"))?;
        let items = rows
            .into_iter()
            .map(WadMeta::try_from)
            .collect::<Result<Vec<WadMeta>>>()?;
        tx.commit().await.context("failed to commit transaction")?;
        Ok(ListWadsResponse {
            offset,
            limit,
            full_count,
            items,
            truncated: offset + limit < full_count,
        })
    }

    pub async fn list_wad_maps(&self, wad_id: Uuid) -> Result<Vec<WadMap>> {
        let mut conn = self.pool.get().await.context("failed to get connection")?;
        let tx = conn
            .transaction()
            .await
            .context("failed to begin transaction")?;
        let list_wad_maps = tx
            .prepare_cached(sql::LIST_WAD_MAPS)
            .await
            .context("failed to prepare LIST_WAD_MAPS")?;
        let items = tx
            .query(&list_wad_maps, &[&wad_id])
            .await
            .context("failed to execute LIST_WAD_MAPS")?
            .into_iter()
            .map(WadMap::try_from)
            .collect::<Result<Vec<WadMap>>>()?;
        tx.commit().await.context("failed to commit transaction")?;
        Ok(items)
    }

    pub async fn get_wad_map(&self, wad_id: Uuid, map_name: &str) -> Result<Option<WadMap>> {
        let mut conn = self.pool.get().await.context("failed to get connection")?;
        let tx = conn
            .transaction()
            .await
            .context("failed to begin transaction")?;
        let get_wad_map = tx
            .prepare_cached(sql::GET_WAD_MAP)
            .await
            .context("failed to prepare GET_WAD_MAP")?;
        let item = tx
            .query_opt(&get_wad_map, &[&wad_id, &map_name])
            .await
            .context("failed to execute GET_WAD_MAP")?
            .map(WadMap::try_from)
            .transpose()
            .context("failed to parse WadMap from GET_WAD_MAP")?;
        tx.commit().await.context("failed to commit transaction")?;
        Ok(item)
    }

    pub async fn get_wad(&self, wad_id: Uuid) -> Result<Option<Wad>> {
        let mut conn = self.pool.get().await.context("failed to get connection")?;
        let tx = conn
            .transaction()
            .await
            .context("failed to begin transaction")?;
        let get_wad = tx
            .prepare_cached(sql::GET_WAD)
            .await
            .context("failed to prepare GET_WAD")?;
        let list_wad_map_names = tx
            .prepare_cached(sql::LIST_WAD_MAP_NAMES)
            .await
            .context("failed to prepare LIST_WAD_MAP_NAMES")?;
        let Some(meta) = tx
            .query_opt(&get_wad, &[&wad_id])
            .await
            .context("failed to execute GET_WAD")?
            .map(WadMeta::try_from)
            .transpose()
            .context("failed to parse WadMeta from GET_WAD")?
        else {
            return Ok(None);
        };
        let map_names = tx
            .query(&list_wad_map_names, &[&wad_id])
            .await
            .context("failed to execute LIST_WAD_MAP_NAMES")?
            .into_iter()
            .map(|row| {
                row.try_get::<_, String>("map_name")
                    .context("failed to get map_name")
            })
            .collect::<Result<Vec<String>>>()
            .context("failed to parse map names from LIST_WAD_MAP_NAMES")?;
        tx.commit().await.context("failed to commit transaction")?;
        Ok(Some(Wad { meta, map_names }))
    }
}

async fn create_tables(conn: &mut deadpool_postgres::Client) {
    let stmts = sql::TABLES.split(';');
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
