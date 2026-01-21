use crate::keycloak::Keycloak;
use anyhow::{Error, Result};
use axum::{Extension, RequestPartsExt, extract::FromRequestParts, http::request::Parts};
use axum_keycloak_auth::decode::KeycloakToken;
use bytes::Bytes;
use deadpool_redis::Pool;
use dorch_common::{
    args::{KeycloakArgs, RedisArgs},
    redis::listen_for_work,
    response,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct AppStateInner {
    cancel: CancellationToken,
    pub pool: Pool,
    pub nats: async_nats::Client,
    pub kc: Keycloak,
    pub handle: Arc<Mutex<Option<tokio::task::JoinHandle<Result<()>>>>>,
    pub master_tx: tokio::sync::broadcast::Sender<Bytes>,
}

#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

impl std::ops::Deref for AppState {
    type Target = AppStateInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl AppState {
    pub async fn new(
        cancel: CancellationToken,
        pool: deadpool_redis::Pool,
        nats: async_nats::Client,
        redis_args: RedisArgs,
        keycloak_args: KeycloakArgs,
    ) -> AppState {
        let cancel_clone = cancel.clone();
        let (tx, _rx) = tokio::sync::broadcast::channel(64);
        let handle = tokio::spawn(listen_for_work(
            cancel_clone,
            redis_args,
            tx.clone(),
            dorch_common::MASTER_TOPIC,
        ));
        let kc = Keycloak {
            args: keycloak_args,
            client: reqwest::Client::new(),
        };
        AppState {
            inner: Arc::new(AppStateInner {
                cancel,
                pool,
                nats,
                kc,
                handle: Arc::new(Mutex::new(Some(handle))),
                master_tx: tx,
            }),
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.cancel.cancel();
        if let Some(handle) = self.handle.lock().await.take() {
            handle.abort();
        }
        Ok(())
    }
}

pub struct UserId(pub Uuid);

impl<S> FromRequestParts<S> for UserId
where
    S: Send + Sync,
{
    type Rejection = axum::response::Response;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Extension(token) = match parts.extract::<Extension<KeycloakToken<String>>>().await {
            Ok(ext) => ext,
            Err(e) => {
                return Err(response::unauthorized(
                    Error::from(e).context("Failed to extract Keycloak token from request"),
                ));
            }
        };
        match Uuid::parse_str(&token.subject) {
            Ok(id) => Ok(UserId(id)),
            Err(e) => Err(response::unauthorized(
                Error::from(e).context("Invalid user ID in token subject claim"),
            )),
        }
    }
}
