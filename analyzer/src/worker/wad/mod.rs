use std::time::Duration;

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
use dorch_wadinfo::client::{ReadWad, WadAnalysis};
use owo_colors::OwoColorize;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

mod prompts {
    pub const ANALYZE_WAD: &str = include_str!("prompts/analyze_wad.txt");
}

pub async fn run(args: args::WadArgs) -> Result<()> {
    dorch_common::metrics::maybe_spawn_metrics_server();
    let cancel = CancellationToken::new();
    let analyzer = Analyzer::new(
        prompts::ANALYZE_WAD.to_string(),
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
        .get_stream(streams::WAD_ANALYSIS)
        .await
        .with_context(|| {
            format!(
                "Failed to get JetStream stream '{}' (did the bootstrap job run)?",
                streams::WAD_ANALYSIS
            )
        })?;
    let consumer = stream
        .get_or_create_consumer(
            "wad_analyzer",
            pull::Config {
                durable_name: Some("wad_analyzer".into()),
                filter_subjects: vec![subjects::analysis::wad("*")],
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
        "🚀 Starting wad analyzer • endpoint=".green(),
        args.endpoint.to_string().green().dimmed(),
    );
    let wadinfo = dorch_wadinfo::client::Client::new(args.endpoint);
    let locker = async_redis_lock::Locker::from_redis_url(args.redis.url().as_str()).await?;
    dorch_common::signal_ready();
    App::new(locker, analyzer, cancel, DeriveWad::new(wadinfo, js))
        .run(consumer)
        .await
}

#[derive(Clone, Deserialize)]
pub struct RawWadAnalysis {
    pub title: Option<String>,
    pub description: String,
    pub tags: Vec<String>,
}

pub struct DeriveWad {
    wadinfo: dorch_wadinfo::client::Client,
    js: async_nats::jetstream::Context,
}

impl DeriveWad {
    pub fn new(wadinfo: dorch_wadinfo::client::Client, js: async_nats::jetstream::Context) -> Self {
        Self { wadinfo, js }
    }
}

impl Worker<ReadWad, RawWadAnalysis> for DeriveWad {
    async fn derive_input(&self, _subject: &str, payload: &Bytes) -> Result<DeriveResult<ReadWad>> {
        let wad_id = Uuid::parse_str(
            std::str::from_utf8(payload.as_ref()).context("Invalid UTF-8 in payload")?,
        )
        .context("Failed to parse wad ID from payload")?;
        let input = self
            .wadinfo
            .get_wad(wad_id)
            .await
            .context("Failed to get wad")?
            .ok_or_else(|| anyhow!("Wad not found: {}", wad_id))?;
        let lock_key = format!("l:w:{}", wad_id);
        if input.maps.is_empty() {
            Ok(DeriveResult::Ready(Work { input, lock_key }))
        } else {
            // Ensure the map analyses are done.
            let map_analysis = self
                .wadinfo
                .list_map_analyses(wad_id)
                .await
                .context("Failed to list map analyses")?;
            if map_analysis.len() == input.maps.len() {
                Ok(DeriveResult::Ready(Work { input, lock_key }))
            } else {
                // Request the missing maps.
                let missing_maps = input
                    .maps
                    .iter()
                    .filter_map(|m| {
                        let map_name = &m.map.map;
                        if !map_analysis.iter().any(|ma| &ma.map_name == map_name) {
                            Some(map_name.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                for map in missing_maps {
                    let subject = subjects::analysis::map(wad_id, &map);
                    let mut map_input = input.clone();
                    map_input.maps.retain(|m| m.map.map == map);
                    let payload = Bytes::from(serde_json::to_vec(&map_input)?);
                    _ = self
                        .js
                        .publish_with_headers(
                            subject,
                            {
                                let mut headers = async_nats::HeaderMap::new();
                                headers.insert(
                                    async_nats::header::NATS_MESSAGE_ID,
                                    format!("wad-{}-map-{}", wad_id, map),
                                );
                                headers
                            },
                            payload,
                        )
                        .await
                        .context("Failed to publish map analysis message")?;
                }
                Ok(DeriveResult::Pending {
                    retry_after: Some(Duration::from_mins(10)),
                })
            }
        }
    }

    async fn post(&self, input: ReadWad, analysis: RawWadAnalysis) -> Result<()> {
        let analysis = WadAnalysis {
            wad_id: input.meta.meta.id,
            title: analysis.title.filter(|t| !t.is_empty()),
            description: analysis.description,
            tags: analysis.tags,
        };
        self.wadinfo
            .post_wad_analysis(analysis)
            .await
            .context("Failed to post wad analysis")
    }
}
