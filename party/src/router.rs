use crate::{args, party_store::PartyInfoStore};
use anyhow::{Context, Result, bail};
use async_nats::{ConnectOptions, Subscriber};
use bytes::Bytes;
use dorch_common::streams::subjects;
use owo_colors::OwoColorize;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub async fn run(args: args::RouterArgs) -> Result<()> {
    let pool = dorch_common::redis::init_redis(&args.redis).await;
    let store = PartyInfoStore::new(pool);
    let nats = async_nats::connect_with_options(
        &args.nats.nats_url,
        ConnectOptions::new()
            .user_and_password(args.nats.nats_user.clone(), args.nats.nats_password.clone()),
    )
    .await
    .context("Failed to connect to NATS")?;
    let sub = nats
        .subscribe(subjects::party("*"))
        .await
        .context("Failed to subscribe to party subject")?;
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        dorch_common::shutdown::shutdown_signal().await;
        cancel_clone.cancel();
    });
    dorch_common::signal_ready();
    println!("{}", "🚀 Starting party router".green());
    run_inner(cancel, nats, sub, store).await
}

async fn run_inner(
    cancel: CancellationToken,
    nats: async_nats::Client,
    mut sub: Subscriber,
    store: PartyInfoStore,
) -> Result<()> {
    loop {
        tokio::select! {
            _ = cancel.cancelled() => bail!("Context cancelled"),
            maybe_msg = sub.next() => {
                match maybe_msg {
                    None => bail!("NATS subscription closed"),
                    Some(msg) => {
                        if let Err(e) = process(&nats, &store, &msg.subject, &msg.payload).await {
                            eprintln!("Failed to process message on subject {}: {:?}", msg.subject, e);
                        }
                    }
                }
            }
        }
    }
}

fn extract_party_id(subject: &str) -> Result<Uuid> {
    let template = subjects::party("*");
    let parts: Vec<&str> = template.split('*').collect();
    if parts.len() != 2 {
        bail!("Invalid subject format: {}", subject);
    }
    let prefix = parts[0];
    let suffix = parts[1];
    if !subject.starts_with(prefix) || !subject.ends_with(suffix) {
        bail!("Subject does not match template: {}", subject);
    }
    let party_id_str = &subject[prefix.len()..subject.len() - suffix.len()];
    Uuid::parse_str(party_id_str).context("Failed to parse party ID from subject")
}

async fn process(
    nats: &async_nats::Client,
    store: &PartyInfoStore,
    subject: &str,
    payload: &Bytes,
) -> Result<()> {
    let party_id = extract_party_id(subject)?;
    let members = store
        .list_members(party_id)
        .await
        .context("Failed to list party members")?;
    for user_id in members {
        nats.publish(subjects::user(user_id), payload.clone())
            .await
            .context("Failed to publish message to user")?;
    }
    Ok(())
}
