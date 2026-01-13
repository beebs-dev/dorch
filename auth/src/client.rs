use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct UserRecordJson {
    pub username: String,
    pub salt_b64: String,
    pub verifier_b64: String,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Clone)]
pub struct Client {
    client: reqwest::Client,
    endpoint: String,
}

impl Client {
    pub fn new(endpoint: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint,
        }
    }

    pub async fn post_user_record(&self, json: &UserRecordJson) -> Result<()> {
        let resp = self
            .client
            .post(format!("{}/admin/user", self.endpoint))
            .json(json)
            .send()
            .await
            .context("Failed to send post user record request")?;
        if resp.status().is_success() {
            Ok(())
        } else {
            bail!(
                "Failed to post user record: {:?}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            )
        }
    }
}
