use anyhow::{Context, Result, bail};
use aws_config::BehaviorVersion;
use aws_credential_types::{Credentials, provider::SharedCredentialsProvider};
use aws_sdk_s3::{
    Client,
    primitives::ByteStream,
    types::{ObjectCannedAcl, StorageClass},
};
use aws_types::region::Region;
use image::{GenericImageView, ImageEncoder, codecs::webp::WebPEncoder, imageops::FilterType};
use reqwest::Url;
use uuid::Uuid;

pub const MAX_AVATAR_UPLOAD_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_AVATAR_DIMENSION_PX: u32 = 256;

#[derive(Clone)]
pub struct AvatarStore {
    client: Client,
    bucket: String,
    endpoint: String,
    key_prefix: String,
}

impl AvatarStore {
    pub async fn new(
        access_key_id: String,
        secret_access_key: String,
        bucket: String,
        region: String,
        endpoint: String,
        key_prefix: String,
    ) -> Result<Self> {
        let creds = Credentials::new(access_key_id, secret_access_key, None, None, "avatar_s3");
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
            key_prefix: normalize_key_prefix(&key_prefix),
        })
    }

    pub async fn upload_avatar(&self, user_id: Uuid, input: &[u8]) -> Result<String> {
        if input.len() > MAX_AVATAR_UPLOAD_BYTES {
            bail!(
                "Avatar exceeds max upload size of {} bytes",
                MAX_AVATAR_UPLOAD_BYTES
            );
        }
        let webp = convert_avatar_to_webp(input)?;
        let key = format!("{}{}.webp", self.key_prefix, user_id);

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .acl(ObjectCannedAcl::PublicRead)
            .content_type("image/webp")
            .cache_control("public,max-age=86400")
            .storage_class(StorageClass::Standard)
            .body(ByteStream::from(webp))
            .send()
            .await
            .context("Failed to upload avatar image to S3")?;

        self.public_url(&key)
    }

    fn public_url(&self, key: &str) -> Result<String> {
        let endpoint = Url::parse(&self.endpoint).context("Invalid avatar S3 endpoint URL")?;
        let host = endpoint
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("avatar S3 endpoint missing host"))?;
        let scheme = endpoint.scheme();
        Ok(format!("{}://{}.{}{}{}", scheme, self.bucket, host, "/", key))
    }
}

fn normalize_key_prefix(key_prefix: &str) -> String {
    let mut prefix = key_prefix.trim().trim_matches('/').to_string();
    if !prefix.is_empty() {
        prefix.push('/');
    }
    prefix
}

fn convert_avatar_to_webp(input: &[u8]) -> Result<Vec<u8>> {
    let image = image::load_from_memory(input).context("Failed to decode avatar image")?;
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        bail!("Avatar image has invalid dimensions");
    }

    let resized = if width > MAX_AVATAR_DIMENSION_PX || height > MAX_AVATAR_DIMENSION_PX {
        image.resize(
            MAX_AVATAR_DIMENSION_PX,
            MAX_AVATAR_DIMENSION_PX,
            FilterType::Lanczos3,
        )
    } else {
        image
    };

    let rgba = resized.to_rgba8();
    let (out_width, out_height) = rgba.dimensions();
    let mut out = Vec::new();
    let encoder = WebPEncoder::new_lossless(&mut out);
    encoder
        .write_image(
            rgba.as_raw(),
            out_width,
            out_height,
            image::ExtendedColorType::Rgba8,
        )
        .context("Failed to encode avatar as webp")?;

    Ok(out)
}
