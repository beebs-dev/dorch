use std::{ops::Deref, sync::Arc};
use tokio_util::sync::CancellationToken;

use crate::db::Database;

pub struct AppInner {
    pub cancel: CancellationToken,
    pub db: Database,
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
    pub fn new(cancel: CancellationToken, db: Database) -> Self {
        Self {
            inner: Arc::new(AppInner { cancel, db }),
        }
    }
}
