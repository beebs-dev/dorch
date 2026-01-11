use std::{ops::Deref, sync::Arc};
use tokio_util::sync::CancellationToken;

pub struct AppInner {
    pub cancel: CancellationToken,
    pub api_key: String,
    pub api_secret: String,
    pub external_livekit_url: String,
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
        api_key: String,
        api_secret: String,
        external_livekit_url: String,
    ) -> Self {
        Self {
            inner: Arc::new(AppInner {
                cancel,
                api_key,
                api_secret,
                external_livekit_url,
            }),
        }
    }
}
