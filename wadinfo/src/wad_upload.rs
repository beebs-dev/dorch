use anyhow::{Context, Result, bail};
use aws_config::BehaviorVersion;
use aws_credential_types::{Credentials, provider::SharedCredentialsProvider};
use aws_sdk_s3::{
    Client,
    primitives::ByteStream,
    types::{CompletedMultipartUpload, CompletedPart, ObjectCannedAcl, StorageClass},
};
use aws_types::region::Region;
use axum::extract::multipart::Field;
use reqwest::Url;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use uuid::Uuid;

/// Minimum part size for S3 multipart upload (5 MiB)
const MIN_PART_SIZE: usize = 5 * 1024 * 1024;

/// Channel buffer size - allows reader to stay ahead of uploader
const PART_CHANNEL_SIZE: usize = 4;

/// Maximum upload size: 1000 MiB
pub const MAX_WAD_UPLOAD_BYTES: usize = 1000 * 1024 * 1024;

/// Supported file extensions for WAD uploads
pub const SUPPORTED_EXTENSIONS: &[&str] = &[".wad", ".pk3", ".wad.gz", ".pk3.gz"];

#[derive(Clone)]
pub struct WadUploadStore {
    client: Client,
    bucket: String,
    endpoint: String,
    draft_key_prefix: String,
    permanent_key_prefix: String,
}

impl WadUploadStore {
    pub async fn new(
        access_key_id: String,
        secret_access_key: String,
        bucket: String,
        region: String,
        endpoint: String,
        draft_key_prefix: String,
        permanent_key_prefix: String,
    ) -> Result<Self> {
        let creds = Credentials::new(access_key_id, secret_access_key, None, None, "wad_upload_s3");
        let shared_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(region))
            .credentials_provider(SharedCredentialsProvider::new(creds))
            .load()
            .await;

        let s3_conf = aws_sdk_s3::config::Builder::from(&shared_config)
            .endpoint_url(endpoint.clone())
            .force_path_style(true)
            .build();

        Ok(Self {
            client: Client::from_conf(s3_conf),
            bucket,
            endpoint,
            draft_key_prefix: normalize_key_prefix(&draft_key_prefix),
            permanent_key_prefix: normalize_key_prefix(&permanent_key_prefix),
        })
    }

    /// Upload a WAD file to draft storage.
    /// Returns (sha256_hash, upload_id, file_size_bytes).
    pub async fn upload_draft(&self, filename: &str, data: &[u8]) -> Result<(String, Uuid, i64)> {
        if data.len() > MAX_WAD_UPLOAD_BYTES {
            bail!(
                "File exceeds max upload size of {} bytes",
                MAX_WAD_UPLOAD_BYTES
            );
        }

        // Validate file extension
        let lower_filename = filename.to_lowercase();
        let has_valid_extension = SUPPORTED_EXTENSIONS
            .iter()
            .any(|ext| lower_filename.ends_with(ext));
        if !has_valid_extension {
            bail!(
                "Unsupported file extension. Supported: {}",
                SUPPORTED_EXTENSIONS.join(", ")
            );
        }

        // Compute SHA256 hash
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hex::encode(hasher.finalize());

        // Generate upload ID
        let upload_id = Uuid::new_v4();

        // Determine content type based on extension
        let content_type = if lower_filename.ends_with(".gz") {
            "application/gzip"
        } else if lower_filename.ends_with(".pk3") {
            "application/zip"
        } else {
            "application/octet-stream"
        };

        // Store with upload_id as the key
        let key = format!("{}{}", self.draft_key_prefix, upload_id);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .content_type(content_type)
            .storage_class(StorageClass::Standard)
            .body(ByteStream::from(data.to_vec()))
            .send()
            .await
            .context("Failed to upload WAD file to S3")?;

        Ok((hash, upload_id, data.len() as i64))
    }

    /// Upload a WAD file to draft storage using streaming to avoid loading
    /// the entire file into memory. Uses S3 multipart upload with two long-lived
    /// tasks: one reads HTTP body, one uploads parts to S3.
    /// Returns (sha256_hash, upload_id, file_size_bytes).
    pub async fn upload_draft_stream(
        &self,
        filename: &str,
        mut field: Field<'_>,
    ) -> Result<(String, Uuid, i64)> {
        // Validate file extension
        let lower_filename = filename.to_lowercase();
        let has_valid_extension = SUPPORTED_EXTENSIONS
            .iter()
            .any(|ext| lower_filename.ends_with(ext));
        if !has_valid_extension {
            bail!(
                "Unsupported file extension. Supported: {}",
                SUPPORTED_EXTENSIONS.join(", ")
            );
        }

        // Determine content type based on extension
        let content_type = if lower_filename.ends_with(".gz") {
            "application/gzip"
        } else if lower_filename.ends_with(".pk3") {
            "application/zip"
        } else {
            "application/octet-stream"
        }
        .to_string();

        // Generate upload ID
        let upload_id = Uuid::new_v4();
        let key = format!("{}{}", self.draft_key_prefix, upload_id);

        // Create bounded channel for parts: reader sends, uploader receives
        let (part_tx, part_rx) = mpsc::channel::<(i32, Vec<u8>)>(PART_CHANNEL_SIZE);

        // Spawn S3 initialization + uploader task
        let client = self.client.clone();
        let bucket = self.bucket.clone();
        let key_clone = key.clone();
        let uploader_handle = tokio::spawn(async move {
            upload_parts_task(client, bucket, key_clone, content_type, part_rx).await
        });

        // Main task: read HTTP body chunks, compute hash, send parts to uploader
        let mut hasher = Sha256::new();
        let mut total_size: usize = 0;
        let mut part_number: i32 = 1;
        let mut buffer: Vec<u8> = Vec::with_capacity(MIN_PART_SIZE);
        let mut read_error: Option<String> = None;
        let mut size_exceeded = false;
        let mut send_failed = false;

        let mut chunk_count = 0;
        loop {
            match field.chunk().await {
                Ok(Some(chunk)) => {
                    chunk_count += 1;
                    if chunk_count <= 3 || chunk_count % 100 == 0 {
                        eprintln!("📦 chunk #{}: {} bytes, total_size={}", chunk_count, chunk.len(), total_size + chunk.len());
                    }
                    
                    // Check size limit
                    if total_size + chunk.len() > MAX_WAD_UPLOAD_BYTES {
                        size_exceeded = true;
                        break;
                    }

                    // Update hash and size
                    hasher.update(&chunk);
                    total_size += chunk.len();
                    buffer.extend_from_slice(&chunk);

                    // Send part when buffer is full
                    if buffer.len() >= MIN_PART_SIZE {
                        let part_data =
                            std::mem::replace(&mut buffer, Vec::with_capacity(MIN_PART_SIZE));
                        if part_tx.send((part_number, part_data)).await.is_err() {
                            send_failed = true;
                            break;
                        }
                        part_number += 1;
                    }
                }
                Ok(None) => {
                    eprintln!("📦 stream complete: {} chunks, {} bytes total", chunk_count, total_size);
                    break;
                }
                Err(e) => {
                    eprintln!("📦 chunk error after {} chunks, {} bytes: {:?}", chunk_count, total_size, e);
                    read_error = Some(format!("{:?}", e));
                    break;
                }
            }
        }

        // Send final part if there's remaining data and no errors
        if read_error.is_none() && !size_exceeded && !send_failed && !buffer.is_empty() {
            let _ = part_tx.send((part_number, buffer)).await;
        }

        // Drop sender to signal uploader task that we're done
        drop(part_tx);

        // Wait for uploader task to complete
        let upload_result = uploader_handle
            .await
            .context("Uploader task panicked")?;

        // Get S3 upload ID for potential abort
        let (s3_upload_id, completed_parts) = match upload_result {
            Ok(result) => result,
            Err(e) => {
                // Uploader already handles its own abort on error
                return Err(e);
            }
        };

        // Handle reader errors - abort multipart upload
        if size_exceeded {
            self.abort_multipart(&key, &s3_upload_id).await;
            bail!(
                "File exceeds max upload size of {} bytes",
                MAX_WAD_UPLOAD_BYTES
            );
        }

        if let Some(err) = read_error {
            self.abort_multipart(&key, &s3_upload_id).await;
            bail!("Failed to read chunk from upload: {}", err);
        }

        // Handle empty upload
        if completed_parts.is_empty() && total_size == 0 {
            self.abort_multipart(&key, &s3_upload_id).await;
            bail!("Cannot upload empty file");
        }

        // Complete the multipart upload
        let completed_upload = CompletedMultipartUpload::builder()
            .set_parts(Some(completed_parts))
            .build();

        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(&key)
            .upload_id(&s3_upload_id)
            .multipart_upload(completed_upload)
            .send()
            .await
            .context("Failed to complete multipart upload")?;

        let hash = hex::encode(hasher.finalize());
        Ok((hash, upload_id, total_size as i64))
    }

    /// Abort a multipart upload (best effort, ignores errors)
    async fn abort_multipart(&self, key: &str, upload_id: &str) {
        let _ = self
            .client
            .abort_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .upload_id(upload_id)
            .send()
            .await;
    }

    /// Move a file from draft storage to permanent storage.
    /// The permanent key will be based on the sha256 hash.
    pub async fn publish_draft(
        &self,
        upload_id: Uuid,
        sha256: &str,
        original_filename: &str,
    ) -> Result<String> {
        let draft_key = format!("{}{}", self.draft_key_prefix, upload_id);
        
        // Determine file extension from original filename
        let extension = SUPPORTED_EXTENSIONS
            .iter()
            .find(|ext| original_filename.to_lowercase().ends_with(*ext))
            .unwrap_or(&".wad");

        let permanent_key = format!("{}{}{}", self.permanent_key_prefix, sha256, extension);

        // Copy from draft to permanent location
        let copy_source = format!("{}/{}", self.bucket, draft_key);
        self.client
            .copy_object()
            .bucket(&self.bucket)
            .key(&permanent_key)
            .copy_source(&copy_source)
            .acl(ObjectCannedAcl::PublicRead)
            .storage_class(StorageClass::Standard)
            .send()
            .await
            .context("Failed to copy WAD file to permanent storage")?;

        // Delete the draft file
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&draft_key)
            .send()
            .await
            .context("Failed to delete draft WAD file")?;

        self.public_url(&permanent_key)
    }

    /// Delete a draft file from storage.
    pub async fn delete_draft(&self, upload_id: Uuid) -> Result<()> {
        let key = format!("{}{}", self.draft_key_prefix, upload_id);
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .context("Failed to delete draft WAD file")?;
        Ok(())
    }

    fn public_url(&self, key: &str) -> Result<String> {
        let endpoint = Url::parse(&self.endpoint).context("Invalid WAD S3 endpoint URL")?;
        let host = endpoint
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("WAD S3 endpoint missing host"))?;
        let scheme = endpoint.scheme();
        Ok(format!(
            "{}://{}.{}{}{}",
            scheme, self.bucket, host, "/", key
        ))
    }
}

fn normalize_key_prefix(key_prefix: &str) -> String {
    let mut prefix = key_prefix.trim().trim_matches('/').to_string();
    if !prefix.is_empty() {
        prefix.push('/');
    }
    prefix
}

/// Long-lived task that initializes S3 multipart upload and uploads parts
/// as they arrive via the channel. Returns (s3_upload_id, completed_parts).
/// 
/// CRITICAL: This task starts receiving from the channel IMMEDIATELY,
/// buffering parts while S3 initialization happens concurrently.
/// This prevents blocking the HTTP body reader.
async fn upload_parts_task(
    client: Client,
    bucket: String,
    key: String,
    content_type: String,
    mut part_rx: mpsc::Receiver<(i32, Vec<u8>)>,
) -> Result<(String, Vec<CompletedPart>)> {
    // Start S3 multipart upload initialization concurrently
    let client_for_init = client.clone();
    let bucket_for_init = bucket.clone();
    let key_for_init = key.clone();
    let (init_tx, init_rx) = oneshot::channel();
    
    tokio::spawn(async move {
        let result = client_for_init
            .create_multipart_upload()
            .bucket(&bucket_for_init)
            .key(&key_for_init)
            .content_type(content_type)
            .storage_class(StorageClass::Standard)
            .send()
            .await
            .context("Failed to create multipart upload");
        let _ = init_tx.send(result);
    });

    // Buffer parts while waiting for S3 init
    let mut buffered_parts: Vec<(i32, Vec<u8>)> = Vec::new();
    let mut s3_upload_id: Option<String> = None;
    let mut completed_parts: Vec<CompletedPart> = Vec::new();
    let mut init_rx = Some(init_rx);

    // Process parts as they arrive, buffering until S3 is ready
    loop {
        tokio::select! {
            biased;
            
            // Check if S3 init completed (only if we haven't already)
            init_result = async { 
                match init_rx.as_mut() {
                    Some(rx) => rx.await.ok(),
                    None => std::future::pending().await,
                }
            } => {
                if let Some(result) = init_result {
                    match result {
                        Ok(response) => {
                            let upload_id = response
                                .upload_id()
                                .ok_or_else(|| anyhow::anyhow!("No upload_id returned from S3"))?
                                .to_string();
                            s3_upload_id = Some(upload_id.clone());
                            init_rx = None; // Don't check again
                            
                            // Upload all buffered parts
                            for (part_number, data) in buffered_parts.drain(..) {
                                let part = upload_single_part(&client, &bucket, &key, &upload_id, part_number, data).await?;
                                completed_parts.push(part);
                            }
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
            
            // Receive parts from channel (always ready to receive)
            part = part_rx.recv() => {
                match part {
                    Some((part_number, data)) => {
                        if let Some(ref upload_id) = s3_upload_id {
                            // S3 is ready, upload directly
                            match upload_single_part(&client, &bucket, &key, upload_id, part_number, data).await {
                                Ok(part) => completed_parts.push(part),
                                Err(e) => {
                                    // Abort the multipart upload on error
                                    let _ = client
                                        .abort_multipart_upload()
                                        .bucket(&bucket)
                                        .key(&key)
                                        .upload_id(upload_id)
                                        .send()
                                        .await;
                                    return Err(e);
                                }
                            }
                        } else {
                            // S3 not ready yet, buffer the part
                            buffered_parts.push((part_number, data));
                        }
                    }
                    None => {
                        // Channel closed, no more parts coming
                        break;
                    }
                }
            }
        }
    }

    // If S3 init hasn't completed yet, wait for it
    if s3_upload_id.is_none() {
        if let Some(rx) = init_rx {
            match rx.await {
                Ok(Ok(response)) => {
                    let upload_id = response
                        .upload_id()
                        .ok_or_else(|| anyhow::anyhow!("No upload_id returned from S3"))?
                        .to_string();
                    s3_upload_id = Some(upload_id.clone());
                    
                    // Upload remaining buffered parts
                    for (part_number, data) in buffered_parts.drain(..) {
                        let part = upload_single_part(&client, &bucket, &key, &upload_id, part_number, data).await?;
                        completed_parts.push(part);
                    }
                }
                Ok(Err(e)) => return Err(e),
                Err(_) => bail!("S3 init task was cancelled"),
            }
        }
    }

    let s3_upload_id = s3_upload_id.ok_or_else(|| anyhow::anyhow!("S3 upload was never initialized"))?;
    
    Ok((s3_upload_id, completed_parts))
}

/// Upload a single part to S3 and return the CompletedPart.
async fn upload_single_part(
    client: &Client,
    bucket: &str,
    key: &str,
    upload_id: &str,
    part_number: i32,
    data: Vec<u8>,
) -> Result<CompletedPart> {
    let response = client
        .upload_part()
        .bucket(bucket)
        .key(key)
        .upload_id(upload_id)
        .part_number(part_number)
        .body(ByteStream::from(data))
        .send()
        .await
        .context("Failed to upload part to S3")?;

    let etag = response
        .e_tag()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("No ETag returned from upload_part"))?;

    Ok(CompletedPart::builder()
        .e_tag(etag)
        .part_number(part_number)
        .build())
}
