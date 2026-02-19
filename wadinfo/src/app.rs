use std::{ops::Deref, sync::Arc};
use tokio_util::sync::CancellationToken;

use crate::{avatar::AvatarStore, db::Database};

pub struct AppInner {
    pub cancel: CancellationToken,
    pub db: Database,
    pub avatar_store: AvatarStore,
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
    pub fn new(cancel: CancellationToken, db: Database, avatar_store: AvatarStore) -> Self {
        Self {
            inner: Arc::new(AppInner {
                cancel,
                db,
                avatar_store,
            }),
        }
    }
}
