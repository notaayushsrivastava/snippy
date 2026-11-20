use chrono::{Duration, Utc};
use platform_contract::{
    ActionKind, ApprovedAction, Observation, RiskCategory, Scope, PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};

use crate::provider::ProposedAction;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "decision")]
pub enum PolicyDecision {
    Allow {
        calculated_risk: RiskCategory,
    },
    RequireApproval {
        calculated_risk: RiskCategory,
        consequence: String,
    },
    RequireTakeover {
        reason: String,
    },
    Deny {
        reason: String,
    },
}

pub struct PolicyEngine;

impl PolicyEngine {
    /// Authoritatively evaluates a proposed action against the current observation and scope.
    /// Never trusts the model's declared `risk` alone.
    pub fn evaluate(
        action: &ProposedAction,
        observation: &Observation,
        scope: &Scope,
    ) -> PolicyDecision {
        if observation.protocol_version != PROTOCOL_VERSION {
            return PolicyDecision::Deny {
                reason: format!(
                    "unsupported protocol version: {}",
                    observation.protocol_version
                ),
            };
        }

        if observation.scope.scope_id != scope.scope_id {
            return PolicyDecision::Deny {
                reason: "observation scope does not match approved scope".into(),
            };
        }

        if action.target.observation_hash != observation.observation_hash {
            return PolicyDecision::Deny {
                reason: "stale observation_hash; re-observation required".into(),
            };
        }

        if action.kind == ActionKind::RequestUserTakeover {
            return PolicyDecision::RequireTakeover {
                reason: "sensitive field or user takeover requested".into(),
            };
        }

        // Check semantic target if specified
        if let Some(locator) = &action.target.semantic_locator {
            let matched = observation.semantic_elements.iter().find(|el| {
                &el.locator == locator
                    || (locator.starts_with("office:excel:") && el.locator == "office:excel:range")
            });
            let Some(element) = matched else {
                return PolicyDecision::Deny {
                    reason: format!("stale or missing semantic locator: {locator}"),
                };
            };

            if element.is_secret {
                return PolicyDecision::RequireTakeover {
                    reason: format!("target '{locator}' is a sensitive/secret field"),
                };
            }

            if !element.is_enabled {
                return PolicyDecision::Deny {
                    reason: format!("target '{locator}' is disabled"),
                };
            }
        }

        let calculated_risk = Self::calculate_risk(action);
        match calculated_risk {
            RiskCategory::Sensitive => PolicyDecision::RequireTakeover {
                reason: "action classified as sensitive".into(),
            },
            RiskCategory::LocalChange | RiskCategory::ExternalImpact => {
                PolicyDecision::RequireApproval {
                    calculated_risk,
                    consequence: action.expected_postcondition.clone(),
                }
            }
            RiskCategory::Observe | RiskCategory::Navigate | RiskCategory::Draft => {
                PolicyDecision::Allow { calculated_risk }
            }
        }
    }

    pub fn calculate_risk(action: &ProposedAction) -> RiskCategory {
        // Host policy overrides model risk upward when needed
        let base = match action.kind {
            ActionKind::Wait => RiskCategory::Observe,
            ActionKind::Focus | ActionKind::Scroll => RiskCategory::Navigate,
            ActionKind::SetNonSecretValue => RiskCategory::Draft,
            ActionKind::RequestUserTakeover => RiskCategory::Sensitive,
            ActionKind::Invoke | ActionKind::KeyPress | ActionKind::Pointer => {
                if action.risk >= RiskCategory::LocalChange {
                    action.risk
                } else {
                    RiskCategory::Navigate
                }
            }
        };
        if action.risk > base {
            action.risk
        } else {
            base
        }
    }

    pub fn bind_approved_action(
        action: &ProposedAction,
        scope: &Scope,
        calculated_risk: RiskCategory,
    ) -> ApprovedAction {
        ApprovedAction {
            protocol_version: PROTOCOL_VERSION,
            action_id: uuid::Uuid::new_v4().to_string(),
            scope_id: scope.scope_id.clone(),
            observation_hash: action.target.observation_hash.clone(),
            kind: action.kind,
            target: action.target.clone(),
            arguments: action.arguments.clone(),
            risk: calculated_risk,
            reason: action.reason.clone(),
            expected_postcondition: action.expected_postcondition.clone(),
            expires_at: Utc::now() + Duration::seconds(120),
        }
    }
}
