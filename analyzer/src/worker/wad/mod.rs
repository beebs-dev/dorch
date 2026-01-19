use std::time::Duration;

use crate::{
    args,
    worker::{
        analyzer::Analyzer,
        app::{App, DeriveResult, Work, Worker},
        optimize_readwad,
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
    tokio::spawn({
        let cancel = cancel.clone();
        async move {
            dorch_common::shutdown::shutdown_signal().await;
            cancel.cancel();
        }
    });
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
                max_deliver: 9999999999,
                num_replicas: 1,
                ..Default::default()
            },
        )
        .await
        .context("Failed to create JetStream consumer")?;
    let wadinfo = dorch_wadinfo::client::Client::new(args.wadinfo_endpoint);
    let locker = async_redis_lock::Locker::from_redis_url(args.redis.url().as_str())
        .await
        .context("Failed to create Redis locker")?;
    dorch_common::signal_ready();
    println!("{}", "🚀 Starting wad analyzer".green());
    App::new(analyzer, cancel, DeriveWad::new(wadinfo, js, locker))
        .run(consumer)
        .await
}

#[derive(Clone, Deserialize)]
pub struct RawWadAnalysis {
    pub title: Option<String>,
    pub description: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Deserialize)]
pub struct WadContext {
    pub wad_id: Uuid,
}

pub struct DeriveWad {
    wadinfo: dorch_wadinfo::client::Client,
    js: async_nats::jetstream::Context,
    locker: async_redis_lock::Locker,
}

impl DeriveWad {
    pub fn new(
        wadinfo: dorch_wadinfo::client::Client,
        js: async_nats::jetstream::Context,
        locker: async_redis_lock::Locker,
    ) -> Self {
        Self {
            wadinfo,
            js,
            locker,
        }
    }
}

impl Worker<ReadWad, RawWadAnalysis, WadContext> for DeriveWad {
    async fn derive_input(
        &self,
        _subject: &str,
        payload: &Bytes,
    ) -> Result<DeriveResult<ReadWad, WadContext>> {
        let wad_id = Uuid::parse_str(
            std::str::from_utf8(payload.as_ref()).context("Invalid UTF-8 in payload")?,
        )
        .context("Failed to parse wad ID from payload")?;
        if wad_id.is_nil() {
            return Ok(DeriveResult::Discard);
        }
        let lock_key = format!("l:w:{}", wad_id);
        let lock = self
            .locker
            .clone()
            .acquire(&lock_key)
            .await
            .context("Failed to acquire lock")?;
        let mut input = self
            .wadinfo
            .get_wad(wad_id)
            .await
            .context("Failed to get wad")?
            .ok_or_else(|| anyhow!("Wad not found: {}", wad_id))?;
        if input.meta.analysis.is_some() {
            println!(
                "{}{}",
                "ℹ️  Skipping WAD that has already been analyzed • wad_id=".blue(),
                wad_id.blue().dimmed(),
            );
            return Ok(DeriveResult::Discard);
        }
        optimize_readwad(&mut input);
        let wad_title = input.meta.meta.title.as_deref().unwrap_or("<untitled>");
        if input.maps.is_empty() {
            input.meta.meta.id = Uuid::nil();
            println!(
                "{}{}{}{}{}{}",
                "ℹ️  Analyzing WAD • wad_id=".blue(),
                wad_id.blue().dimmed(),
                " • map_count=".blue(),
                0.blue().dimmed(),
                " • title=".blue(),
                wad_title.blue().dimmed(),
            );
            Ok(DeriveResult::Ready(Work {
                input,
                lock: Some(lock),
                context: WadContext { wad_id },
            }))
        } else {
            let analyzed_map_count = input.maps.iter().filter(|m| m.analysis.is_some()).count();
            if analyzed_map_count == input.maps.len() {
                input.meta.meta.id = Uuid::nil();
                println!(
                    "{}{}{}{}{}{}",
                    "ℹ️ Analyzing WAD • wad_id=".blue(),
                    wad_id.blue().dimmed(),
                    " • map_count=".blue(),
                    input.maps.len().blue().dimmed(),
                    " • title=".blue(),
                    wad_title.blue().dimmed(),
                );
                Ok(DeriveResult::Ready(Work {
                    input,
                    lock: Some(lock),
                    context: WadContext { wad_id },
                }))
            } else {
                let missing_maps = input
                    .maps
                    .iter()
                    .filter(|m| m.analysis.is_none())
                    .map(|m| m.map.map.clone())
                    .collect::<Vec<_>>();
                println!(
                    "{}{}{}{}{}{}",
                    "ℹ️ Requesting analyses for missing maps • wad_id=".blue(),
                    wad_id.blue().dimmed(),
                    " • missing_count=".blue(),
                    missing_maps.len().blue().dimmed(),
                    " • missing_maps=".blue(),
                    format!("{:?}", missing_maps).blue().dimmed(),
                );
                for map in missing_maps {
                    let subject = subjects::analysis::map(wad_id, &map);
                    let mut map_input = input.clone();
                    map_input.meta.meta.content.counts = None;
                    map_input.meta.meta.content.maps = None;
                    map_input.maps.retain(|m| m.map.map == map);
                    map_input.maps.first_mut().unwrap().analysis = None;
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
                    retry_after: Some(Duration::from_mins(60)),
                })
            }
        }
    }

    async fn post(&self, context: WadContext, analysis: RawWadAnalysis) -> Result<()> {
        let analysis = WadAnalysis {
            wad_id: context.wad_id,
            title: analysis.title.filter(|t| !t.is_empty()),
            description: analysis.description,
            tags: analysis.tags,
        };
        println!(
            "{}{}{}{}{}{}{}{}{}",
            "✅ Completed WAD analysis • wad_id=".green(),
            context.wad_id.green().dimmed(),
            " • title=".green(),
            analysis
                .title
                .as_ref()
                .unwrap_or(&"<untitled>".to_string())
                .green()
                .dimmed(),
            " • description=".green(),
            analysis.description.green().dimmed(),
            " • tags=[".green(),
            analysis.tags.join(", ").green().dimmed(),
            "]".green()
        );
        self.wadinfo
            .post_wad_analysis(analysis)
            .await
            .context("Failed to post wad analysis")
    }
}
