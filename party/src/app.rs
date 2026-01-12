use std::{ops::Deref, sync::Arc};
use tokio_util::sync::CancellationToken;

use crate::party_store::PartyInfoStore;

pub struct AppInner {
    pub cancel: CancellationToken,
    pub store: PartyInfoStore,
    pub nats: async_nats::Client,
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
    pub fn new(cancel: CancellationToken, nats: async_nats::Client, store: PartyInfoStore) -> Self {
        Self {
            inner: Arc::new(AppInner {
                cancel,
                nats,
                store,
            }),
        }
    }
}
