use async_trait::async_trait;
use platform_contract::{ActionKind, Observation, RiskCategory, Scope, TargetLocator};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Ready,
    Blocked,
    RequestUserTakeover,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedAction {
    pub kind: ActionKind,
    pub target: TargetLocator,
    pub arguments: serde_json::Value,
    pub risk: RiskCategory,
    pub reason: String,
    pub expected_postcondition: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenAnnotation {
    pub kind: String,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub width: Option<i32>,
    #[serde(default)]
    pub height: Option<i32>,
    #[serde(default)]
    pub x2: Option<i32>,
    #[serde(default)]
    pub y2: Option<i32>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentProposal {
    pub status: ProposalStatus,
    pub user_message: String,
    pub actions: Vec<ProposedAction>,
    #[serde(default)]
    pub annotations: Vec<ScreenAnnotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentInput {
    pub user_task: String,
    pub scope: Scope,
    pub observation: Observation,
    pub recent_steps: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error", content = "detail")]
pub enum ProviderError {
    #[error("provider request timed out: {0}")]
    Timeout(String),
    #[error("invalid structured proposal: {0}")]
    InvalidProposal(String),
    #[error("provider unavailable: {0}")]
    Unavailable(String),
}

#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    async fn propose(&self, input: AgentInput) -> Result<AgentProposal, ProviderError>;
}

/// Validates a model-returned `AgentProposal` against `PROMPTS.md` rules.
pub fn validate_proposal(
    proposal: &AgentProposal,
    observation: &Observation,
) -> Result<(), ProviderError> {
    if proposal.user_message.trim().is_empty() {
        return Err(ProviderError::InvalidProposal(
            "user_message must not be empty".into(),
        ));
    }

    if proposal.status == ProposalStatus::Ready && proposal.actions.is_empty() {
        return Err(ProviderError::InvalidProposal(
            "ready proposal must contain at least one action".into(),
        ));
    }

    let consequential_count = proposal
        .actions
        .iter()
        .filter(|a| {
            matches!(
                a.risk,
                RiskCategory::LocalChange | RiskCategory::ExternalImpact
            )
        })
        .count();
    if consequential_count > 1 {
        return Err(ProviderError::InvalidProposal(
            "multiple consequential actions in one proposal are forbidden".into(),
        ));
    }

    for action in &proposal.actions {
        if action.expected_postcondition.trim().is_empty() {
            return Err(ProviderError::InvalidProposal(
                "expected_postcondition must not be empty".into(),
            ));
        }
        if action.target.observation_hash.trim().is_empty() {
            return Err(ProviderError::InvalidProposal(
                "target.observation_hash must not be empty".into(),
            ));
        }
        if action.target.observation_hash != observation.observation_hash {
            return Err(ProviderError::InvalidProposal(
                "target.observation_hash does not match current observation".into(),
            ));
        }
        if let Some(coord) = action.target.coordinate_hint {
            if coord.x < 0 || coord.y < 0 || coord.x > 16384 || coord.y > 16384 {
                return Err(ProviderError::InvalidProposal(
                    "coordinate_hint out of safe bounds".into(),
                ));
            }
            // Reject coordinate-only pointer actions when semantic elements are available in single-window mode
            if action.kind == ActionKind::Pointer
                && action.target.semantic_locator.is_none()
                && !observation.semantic_elements.is_empty()
                && observation.scope.scope_id != "desktop:all"
            {
                return Err(ProviderError::InvalidProposal(
                    "coordinate-only action rejected when semantic elements are available".into(),
                ));
            }
        }
    }

    Ok(())
}
