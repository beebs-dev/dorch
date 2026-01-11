#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Clone)]
pub struct Keycloak {
    pub args: dorch_common::args::KeycloakArgs,
    pub client: reqwest::Client,
}

impl Keycloak {
    pub async fn introspect_token(
        &self,
        token: &str, // the access token to introspect
    ) -> Result<Introspection> {
        let url = format!(
            "{}/realms/{}/protocol/openid-connect/token/introspect",
            self.args.endpoint.trim_end_matches('/'),
            self.args.realm,
        );
        self.client
            .post(url)
            .basic_auth(&self.args.client_id, Some(&self.args.client_secret))
            .form(&[("token", token), ("token_type_hint", "access_token")])
            .send()
            .await
            .context("Failed to send introspection request")?
            .error_for_status()
            .context("Failed to introspect token")?
            .json()
            .await
            .context("Failed to parse introspection response")
    }
}

#[derive(Debug, Deserialize)]
pub struct Introspection {
    pub active: bool,
    pub sub: Option<String>,
    pub scope: Option<String>,
    pub username: Option<String>,
    pub exp: Option<u64>,
    pub iat: Option<u64>,
    pub nbf: Option<u64>,
    pub aud: Option<serde_json::Value>,
    pub iss: Option<String>,
    pub jti: Option<String>,
    pub token_type: Option<String>,
    pub client_id: Option<String>,
}

impl Introspection {
    pub fn has_scope(&self, required: &str) -> bool {
        if let Some(scopes) = &self.scope {
            scopes.split_whitespace().any(|s| s == required)
        } else {
            false
        }
    }

    pub fn validate(&self) -> Result<()> {
        if !self.active {
            return Err(anyhow::anyhow!("Token is not active"));
        }
        Ok(())
    }
}
