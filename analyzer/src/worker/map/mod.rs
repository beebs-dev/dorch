use crate::{
    args,
    worker::{
        analyzer::Analyzer,
        app::{App, DeriveResult, Work, Worker},
    },
};
use anyhow::{Context, Result, anyhow};
use async_nats::{
    ConnectOptions,
    jetstream::{
        consumer::{AckPolicy, pull},
        stream::Stream,
    },
};
use bytes::Bytes;
use dorch_common::streams::{self, subjects};
use dorch_wadinfo::client::{MapAnalysis, ReadWad};
use owo_colors::OwoColorize;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

mod prompts {
    pub const ANALYZE_MAP: &str = include_str!("prompts/analyze_map.txt");
}

pub async fn run(args: args::MapArgs) -> Result<()> {
    dorch_common::metrics::maybe_spawn_metrics_server();
    let cancel = CancellationToken::new();
    let analyzer = Analyzer::new(
        prompts::ANALYZE_MAP.to_string(),
        args.model,
        args.openai_api_key,
        args.openai_base_url,
    );
    let nats = async_nats::connect_with_options(
        &args.nats.nats_url,
        ConnectOptions::new().user_and_password("app".to_string(), "devpass".to_string()),
    )
    .await
    .context("Failed to connect to NATS")?;
    let js = async_nats::jetstream::new(nats);
    let stream: Stream = js
        .get_stream(streams::MAP_ANALYSIS)
        .await
        .with_context(|| {
            format!(
                "Failed to get JetStream stream '{}' (did the bootstrap job run)?",
                streams::MAP_ANALYSIS
            )
        })?;
    let consumer = stream
        .get_or_create_consumer(
            "map_analyzer",
            pull::Config {
                durable_name: Some("map_analyzer".into()),
                filter_subjects: vec![subjects::analysis::map("*", "*")],
                ack_policy: AckPolicy::Explicit,
                ack_wait: std::time::Duration::from_secs(60),
                //max_deliver: args.nats.max_deliver,
                //num_replicas: args.nats.consumer_replicas,
                ..Default::default()
            },
        )
        .await
        .context("Failed to create JetStream consumer")?;
    println!(
        "{}{}",
        "🚀 Starting map analyzer • endpoint=".green(),
        args.endpoint.to_string().green().dimmed(),
    );
    let wadinfo = dorch_wadinfo::client::Client::new(args.endpoint);
    let locker = async_redis_lock::Locker::from_redis_url(args.redis.url().as_str()).await?;
    dorch_common::signal_ready();
    App::new(locker, analyzer, cancel, DeriveMap::new(wadinfo))
        .run(consumer)
        .await
}

#[derive(Clone, Deserialize)]
pub struct RawMapAnalysis {
    pub title: Option<String>,
    pub description: String,
    pub tags: Vec<String>,
}

pub struct DeriveMap {
    wadinfo: dorch_wadinfo::client::Client,
}

impl DeriveMap {
    pub fn new(wadinfo: dorch_wadinfo::client::Client) -> Self {
        Self { wadinfo }
    }
}

impl Worker<ReadWad, RawMapAnalysis> for DeriveMap {
    async fn derive_input(&self, _subject: &str, payload: &Bytes) -> Result<DeriveResult<ReadWad>> {
        let input: ReadWad = serde_json::from_slice(payload.as_ref())
            .context("Failed to deserialize map analysis input")?;
        let lock_key = format!(
            "l:w:{}:m:{}",
            input.meta.meta.id,
            input
                .maps
                .first()
                .as_ref()
                .map(|m| m.map.map.clone())
                .ok_or_else(|| anyhow!("No map found in ReadWad"))?,
        );
        Ok(DeriveResult::Ready(Work { input, lock_key }))
    }

    async fn post(&self, input: ReadWad, analysis: RawMapAnalysis) -> Result<()> {
        let analysis = MapAnalysis {
            wad_id: input.meta.meta.id,
            map_name: input
                .maps
                .first()
                .as_ref()
                .map(|m| m.map.map.clone())
                .ok_or_else(|| anyhow!("No map found in ReadWad"))?,
            map_title: analysis.title.filter(|t| !t.is_empty()),
            description: analysis.description,
            tags: analysis.tags,
        };
        self.wadinfo
            .post_map_analysis(analysis)
            .await
            .context("Failed to post map analysis")
    }
}
