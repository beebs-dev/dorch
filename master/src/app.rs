use crate::store::GameInfoStore;
use dorch_auth::client::Client as AuthClient;
use kube::client::Client;
use std::{ops::Deref, sync::Arc};
use tokio_util::sync::CancellationToken;

pub struct AppInner {
    pub cancel: CancellationToken,
    pub nats: async_nats::Client,
    pub client: Client,
    pub namespace: String,
    pub store: GameInfoStore,
    pub auth: AuthClient,
    pub game_resource_prefix: String,
}

#[derive(Clone)]
pub struct App {
    inner: Arc<AppInner>,
}

impl Deref for App {
    type Target = AppInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl App {
    pub fn new(
        cancel: CancellationToken,
        nats: async_nats::Client,
        client: Client,
        namespace: String,
        store: GameInfoStore,
        auth: AuthClient,
        game_resource_prefix: String,
    ) -> Self {
        Self {
            inner: Arc::new(AppInner {
                cancel,
                nats,
                client,
                namespace,
                store,
                auth,
                game_resource_prefix,
            }),
        }
    }
}
