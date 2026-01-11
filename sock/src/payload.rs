use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
pub struct WebsockAuthPayload {
    #[serde(rename = "c")]
    pub conn_id: Uuid,

    #[serde(rename = "p")]
    pub base64_encrypted_access_token: String,
}
