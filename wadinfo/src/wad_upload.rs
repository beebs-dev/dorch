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
use sha1::Sha1;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
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
    /// Returns (sha256_hash, sha1_hash, upload_id, file_size_bytes).
    pub async fn upload_draft(&self, filename: &str, data: &[u8]) -> Result<(String, String, Uuid, i64)> {
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

        // Compute SHA256 and SHA1 hashes
        let mut sha256_hasher = Sha256::new();
        let mut sha1_hasher = Sha1::new();
        sha256_hasher.update(data);
        sha1_hasher.update(data);
        let sha256_hash = hex::encode(sha256_hasher.finalize());
        let sha1_hash = hex::encode(sha1_hasher.finalize());

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

        Ok((sha256_hash, sha1_hash, upload_id, data.len() as i64))
    }

    /// Upload a WAD file to draft storage using streaming to avoid loading
    /// the entire file into memory. Uses S3 multipart upload with two long-lived
    /// tasks: one reads HTTP body, one uploads parts to S3.
    /// Returns (sha256_hash, sha1_hash, upload_id, file_size_bytes).
    pub async fn upload_draft_stream(
        &self,
        filename: &str,
        mut field: Field<'_>,
    ) -> Result<(String, String, Uuid, i64)> {
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
        let mut sha256_hasher = Sha256::new();
        let mut sha1_hasher = Sha1::new();
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

                    // Update hashes and size
                    sha256_hasher.update(&chunk);
                    sha1_hasher.update(&chunk);
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

        let sha256_hash = hex::encode(sha256_hasher.finalize());
        let sha1_hash = hex::encode(sha1_hasher.finalize());
        Ok((sha256_hash, sha1_hash, upload_id, total_size as i64))
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
        _sha256: &str,
        original_filename: &str,
    ) -> Result<String> {
        let draft_key = format!("{}{}", self.draft_key_prefix, upload_id);
        
        // Determine file extension from original filename
        let extension = SUPPORTED_EXTENSIONS
            .iter()
            .find(|ext| original_filename.to_lowercase().ends_with(*ext))
            .unwrap_or(&".wad");

        let permanent_key = format!("{}{}/{}{}", self.permanent_key_prefix, upload_id, original_filename, extension);

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

    /// Delete a WAD file from permanent storage by sha256 and original filename.
    pub async fn delete_permanent(&self, sha256: &str, original_filename: &str) -> Result<()> {
        // Determine file extension from original filename
        let extension = SUPPORTED_EXTENSIONS
            .iter()
            .find(|ext| original_filename.to_lowercase().ends_with(*ext))
            .unwrap_or(&".wad");

        let key = format!("{}{}{}", self.permanent_key_prefix, sha256, extension);
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .context("Failed to delete permanent WAD file")?;
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
/// This task waits for S3 multipart init to complete before receiving parts,
/// ensuring backpressure from the channel limits memory usage.
async fn upload_parts_task(
    client: Client,
    bucket: String,
    key: String,
    content_type: String,
    mut part_rx: mpsc::Receiver<(i32, Vec<u8>)>,
) -> Result<(String, Vec<CompletedPart>)> {
    // Initialize S3 multipart upload FIRST (blocks until ready)
    let response = client
        .create_multipart_upload()
        .bucket(&bucket)
        .key(&key)
        .content_type(content_type)
        .storage_class(StorageClass::Standard)
        .send()
        .await
        .context("Failed to create multipart upload")?;

    let s3_upload_id = response
        .upload_id()
        .ok_or_else(|| anyhow::anyhow!("No upload_id returned from S3"))?
        .to_string();

    let mut completed_parts: Vec<CompletedPart> = Vec::new();

    // Now receive and upload parts - channel backpressure limits memory
    while let Some((part_number, data)) = part_rx.recv().await {
        match upload_single_part(&client, &bucket, &key, &s3_upload_id, part_number, data).await {
            Ok(part) => completed_parts.push(part),
            Err(e) => {
                // Abort the multipart upload on error
                let _ = client
                    .abort_multipart_upload()
                    .bucket(&bucket)
                    .key(&key)
                    .upload_id(&s3_upload_id)
                    .send()
                    .await;
                return Err(e);
            }
        }
    }

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
