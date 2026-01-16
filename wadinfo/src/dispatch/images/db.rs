use anyhow::{Context, Result};
use dorch_common::postgres::strip_sql_comments;
use uuid::Uuid;

mod sql {
    pub const TABLES: &str = concat!(
        include_str!("../../sql/tables.sql"),
        "\n",
        include_str!("tables.sql")
    );

    // Lock one undispatched row and return its wad_id.
    // SKIP LOCKED ensures multiple dispatchers can run concurrently.
    pub const PULL_ONE: &str = r#"
        select w.wad_id
        from wads w
        left join wad_dispatch_images d
            on d.wad_id = w.wad_id
        where d.wad_id is null
          and exists (
              select 1
              from wad_maps m
              where m.wad_id = w.wad_id
          )
        order by w.created_at asc
        limit 1
        for update of w skip locked
    "#;

    pub const PULL_N: &str = r#"
        select w.wad_id
        from wads w
        left join wad_dispatch_images d
            on d.wad_id = w.wad_id
        where d.wad_id is null
          and exists (
              select 1
              from wad_maps m
              where m.wad_id = w.wad_id
          )
        order by w.created_at asc
        limit $1
        for update of w skip locked
    "#;

    pub const MARK_DISPATCHED: &str = r#"
        insert into wad_dispatch_images (wad_id)
        values ($1)
        on conflict (wad_id) do nothing
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
            .prepare(sql::PULL_N)
            .await
            .context("failed to prepare PULL_N")?;
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

    pub async fn pull_n(
        &self,
        tx: &deadpool_postgres::Transaction<'_>,
        n: usize,
    ) -> Result<Vec<Uuid>> {
        let pull = tx
            .prepare_cached(sql::PULL_N)
            .await
            .context("failed to prepare_cached PULL_N")?;
        tx.query(&pull, &[&(n as i64)])
            .await
            .context("failed to execute PULL_N")?
            .into_iter()
            .map(|row| {
                let wad_id: Uuid = row
                    .try_get("wad_id")
                    .context("failed to get wad_id from row")?;
                Ok(wad_id)
            })
            .collect::<Result<Vec<Uuid>>>()
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

    pub async fn mark_dispatched_images_many(
        &self,
        tx: &deadpool_postgres::Transaction<'_>,
        wad_ids: &[Uuid],
    ) -> Result<()> {
        let stmt = tx
            .prepare_cached(sql::MARK_DISPATCHED)
            .await
            .context("failed to prepare_cached MARK_DISPATCHED")?;
        for wad_id in wad_ids {
            tx.execute(&stmt, &[&wad_id])
                .await
                .context("failed to execute MARK_DISPATCHED")?;
        }
        Ok(())
    }

    pub async fn mark_dispatched_images(
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
