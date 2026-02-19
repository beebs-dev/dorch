use std::{ops::Deref, sync::Arc};

pub struct AppInner {
    pub kc: dorch_common::args::KeycloakArgs,
    pub wadinfo: dorch_wadinfo::client::Client,
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
    pub fn new(kc: dorch_common::args::KeycloakArgs, wadinfo_endpoint: String) -> Self {
        Self {
            inner: Arc::new(AppInner {
                kc,
                wadinfo: dorch_wadinfo::client::Client::new(wadinfo_endpoint),
            }),
        }
    }
}
