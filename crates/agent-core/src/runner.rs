use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::Utc;
use platform_contract::{
    ApprovedAction, ExpectedState, PlatformAdapter, PlatformError, RiskCategory, Verification,
};
use serde::{Deserialize, Serialize};
use session_store::{SessionStatus, SessionStore, TimelineStep};

use crate::policy::{PolicyDecision, PolicyEngine};
use crate::provider::{
    validate_proposal, AgentInput, ProposalStatus, ProviderAdapter, ProviderError,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum StepOutcome {
    Executed {
        verification: Verification,
        summary: String,
    },
    AwaitingApproval {
        action: Box<ApprovedAction>,
        consequence: String,
    },
    AwaitingSecret {
        reason: String,
    },
    Blocked {
        reason: String,
    },
    Complete {
        summary: String,
    },
    Stopped,
}

pub struct AgentRunner<P: PlatformAdapter, M: ProviderAdapter, S: SessionStore> {
    platform: Arc<P>,
    provider: Arc<M>,
    store: Arc<S>,
}

impl<P: PlatformAdapter, M: ProviderAdapter, S: SessionStore> AgentRunner<P, M, S> {
    pub fn new(platform: Arc<P>, provider: Arc<M>, store: Arc<S>) -> Self {
        Self {
            platform,
            provider,
            store,
        }
    }

    pub async fn step_once(
        &self,
        session_id: &str,
        cancel_flag: &AtomicBool,
    ) -> Result<StepOutcome, String> {
        if cancel_flag.load(Ordering::SeqCst) {
            let _ = self
                .store
                .update_status(session_id, SessionStatus::Stopped)
                .await;
            return Ok(StepOutcome::Stopped);
        }

        let session = self
            .store
            .get_session(session_id)
            .await
            .map_err(|e| e.to_string())?;

        self.store
            .update_status(session_id, SessionStatus::Inspecting)
            .await
            .map_err(|e| e.to_string())?;

        let observation = match self.platform.observe_target(&session.scope).await {
            Ok(obs) => obs,
            Err(err) => {
                self.store
                    .update_status(session_id, SessionStatus::Blocked)
                    .await
                    .map_err(|e| e.to_string())?;
                return Ok(StepOutcome::Blocked {
                    reason: format_platform_error(&err),
                });
            }
        };

        if cancel_flag.load(Ordering::SeqCst) {
            self.store
                .update_status(session_id, SessionStatus::Stopped)
                .await
                .map_err(|e| e.to_string())?;
            return Ok(StepOutcome::Stopped);
        }

        self.store
            .update_status(session_id, SessionStatus::Planning)
            .await
            .map_err(|e| e.to_string())?;

        let recent_steps = session
            .steps
            .iter()
            .map(|s| s.action_summary.clone())
            .collect();

        let proposal = match self
            .provider
            .propose(AgentInput {
                user_task: session.task.clone(),
                scope: session.scope.clone(),
                observation: observation.clone(),
                recent_steps,
            })
            .await
        {
            Ok(p) => p,
            Err(err) => {
                self.store
                    .update_status(session_id, SessionStatus::Blocked)
                    .await
                    .map_err(|e| e.to_string())?;
                return Ok(StepOutcome::Blocked {
                    reason: format_provider_error(&err),
                });
            }
        };

        if let Err(err) = validate_proposal(&proposal, &observation) {
            self.store
                .update_status(session_id, SessionStatus::Blocked)
                .await
                .map_err(|e| e.to_string())?;
            return Ok(StepOutcome::Blocked {
                reason: format_provider_error(&err),
            });
        }

        match proposal.status {
            ProposalStatus::Complete => {
                self.store
                    .update_status(session_id, SessionStatus::Complete)
                    .await
                    .map_err(|e| e.to_string())?;
                return Ok(StepOutcome::Complete {
                    summary: proposal.user_message,
                });
            }
            ProposalStatus::Blocked => {
                self.store
                    .update_status(session_id, SessionStatus::Blocked)
                    .await
                    .map_err(|e| e.to_string())?;
                return Ok(StepOutcome::Blocked {
                    reason: proposal.user_message,
                });
            }
            ProposalStatus::RequestUserTakeover => {
                self.store
                    .update_status(session_id, SessionStatus::AwaitingSecret)
                    .await
                    .map_err(|e| e.to_string())?;
                return Ok(StepOutcome::AwaitingSecret {
                    reason: proposal.user_message,
                });
            }
            ProposalStatus::Ready => {}
        }

        let proposed_action = &proposal.actions[0];
        let decision = PolicyEngine::evaluate(proposed_action, &observation, &session.scope);

        match decision {
            PolicyDecision::Allow { calculated_risk } => {
                if cancel_flag.load(Ordering::SeqCst) {
                    self.store
                        .update_status(session_id, SessionStatus::Stopped)
                        .await
                        .map_err(|e| e.to_string())?;
                    return Ok(StepOutcome::Stopped);
                }

                let approved = PolicyEngine::bind_approved_action(
                    proposed_action,
                    &session.scope,
                    calculated_risk,
                );
                self.execute_and_verify(session_id, approved, "allow".into())
                    .await
            }
            PolicyDecision::RequireApproval {
                calculated_risk,
                consequence,
            } => {
                let approved = PolicyEngine::bind_approved_action(
                    proposed_action,
                    &session.scope,
                    calculated_risk,
                );
                self.store
                    .update_status(session_id, SessionStatus::AwaitingApproval)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(StepOutcome::AwaitingApproval {
                    action: Box::new(approved),
                    consequence,
                })
            }
            PolicyDecision::RequireTakeover { reason } => {
                self.store
                    .update_status(session_id, SessionStatus::AwaitingSecret)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(StepOutcome::AwaitingSecret { reason })
            }
            PolicyDecision::Deny { reason } => {
                self.store
                    .update_status(session_id, SessionStatus::Blocked)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(StepOutcome::Blocked { reason })
            }
        }
    }

    pub async fn execute_approved(
        &self,
        session_id: &str,
        approved: ApprovedAction,
        cancel_flag: &AtomicBool,
    ) -> Result<StepOutcome, String> {
        if cancel_flag.load(Ordering::SeqCst) || Utc::now() > approved.expires_at {
            self.store
                .update_status(session_id, SessionStatus::Blocked)
                .await
                .map_err(|e| e.to_string())?;
            return Ok(StepOutcome::Blocked {
                reason: "approval expired or cancelled".into(),
            });
        }
        self.execute_and_verify(session_id, approved, "user_approved".into())
            .await
    }

    async fn execute_and_verify(
        &self,
        session_id: &str,
        approved: ApprovedAction,
        policy_decision: String,
    ) -> Result<StepOutcome, String> {
        self.store
            .update_status(session_id, SessionStatus::Acting)
            .await
            .map_err(|e| e.to_string())?;

        let result = match self.platform.execute_action(&approved).await {
            Ok(res) => res,
            Err(err) => {
                self.store
                    .update_status(session_id, SessionStatus::Blocked)
                    .await
                    .map_err(|e| e.to_string())?;
                return Ok(StepOutcome::Blocked {
                    reason: format_platform_error(&err),
                });
            }
        };

        let verification = self
            .platform
            .verify_result(&ExpectedState {
                action_id: approved.action_id.clone(),
                scope_id: approved.scope_id.clone(),
                expected_postcondition: approved.expected_postcondition.clone(),
            })
            .await;

        let session = self
            .store
            .get_session(session_id)
            .await
            .map_err(|e| e.to_string())?;
        let step_number = (session.steps.len() as u32) + 1;

        self.store
            .append_step(
                session_id,
                TimelineStep {
                    step_number,
                    timestamp: Utc::now(),
                    action_summary: result.summary.clone(),
                    target_label: approved
                        .target
                        .semantic_locator
                        .clone()
                        .unwrap_or_else(|| "coordinate_target".into()),
                    risk: approved.risk,
                    observation_hash: approved.observation_hash.clone(),
                    policy_decision,
                    verification: Some(verification.clone()),
                },
            )
            .await
            .map_err(|e| e.to_string())?;

        match &verification {
            Verification::Confirmed { .. } => Ok(StepOutcome::Executed {
                verification,
                summary: result.summary,
            }),
            Verification::NotConfirmed { reason }
            | Verification::Ambiguous { reason }
            | Verification::Blocked { reason } => {
                self.store
                    .update_status(session_id, SessionStatus::Blocked)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(StepOutcome::Blocked {
                    reason: reason.clone(),
                })
            }
        }
    }
}

fn format_platform_error(err: &PlatformError) -> String {
    err.to_string()
}

fn format_provider_error(err: &ProviderError) -> String {
    err.to_string()
}

pub fn default_safe_risk() -> RiskCategory {
    RiskCategory::Observe
}
