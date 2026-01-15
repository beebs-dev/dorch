use anyhow::{Context, Result};
use async_nats::ConnectOptions;
use rand::Rng;
use tokio::time::{Duration, sleep};
use tokio_util::sync::CancellationToken;

use crate::args::DispatchImagesArgs;

pub mod db;

pub async fn run(args: DispatchImagesArgs) -> Result<()> {
    let pool = dorch_common::postgres::create_pool(args.postgres.clone()).await;
    let db = db::Database::new(pool)
        .await
        .context("Failed to create dispatch-images database")?;

    let nats = async_nats::connect_with_options(
        &args.nats.nats_url,
        ConnectOptions::new().user_and_password(args.nats.nats_user, args.nats.nats_password),
    )
    .await
    .context("Failed to connect to NATS")?;
    let js = async_nats::jetstream::new(nats);

    let cancel = CancellationToken::new();
    tokio::spawn({
        let cancel = cancel.clone();
        async move {
            dorch_common::shutdown::shutdown_signal().await;
            cancel.cancel();
        }
    });

    let mut empty_pulls: u32 = 0;
    while !cancel.is_cancelled() {
        let mut conn = db.get_conn().await?;
        let tx = conn.transaction().await.context("begin tx")?;

        let Some(wad_id) = db.pull_one(&tx).await? else {
            tx.commit().await.context("commit empty pull tx")?;
            empty_pulls = empty_pulls.saturating_add(1);
            tokio::select! {
                _ = sleep(backoff_delay(empty_pulls)) => continue,
                _ = cancel.cancelled() => break,
            }
        };

        empty_pulls = 0;

        let wad_id_str = wad_id.to_string();
        let subject = format!("dorch.wad.{wad_id_str}.img");

        let publish_ack = js
            .publish(subject, wad_id_str.clone().into())
            .await
            .context("JetStream publish failed")?;
        publish_ack.await.context("JetStream publish ack failed")?;

        db.mark_dispatched_images(&tx, wad_id)
            .await
            .context("Failed to mark dispatched_images_at")?;
        tx.commit().await.context("commit dispatch-images tx")?;
    }

    Ok(())
}

fn backoff_delay(empty_pulls: u32) -> Duration {
    // Exponential backoff with cap at 15 seconds, plus random jitter.
    // Sequence (approx): 250ms, 500ms, 1s, 2s, 4s, 8s, 15s...
    let base_ms: u64 = 250;
    let exp = empty_pulls.min(16); // avoid overflow
    let shift = exp.min(63) as u32;
    let factor = 1_u64.checked_shl(shift).unwrap_or(u64::MAX);
    let backoff_ms = base_ms.saturating_mul(factor);
    let capped = Duration::from_millis(backoff_ms).min(Duration::from_secs(15));

    let mut rng = rand::rng();
    let jitter_ms: u64 = rng.random_range(0..=1000);
    capped + Duration::from_millis(jitter_ms)
}
