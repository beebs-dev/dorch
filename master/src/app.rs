use std::{ops::Deref, sync::Arc};
use tokio_util::sync::CancellationToken;

pub struct AppInner {
    pub cancel: CancellationToken,
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
    pub fn new(cancel: CancellationToken, nats: async_nats::Client) -> Self {
        Self {
            inner: Arc::new(AppInner { cancel, nats }),
        }
    }
}
