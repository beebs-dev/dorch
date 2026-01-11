#![allow(dead_code)]

use anyhow::Result;
use std::{ops::Deref, sync::Arc};
use uuid::Uuid;

pub struct ClientInner {
    pub endpoint: String,
    pub client: reqwest::Client,
}

#[derive(Clone)]
pub struct Client {
    inner: Arc<ClientInner>,
}

impl Deref for Client {
    type Target = ClientInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Client {
    pub fn new(endpoint: String) -> Self {
        Self {
            inner: Arc::new(ClientInner {
                endpoint,
                client: reqwest::Client::new(),
            }),
        }
    }

    pub async fn is_party_member(&self, party_id: Uuid, user_id: Uuid) -> Result<bool> {
        let resp = self
            .client
            .get(format!(
                "{}/party/{}/member/{}",
                self.endpoint, party_id, user_id
            ))
            .send()
            .await?;
        match resp.status() {
            reqwest::StatusCode::OK => return Ok(true),
            reqwest::StatusCode::NOT_FOUND => return Ok(false),
            status => Err(anyhow::anyhow!(
                "Failed to check party membership: {}: {}",
                status,
                resp.text().await.unwrap_or_default()
            )),
        }
    }
}
