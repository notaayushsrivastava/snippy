use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Window,
    BrowserTab,
    CompanionContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub scope_id: String,
    pub target_kind: TargetKind,
    pub app_name: String,
    pub window_title: String,
    pub process_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionKind {
    Accessibility,
    ScreenCapture,
    InputInjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionRequest {
    pub permissions: Vec<PermissionKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionState {
    Granted,
    Denied { missing: Vec<PermissionKind> },
    RequiresUserAction { instructions: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticElement {
    pub locator: String,
    pub role: String,
    pub label: String,
    pub bounds: Option<Bounds>,
    pub is_secret: bool,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub protocol_version: u32,
    pub scope: Scope,
    pub active_window_proof: String,
    pub semantic_elements: Vec<SemanticElement>,
    pub visual_context_summary: Option<String>,
    pub observation_hash: String,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Focus,
    Invoke,
    SetNonSecretValue,
    KeyPress,
    Pointer,
    Scroll,
    Wait,
    RequestUserTakeover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskCategory {
    Observe,
    Navigate,
    Draft,
    LocalChange,
    ExternalImpact,
    Sensitive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoordinateHint {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetLocator {
    pub observation_hash: String,
    pub semantic_locator: Option<String>,
    pub coordinate_hint: Option<CoordinateHint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovedAction {
    pub protocol_version: u32,
    pub action_id: String,
    pub scope_id: String,
    pub observation_hash: String,
    pub kind: ActionKind,
    pub target: TargetLocator,
    pub arguments: serde_json::Value,
    pub risk: RiskCategory,
    pub reason: String,
    pub expected_postcondition: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionResult {
    pub action_id: String,
    pub executed_at: DateTime<Utc>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedState {
    pub action_id: String,
    pub scope_id: String,
    pub expected_postcondition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum Verification {
    Confirmed { evidence: String },
    NotConfirmed { reason: String },
    Ambiguous { reason: String },
    Blocked { reason: String },
}

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error", content = "detail")]
pub enum PlatformError {
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("target not found: {0}")]
    TargetNotFound(String),
    #[error("stale locator: {0}")]
    StaleLocator(String),
    #[error("active window focus changed: {0}")]
    FocusChanged(String),
    #[error("unsupported control: {0}")]
    UnsupportedControl(String),
    #[error("platform operation timed out: {0}")]
    Timeout(String),
    #[error("secure storage error: {0}")]
    SecureStorage(String),
}

#[async_trait]
pub trait SecureStorage: Send + Sync {
    async fn get_secret(&self, key: &str) -> Result<Option<String>, PlatformError>;
    async fn set_secret(&self, key: &str, value: &str) -> Result<(), PlatformError>;
    async fn delete_secret(&self, key: &str) -> Result<(), PlatformError>;
}

#[async_trait]
pub trait PlatformAdapter: Send + Sync {
    async fn request_permissions(&self, request: PermissionRequest) -> PermissionState;
    async fn observe_target(&self, scope: &Scope) -> Result<Observation, PlatformError>;
    async fn execute_action(&self, action: &ApprovedAction) -> Result<ActionResult, PlatformError>;
    async fn verify_result(&self, expected: &ExpectedState) -> Verification;
    fn secure_storage(&self) -> &dyn SecureStorage;
}
