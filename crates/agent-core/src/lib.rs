pub mod mocks;
pub mod policy;
pub mod provider;
pub mod runner;

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    use chrono::Utc;
    use platform_contract::{
        ActionKind, Bounds, CoordinateHint, Observation, PlatformError, RiskCategory, Scope,
        SemanticElement, TargetKind, TargetLocator, Verification, PROTOCOL_VERSION,
    };
    use serde_json::json;
    use session_store::{InMemorySessionStore, SessionStatus, SessionStore};

    use crate::mocks::{MockPlatformAdapter, MockProviderAdapter};
    use crate::policy::{PolicyDecision, PolicyEngine};
    use crate::provider::{AgentProposal, ProposalStatus, ProposedAction, ProviderError};
    use crate::runner::{AgentRunner, StepOutcome};

    fn test_scope() -> Scope {
        Scope {
            scope_id: "scope-chrome-1".into(),
            target_kind: TargetKind::BrowserTab,
            app_name: "Chrome".into(),
            window_title: "Research Tab".into(),
            process_id: Some(1234),
        }
    }

    fn test_observation(scope: &Scope, include_secret: bool) -> Observation {
        Observation {
            protocol_version: PROTOCOL_VERSION,
            scope: scope.clone(),
            active_window_proof: "hwnd:1234".into(),
            semantic_elements: vec![
                SemanticElement {
                    locator: "btn-search".into(),
                    role: "button".into(),
                    label: "Search".into(),
                    bounds: Some(Bounds {
                        x: 10,
                        y: 20,
                        width: 80,
                        height: 30,
                    }),
                    is_secret: false,
                    is_enabled: true,
                },
                SemanticElement {
                    locator: "input-password".into(),
                    role: "textbox".into(),
                    label: "Password".into(),
                    bounds: Some(Bounds {
                        x: 10,
                        y: 60,
                        width: 200,
                        height: 30,
                    }),
                    is_secret: include_secret,
                    is_enabled: true,
                },
            ],
            visual_context_summary: Some("Browser page".into()),
            observation_hash: "obs-hash-1".into(),
            observed_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn safe_navigate_action_executes_and_verifies() {
        let scope = test_scope();
        let obs = test_observation(&scope, true);
        let platform = Arc::new(MockPlatformAdapter::new(
            vec![Ok(obs.clone())],
            vec![Verification::Confirmed {
                evidence: "Search button focused".into(),
            }],
        ));
        let provider = Arc::new(MockProviderAdapter::new(vec![Ok(AgentProposal {
            status: ProposalStatus::Ready,
            user_message: "Focusing Search button".into(),
            actions: vec![ProposedAction {
                kind: ActionKind::Focus,
                target: TargetLocator {
                    observation_hash: "obs-hash-1".into(),
                    semantic_locator: Some("btn-search".into()),
                    coordinate_hint: None,
                },
                arguments: json!({}),
                risk: RiskCategory::Navigate,
                reason: "Focus search".into(),
                expected_postcondition: "Search button has focus".into(),
            }],
            annotations: Vec::new(),
        })]));
        let store = Arc::new(InMemorySessionStore::new());
        let session = store
            .create_session("Search docs".into(), scope)
            .await
            .unwrap();

        let runner = AgentRunner::new(platform, provider, store.clone());
        let cancel = AtomicBool::new(false);
        let outcome = runner
            .step_once(&session.session_id, &cancel)
            .await
            .unwrap();

        assert!(matches!(outcome, StepOutcome::Executed { .. }));
        let updated = store.get_session(&session.session_id).await.unwrap();
        assert_eq!(updated.steps.len(), 1);
    }

    #[tokio::test]
    async fn secret_field_forces_user_takeover_even_if_model_says_draft() {
        let scope = test_scope();
        let obs = test_observation(&scope, true);
        let action = ProposedAction {
            kind: ActionKind::SetNonSecretValue,
            target: TargetLocator {
                observation_hash: "obs-hash-1".into(),
                semantic_locator: Some("input-password".into()),
                coordinate_hint: None,
            },
            arguments: json!({"text": "hunter2"}),
            risk: RiskCategory::Draft,
            reason: "Fill field".into(),
            expected_postcondition: "Field updated".into(),
        };

        let decision = PolicyEngine::evaluate(&action, &obs, &scope);
        assert!(matches!(decision, PolicyDecision::RequireTakeover { .. }));
    }

    #[tokio::test]
    async fn consequential_action_requires_approval() {
        let scope = test_scope();
        let obs = test_observation(&scope, true);
        let platform = Arc::new(MockPlatformAdapter::new(vec![Ok(obs)], vec![]));
        let provider = Arc::new(MockProviderAdapter::new(vec![Ok(AgentProposal {
            status: ProposalStatus::Ready,
            user_message: "Submitting form".into(),
            actions: vec![ProposedAction {
                kind: ActionKind::Invoke,
                target: TargetLocator {
                    observation_hash: "obs-hash-1".into(),
                    semantic_locator: Some("btn-search".into()),
                    coordinate_hint: None,
                },
                arguments: json!({}),
                risk: RiskCategory::ExternalImpact,
                reason: "Submit".into(),
                expected_postcondition: "Form submitted".into(),
            }],
            annotations: Vec::new(),
        })]));
        let store = Arc::new(InMemorySessionStore::new());
        let session = store.create_session("Submit".into(), scope).await.unwrap();

        let runner = AgentRunner::new(platform, provider, store.clone());
        let cancel = AtomicBool::new(false);
        let outcome = runner
            .step_once(&session.session_id, &cancel)
            .await
            .unwrap();

        assert!(matches!(outcome, StepOutcome::AwaitingApproval { .. }));
        let updated = store.get_session(&session.session_id).await.unwrap();
        assert_eq!(updated.status, SessionStatus::AwaitingApproval);
    }

    #[tokio::test]
    async fn stale_locator_fails_closed() {
        let scope = test_scope();
        let obs = test_observation(&scope, true);
        let action = ProposedAction {
            kind: ActionKind::Invoke,
            target: TargetLocator {
                observation_hash: "obs-hash-1".into(),
                semantic_locator: Some("btn-nonexistent".into()),
                coordinate_hint: None,
            },
            arguments: json!({}),
            risk: RiskCategory::Navigate,
            reason: "Click missing".into(),
            expected_postcondition: "Clicked".into(),
        };

        let decision = PolicyEngine::evaluate(&action, &obs, &scope);
        assert!(matches!(decision, PolicyDecision::Deny { .. }));
    }

    #[tokio::test]
    async fn coordinate_only_rejected_when_semantic_elements_exist() {
        let scope = test_scope();
        let obs = test_observation(&scope, true);
        let platform = Arc::new(MockPlatformAdapter::new(vec![Ok(obs)], vec![]));
        let provider = Arc::new(MockProviderAdapter::new(vec![Ok(AgentProposal {
            status: ProposalStatus::Ready,
            user_message: "Clicking coordinates".into(),
            actions: vec![ProposedAction {
                kind: ActionKind::Pointer,
                target: TargetLocator {
                    observation_hash: "obs-hash-1".into(),
                    semantic_locator: None,
                    coordinate_hint: Some(CoordinateHint { x: 15, y: 25 }),
                },
                arguments: json!({}),
                risk: RiskCategory::Navigate,
                reason: "Click".into(),
                expected_postcondition: "Clicked".into(),
            }],
            annotations: Vec::new(),
        })]));
        let store = Arc::new(InMemorySessionStore::new());
        let session = store.create_session("Click".into(), scope).await.unwrap();

        let runner = AgentRunner::new(platform, provider, store.clone());
        let cancel = AtomicBool::new(false);
        let outcome = runner
            .step_once(&session.session_id, &cancel)
            .await
            .unwrap();

        assert!(matches!(outcome, StepOutcome::Blocked { .. }));
    }

    #[tokio::test]
    async fn ambiguous_verification_blocks_further_progress() {
        let scope = test_scope();
        let obs = test_observation(&scope, true);
        let platform = Arc::new(MockPlatformAdapter::new(
            vec![Ok(obs)],
            vec![Verification::Ambiguous {
                reason: "Window state unclear".into(),
            }],
        ));
        let provider = Arc::new(MockProviderAdapter::new(vec![Ok(AgentProposal {
            status: ProposalStatus::Ready,
            user_message: "Focusing".into(),
            actions: vec![ProposedAction {
                kind: ActionKind::Focus,
                target: TargetLocator {
                    observation_hash: "obs-hash-1".into(),
                    semantic_locator: Some("btn-search".into()),
                    coordinate_hint: None,
                },
                arguments: json!({}),
                risk: RiskCategory::Navigate,
                reason: "Focus".into(),
                expected_postcondition: "Focused".into(),
            }],
            annotations: Vec::new(),
        })]));
        let store = Arc::new(InMemorySessionStore::new());
        let session = store.create_session("Focus".into(), scope).await.unwrap();

        let runner = AgentRunner::new(platform, provider, store.clone());
        let cancel = AtomicBool::new(false);
        let outcome = runner
            .step_once(&session.session_id, &cancel)
            .await
            .unwrap();

        assert!(matches!(outcome, StepOutcome::Blocked { .. }));
        let updated = store.get_session(&session.session_id).await.unwrap();
        assert_eq!(updated.status, SessionStatus::Blocked);
    }

    #[tokio::test]
    async fn permission_denied_and_provider_timeout_fail_closed() {
        let scope = test_scope();
        let platform = Arc::new(MockPlatformAdapter::new(
            vec![Err(PlatformError::PermissionDenied("AX denied".into()))],
            vec![],
        ));
        let provider = Arc::new(MockProviderAdapter::new(vec![Err(ProviderError::Timeout(
            "30s".into(),
        ))]));
        let store = Arc::new(InMemorySessionStore::new());
        let session = store.create_session("Test".into(), scope).await.unwrap();

        let runner = AgentRunner::new(platform, provider, store.clone());
        let cancel = AtomicBool::new(false);
        let outcome = runner
            .step_once(&session.session_id, &cancel)
            .await
            .unwrap();
        assert!(matches!(outcome, StepOutcome::Blocked { .. }));
    }

    #[tokio::test]
    async fn emergency_stop_halts_immediately() {
        let scope = test_scope();
        let platform = Arc::new(MockPlatformAdapter::new(vec![], vec![]));
        let provider = Arc::new(MockProviderAdapter::new(vec![]));
        let store = Arc::new(InMemorySessionStore::new());
        let session = store
            .create_session("Stop test".into(), scope)
            .await
            .unwrap();

        let runner = AgentRunner::new(platform, provider, store.clone());
        let cancel = AtomicBool::new(true);
        let outcome = runner
            .step_once(&session.session_id, &cancel)
            .await
            .unwrap();

        assert_eq!(outcome, StepOutcome::Stopped);
        let updated = store.get_session(&session.session_id).await.unwrap();
        assert_eq!(updated.status, SessionStatus::Stopped);
    }
}
