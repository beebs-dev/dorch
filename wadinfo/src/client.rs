use std::time::Duration;

use anyhow::{Context, Result, bail};
use dorch_common::{
    Pagination,
    types::wad::{MapStat, ReadWadMeta, TextFile},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserProfileFull {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_color: Option<i32>,
    pub registered_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_active_at: Option<i64>,
    pub privacy_hide_activity: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserProfilePublic {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_color: Option<i32>,
    pub registered_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_active_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum UserProfileView {
    Full(UserProfileFull),
    Public(UserProfilePublic),
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PutUserProfileRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_color: Option<Option<i32>>,
    /// Only used during profile creation. Ignored on updates (username is immutable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_hide_activity: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MapReference {
    pub wad_id: Uuid,
    pub map: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolveMapThumbnailsRequest {
    pub items: Vec<MapReference>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolveWadDownloadsRequest {
    pub wad_ids: Vec<Uuid>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolveWadDownloadsResponse {
    pub items: Vec<ResolvedWadDownload>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolvedWadDownload {
    pub wad_id: Uuid,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MapThumbnail {
    pub wad_id: Uuid,
    pub map: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolveMapThumbnailsResponse {
    pub items: Vec<MapThumbnail>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WadImage {
    /// Optional on PUT; always present on GET.
    #[serde(default)]
    pub id: Option<Uuid>,

    pub url: String,

    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

/// Read-only view of a map, including image metadata.
///
/// The underlying per-map schema is stored as JSONB for speed. We add `images` at query time
/// (Postgres JSONB composition) so read endpoints can return a richer shape without changing the
/// insert/upsert payload types.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReadMapStat {
    #[serde(flatten)]
    pub map: MapStat,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<WadImage>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<AbridgedMapAnalysis>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReadWadMetaWithTextFiles {
    #[serde(flatten)]
    pub meta: ReadWadMeta,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<AbridgedWadAnalysis>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_files: Option<Vec<TextFile>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReadWad {
    pub meta: ReadWadMetaWithTextFiles,
    pub maps: Vec<ReadMapStat>,
    /// User who uploaded/published this WAD (None for imported WADs)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uploader_id: Option<Uuid>,
    /// User-provided description (None for imported WADs or if not set)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WadMapImages {
    pub map: String,
    pub items: Vec<WadImage>,
}

#[derive(Deserialize)]
pub struct SearchOptions {
    #[serde(flatten)]
    pub pagination: Pagination,

    #[serde(rename = "q")]
    pub query: String,
}

#[derive(Deserialize)]
pub struct ResolveWadURLsRequest {
    pub wad_ids: Vec<Uuid>,
}

#[derive(Serialize, Deserialize)]
pub struct GetWadMetasRequest {
    pub wad_ids: Vec<Uuid>,
}

#[derive(Serialize, Deserialize)]
pub struct GetWadMetasResponse {
    pub items: Vec<ReadWadMeta>,
}

#[derive(Serialize, Deserialize)]
pub struct ResolvedWadURL {
    pub wad_id: Uuid,
    pub url: String,
}

#[derive(Serialize, Deserialize)]
pub struct ResolveWadURLsResponse {
    pub items: Vec<ResolvedWadURL>,
}

#[derive(Deserialize)]
pub struct ListWadsRequest {
    #[serde(flatten)]
    pub pagination: Pagination,

    /// If true, sort descending. Otherwise, sort ascending.
    #[serde(rename = "d", default)]
    pub sort_desc: bool,
}

#[derive(Serialize)]
pub struct GetWadMapResponse {
    #[serde(flatten)]
    pub map: ReadMapStat,
    pub wad_meta: ReadWadMeta,
}

#[derive(Deserialize)]
pub struct WadSearchRequest {
    pub query: String,

    #[serde(default)]
    pub offset: i64,

    #[serde(default)]
    pub limit: Option<i64>,
    // TODO: add filters, sorting, etc.
}

#[derive(Serialize, Deserialize)]
pub struct WadSearchResults {
    pub request_id: Uuid,
    pub query: String,
    pub items: Vec<ReadWadMeta>,
    pub full_count: i64,
    pub offset: i64,
    pub limit: i64,
    pub truncated: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ListWadsResponse {
    pub items: Vec<ReadWadMeta>,
    pub full_count: i64,
    pub offset: i64,
    pub limit: i64,
    pub truncated: bool,
}

#[derive(Serialize, Deserialize)]
pub struct FeaturedWadViewItem {
    pub wad: ReadWadMeta,

    /// Non-panorama images (flattened across all maps).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<WadImage>,
}

#[derive(Serialize, Deserialize)]
pub struct FeaturedViewResponse {
    /// The main WAD list for the WAD browser.
    pub results: ListWadsResponse,

    /// Featured items with pre-resolved non-panorama thumbnails.
    pub featured: Vec<FeaturedWadViewItem>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WadAnalysis {
    pub wad_id: Uuid,
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub origin: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AbridgedWadAnalysis {
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub origin: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AbridgedMapAnalysis {
    pub title: Option<String>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MapAnalysis {
    #[serde(default, skip_serializing_if = "Uuid::is_nil")]
    pub wad_id: Uuid,
    pub map_name: String,
    pub map_title: Option<String>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub origin: String,
}

// ----------------------------
// WAD Draft Types
// ----------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WadDraft {
    pub draft_id: Uuid,
    pub uploader_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub ai_enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wad_id: Option<Uuid>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UpdateDraftRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_sha1: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ListDraftsResponse {
    pub items: Vec<WadDraft>,
}

/// A WAD owned by a user (published)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserWad {
    pub wad_id: Uuid,
    pub sha1: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_size_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_url: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ListUserWadsResponse {
    pub items: Vec<UserWad>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UpdateWadRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UploadResponse {
    pub hash: String,
    pub sha1: String,
    pub id: Uuid,
    pub size: i64,
}

#[derive(Clone)]
pub struct Client {
    inner: reqwest::Client,
    base_url: String,
}

impl Client {
    pub fn new(base_url: String) -> Self {
        let inner = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap();
        Self { inner, base_url }
    }

    pub async fn map_analysis_exists(&self, wad_id: Uuid, map_name: &str) -> Result<bool> {
        let url = format!("{}/wad/{}/map/{}/analysis", self.base_url, wad_id, map_name);
        let resp = self
            .inner
            .get(&url)
            .send()
            .await
            .context("Failed to send map_analysis_exists request")?;
        match resp.status() {
            reqwest::StatusCode::NOT_FOUND => Ok(false),
            status if !status.is_success() => {
                bail!(
                    "map_analysis_exists request failed with status {}: {}",
                    status,
                    resp.text().await.unwrap_or_default()
                );
            }
            _ => Ok(true),
        }
    }

    pub async fn list_map_analyses(&self, wad_id: Uuid) -> Result<Vec<MapAnalysis>> {
        self.inner
            .get(format!("{}/wad/{}/map_analyses", self.base_url, wad_id))
            .send()
            .await
            .context("Failed to send list_map_analyses request")?
            .error_for_status()
            .context("list_map_analyses request returned error status")?
            .json::<Vec<MapAnalysis>>()
            .await
            .context("Failed to parse list_map_analyses response")
    }

    pub async fn post_wad_analysis(&self, analysis: WadAnalysis) -> Result<()> {
        let url = format!("{}/wad/{}/analysis", self.base_url, analysis.wad_id);
        let resp = self
            .inner
            .post(&url)
            .json(&analysis)
            .send()
            .await
            .context("Failed to send post_wad_analysis request")?;
        if !resp.status().is_success() {
            bail!(
                "post_wad_analysis request failed with status {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            );
        }
        Ok(())
    }

    pub async fn post_map_analysis(&self, analysis: MapAnalysis) -> Result<()> {
        let url = format!(
            "{}/wad/{}/map/{}/analysis",
            self.base_url, analysis.wad_id, analysis.map_name
        );
        let resp = self
            .inner
            .post(&url)
            .json(&analysis)
            .send()
            .await
            .context("Failed to send post_map_analysis request")?;
        if !resp.status().is_success() {
            bail!(
                "post_map_analysis request failed with status {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            );
        }
        Ok(())
    }

    pub async fn get_wad(&self, wad_id: Uuid) -> Result<Option<ReadWad>> {
        let url = format!("{}/wad/{}", self.base_url, wad_id);
        let resp = self
            .inner
            .get(&url)
            .send()
            .await
            .context("Failed to send get_wad request")?;
        match resp.status() {
            reqwest::StatusCode::NOT_FOUND => Ok(None),
            status if !status.is_success() => {
                bail!(
                    "get_wad request failed with status {}: {}",
                    status,
                    resp.text().await.unwrap_or_default()
                );
            }
            _ => Ok(Some(
                resp.json::<ReadWad>()
                    .await
                    .context("Failed to parse get_wad response")?,
            )),
        }
    }

    pub async fn get_user_profile(&self, user_id: Uuid) -> Result<Option<UserProfileView>> {
        let url = format!("{}/user/profile/{}", self.base_url, user_id);
        let resp = self
            .inner
            .get(&url)
            .send()
            .await
            .context("Failed to send get_user_profile request")?;
        match resp.status() {
            reqwest::StatusCode::NOT_FOUND => Ok(None),
            status if !status.is_success() => {
                bail!(
                    "get_user_profile request failed with status {}: {}",
                    status,
                    resp.text().await.unwrap_or_default()
                );
            }
            _ => Ok(Some(
                resp.json::<UserProfileView>()
                    .await
                    .context("Failed to parse get_user_profile response")?,
            )),
        }
    }

    pub async fn put_user_profile(
        &self,
        user_id: Uuid,
        req: &PutUserProfileRequest,
    ) -> Result<UserProfileFull> {
        let url = format!("{}/user/profile/{}", self.base_url, user_id);
        let resp = self
            .inner
            .put(&url)
            .json(req)
            .send()
            .await
            .context("Failed to send put_user_profile request")?;
        if !resp.status().is_success() {
            bail!(
                "put_user_profile request failed with status {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            );
        }
        resp.json::<UserProfileFull>()
            .await
            .context("Failed to parse put_user_profile response")
    }

    pub async fn create_user_profile(
        &self,
        user_id: Uuid,
        username: &str,
    ) -> Result<UserProfileFull> {
        self.put_user_profile(
            user_id,
            &PutUserProfileRequest {
                avatar_url: None,
                player_color: None,
                username: Some(username.to_string()),
                display_name: Some(username.to_string()),
                privacy_hide_activity: Some(false),
            },
        )
        .await
    }

    pub async fn post_user_activity(&self, user_id: Uuid) -> Result<()> {
        let url = format!("{}/user/activity/{}", self.base_url, user_id);
        let resp = self
            .inner
            .post(&url)
            .send()
            .await
            .context("Failed to send post_user_activity request")?;
        if !resp.status().is_success() {
            bail!(
                "post_user_activity request failed with status {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            );
        }
        Ok(())
    }
}
