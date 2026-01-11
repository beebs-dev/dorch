use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Default)]
pub struct VideoGrant {
    #[serde(
        rename = "roomJoin",
        skip_serializing_if = "std::ops::Not::not",
        default
    )]
    pub room_join: bool,

    #[serde(
        rename = "roomAdmin",
        skip_serializing_if = "std::ops::Not::not",
        default
    )]
    pub room_admin: bool,

    #[serde(
        rename = "roomCreate",
        skip_serializing_if = "std::ops::Not::not",
        default
    )]
    pub room_create: bool,

    #[serde(
        rename = "roomList",
        skip_serializing_if = "std::ops::Not::not",
        default
    )]
    pub room_list: bool,

    #[serde(
        rename = "roomRecord",
        skip_serializing_if = "std::ops::Not::not",
        default
    )]
    pub room_record: bool,

    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,

    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recorder: bool,

    pub room: String,

    #[serde(
        default,
        rename = "canPublish",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub can_publish: bool,

    #[serde(
        default,
        rename = "canPublishData",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub can_publish_data: bool,

    #[serde(
        rename = "canPublishSources",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub can_publish_sources: Option<Vec<String>>,

    #[serde(
        rename = "canSubscribe",
        skip_serializing_if = "std::ops::Not::not",
        default
    )]
    pub can_subscribe: bool,
}

#[derive(Serialize)]
pub struct LiveKitClaims {
    pub iss: String,       // API key
    pub exp: usize,        // expiration timestamp
    pub sub: String,       // identity of the user
    pub video: VideoGrant, // LiveKit video grant
}

fn token_expire_seconds() -> usize {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    (now + 5 * 60) as usize // 5 minutes
}

fn default_claims(api_key: &str, identity: &str, room: &str) -> LiveKitClaims {
    LiveKitClaims {
        iss: api_key.to_owned(),
        exp: token_expire_seconds(),
        sub: identity.to_owned(),
        video: VideoGrant {
            room_join: true,
            room: room.to_owned(),
            can_publish: true,
            can_subscribe: true,
            can_publish_data: true,
            can_publish_sources: None,
            hidden: false,
            recorder: false,
            room_admin: false,
            room_create: false,
            room_list: false,
            room_record: false,
        },
    }
}

pub fn make_livekit_token(api_key: &str, api_secret: &str, identity: &str, room: &str) -> String {
    let claims = default_claims(api_key, identity, room);
    let header = Header::new(Algorithm::HS256);
    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(api_secret.as_bytes()),
    )
    .expect("failed to sign livekit token")
}
