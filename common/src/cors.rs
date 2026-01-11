use reqwest::{
    Method,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use std::time::Duration;
use tower_http::cors::{AllowOrigin, CorsLayer};

pub fn dev() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_credentials(true)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION]) // list explicitly
        .max_age(Duration::from_secs(60 * 60))
}
