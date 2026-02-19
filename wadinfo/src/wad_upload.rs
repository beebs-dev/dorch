use anyhow::{Context, Result, bail};
use aws_config::BehaviorVersion;
use aws_credential_types::{Credentials, provider::SharedCredentialsProvider};
use aws_sdk_s3::{
    Client,
    primitives::ByteStream,
    types::{ObjectCannedAcl, StorageClass},
};
use aws_types::region::Region;
use reqwest::Url;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Maximum upload size: 100 MiB
pub const MAX_WAD_UPLOAD_BYTES: usize = 100 * 1024 * 1024;

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
