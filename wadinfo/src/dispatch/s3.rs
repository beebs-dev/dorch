use anyhow::{Context, Result};
use aws_credential_types::{provider::SharedCredentialsProvider, Credentials};
use aws_types::region::Region;
use std::{collections::HashSet, env};
use uuid::Uuid;

use crate::args::S3PruneArgs;

fn parse_wad_id_from_prefix(prefix: &str) -> Option<Uuid> {
    let trimmed = prefix.trim_end_matches('/');
    let first = trimmed.split('/').next().unwrap_or(trimmed);
    Uuid::parse_str(first).ok()
}

fn parse_wad_id_from_key(key: &str) -> Option<Uuid> {
    let first = key.split('/').next()?;
    Uuid::parse_str(first).ok()
}

fn env_credentials() -> Result<SharedCredentialsProvider> {
    let access_key_id = env::var("AWS_ACCESS_KEY_ID").context("AWS_ACCESS_KEY_ID must be set")?;
    let secret_access_key =
        env::var("AWS_SECRET_ACCESS_KEY").context("AWS_SECRET_ACCESS_KEY must be set")?;
    let session_token = env::var("AWS_SESSION_TOKEN").ok();

    // Ensure creds come from env only.
    let creds = Credentials::new(
        access_key_id,
        secret_access_key,
        session_token,
        None,
        "env",
    );
    Ok(SharedCredentialsProvider::new(creds))
}

async fn load_shared_config(args: &S3PruneArgs) -> Result<aws_config::SdkConfig> {
    let creds_provider = env_credentials()?;
    let region = Region::new(args.s3_region.clone());
    Ok(aws_config::from_env()
        .region(region)
        .credentials_provider(creds_provider)
        .load()
        .await)
}

pub async fn list_wad_ids_in_bucket(args: &S3PruneArgs) -> Result<HashSet<Uuid>> {
    let shared_config = load_shared_config(args).await?;

    let mut s3_conf_builder = aws_sdk_s3::config::Builder::from(&shared_config);
    if let Some(ref endpoint) = args.s3_endpoint {
        // Common for MinIO / Spaces: path-style addressing.
        s3_conf_builder = s3_conf_builder
            .endpoint_url(endpoint)
            .force_path_style(true);
    }
    let s3_conf = s3_conf_builder.build();
    let client = aws_sdk_s3::Client::from_conf(s3_conf);

    // Preferred: use delimiter to enumerate folder-like prefixes.
    let mut wad_ids: HashSet<Uuid> = HashSet::new();
    let mut continuation: Option<String> = None;
    loop {
        let mut req = client
            .list_objects_v2()
            .bucket(&args.bucket)
            .delimiter("/");
        if let Some(ref token) = continuation {
            req = req.continuation_token(token);
        }

        let resp = req.send().await.context("S3 list_objects_v2 failed")?;

        for p in resp.common_prefixes() {
            if let Some(prefix) = p.prefix() {
                if let Some(wad_id) = parse_wad_id_from_prefix(prefix) {
                    wad_ids.insert(wad_id);
                }
            }
        }

        if resp.is_truncated().unwrap_or(false) {
            continuation = resp.next_continuation_token().map(|s| s.to_string());
        } else {
            break;
        }
    }

    // Fallback: some buckets may not have stable prefixes. If delimiter yielded nothing,
    // scan object keys and infer WAD IDs from the first path component.
    if wad_ids.is_empty() {
        let mut continuation: Option<String> = None;
        loop {
            let mut req = client.list_objects_v2().bucket(&args.bucket);
            if let Some(ref token) = continuation {
                req = req.continuation_token(token);
            }
            let resp = req.send().await.context("S3 list_objects_v2 fallback failed")?;
            for obj in resp.contents() {
                if let Some(key) = obj.key() {
                    if let Some(wad_id) = parse_wad_id_from_key(key) {
                        wad_ids.insert(wad_id);
                    }
                }
            }
            if resp.is_truncated().unwrap_or(false) {
                continuation = resp.next_continuation_token().map(|s| s.to_string());
            } else {
                break;
            }
        }
    }

    Ok(wad_ids)
}
