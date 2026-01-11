use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::RequestContext;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Email {
    pub id: Uuid,
    pub to: String,
    pub from: String,
    pub subject: Option<String>,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cc: Option<String>,
    pub thread_id: Option<Uuid>,
    pub headers: Vec<PostmarkHeader>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChannelGenRequest {
    pub number: i64,
    pub ctx: RequestContext,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GuideGenRequest {
    pub start_chunk: i64,
    pub end_chunk: i64,
    pub offset: i64,
    pub limit: i64,
    pub ctx: RequestContext,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ScheduleGenRequest {
    pub channel: Channel,
    pub chunk: i64,
    pub ctx: RequestContext,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct EmailGenRequest {
    pub correlation_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<Uuid>,
    pub from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,

    pub reply_headers: Vec<PostmarkHeader>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Channel {
    pub id: Uuid,            // UUID of the channel
    pub number: i64,         // e.g., 42
    pub name: String,        // "Sludge Network"
    pub description: String, // "A channel about sludge."
    pub prompt: String,      // The verbatim input prompt used to generate the model.
    pub created_at: i64,     // milliseconds since epoch
    pub updated_at: i64,     // milliseconds since epoch
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Program {
    pub id: Uuid, // UUID of the program
    pub name: String,
    pub duration: i64,
    pub description: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Schedule {
    pub id: Uuid, // UUID of the schedule segment
    pub channel_id: Uuid,
    pub channel_number: i64,
    pub chunk: i64,
    pub created_at: i64,
    pub programs: Vec<Program>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuideProgram {
    pub start_time: i64,
    pub end_time: i64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated_left: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated_right: bool,
    pub normalized_duration: i64,
    #[serde(flatten)]
    pub program: Program,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuideChannel {
    pub id: Uuid,            // UUID of the channel
    pub number: i64,         // e.g., 42
    pub name: String,        // "Sludge Network"
    pub description: String, // "A channel about sludge."
    pub updated_at: i64,     // milliseconds since epoch
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuideRow {
    pub channel: GuideChannel,
    pub programs: Vec<GuideProgram>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Guide {
    pub start_chunk: i64,
    pub end_chunk: i64,
    pub start_time: i64,
    pub end_time: i64,
    pub offset: i64,
    pub limit: i64,
    pub rows: Vec<GuideRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,
    pub classification: Classification,
    pub disposition: Disposition,
    #[serde(default)]
    pub risk_tags: Vec<RiskTag>,
    #[serde(default)]
    pub policy_refs: Vec<PolicyRef>,
    pub log: LogBlock,
    pub reply: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub appendix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Classification {
    Low,
    Medium,
    High,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Disposition {
    Retain,
    Escalate,
    NoAction,
    CloseRecommended,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskTag {
    Broadcast,
    Material,
    Regulatory,
    Privacy,
    Security,
    Export,
    Brand,
    ThirdParty,
    Financial,
    Medical,
    Harassment,
    Ip,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRef {
    pub code: String,
    pub title: String,
    pub rev: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogBlock {
    pub case_status: CaseStatus,
    pub routing: Routing,
    pub retention: String,
    #[serde(default)]
    pub audit_flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CaseStatus {
    Open,
    PendingReview,
    OnHold,
    Closing,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Routing {
    Triage,
    Legal,
    Security,
    BroadcastStandards,
    Archives,
    Risk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PostmarkHeader {
    pub name: String,
    pub value: String,
}
