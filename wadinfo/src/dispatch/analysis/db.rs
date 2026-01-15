use anyhow::{Context, Result};
use dorch_common::postgres::strip_sql_comments;
use uuid::Uuid;

mod sql {
    pub const TABLES: &str = include_str!("../../sql/tables.sql");

    // Lock one undispatched row and return its wad_id.
    // SKIP LOCKED ensures multiple dispatchers can run concurrently.
    pub const PULL_ONE: &str = r#"
		select wad_id
		from wads
		where dispatched_analysis_at is null
		order by created_at asc
		limit 1
		for update skip locked
	"#;

    pub const MARK_DISPATCHED: &str = r#"
		update wads
		set dispatched_analysis_at = now()
		where wad_id = $1
	"#;
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
            .prepare(sql::PULL_ONE)
            .await
            .context("failed to prepare PULL_ONE")?;
        _ = conn
            .prepare(sql::MARK_DISPATCHED)
            .await
            .context("failed to prepare MARK_DISPATCHED")?;
        Ok(Self { pool })
    }

    pub async fn get_conn(&self) -> Result<deadpool_postgres::Client> {
        self.pool.get().await.context("failed to get connection")
    }

    pub async fn pull_one(&self, tx: &deadpool_postgres::Transaction<'_>) -> Result<Option<Uuid>> {
        let pull = tx
            .prepare_cached(sql::PULL_ONE)
            .await
            .context("failed to prepare_cached PULL_ONE")?;
        let row = tx
            .query_opt(&pull, &[])
            .await
            .context("failed to execute PULL_ONE")?;

        let Some(row) = row else {
            return Ok(None);
        };

        let wad_id: Uuid = row.try_get("wad_id")?;
        Ok(Some(wad_id))
    }

    pub async fn mark_dispatched_analysis(
        &self,
        tx: &deadpool_postgres::Transaction<'_>,
        wad_id: Uuid,
    ) -> Result<()> {
        let stmt = tx
            .prepare_cached(sql::MARK_DISPATCHED)
            .await
            .context("failed to prepare_cached MARK_DISPATCHED")?;
        tx.execute(&stmt, &[&wad_id])
            .await
            .context("failed to execute MARK_DISPATCHED")?;
        Ok(())
    }
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
