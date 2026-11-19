#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_core::mocks::MockSecureStorage;
use agent_core::provider::{AgentInput, AgentProposal, ProposalStatus, ProposedAction, ProviderAdapter, ProviderError};
use agent_core::runner::{AgentRunner, StepOutcome};
use async_trait::async_trait;
use chrono::Utc;
use platform_contract::{
    ActionKind, ActionResult, ApprovedAction, Bounds, ExpectedState, Observation,
    PROTOCOL_VERSION, PermissionRequest, PermissionState, PlatformAdapter, PlatformError,
    RiskCategory, Scope, SecureStorage, SemanticElement, TargetLocator, Verification,
};
use serde::Serialize;
use serde_json::json;
use session_store::{SessionRecord, SessionStatus, SessionStore, SqliteSessionStore};
use tauri::{Manager, State};
use tokio::sync::Mutex;

/// Demo platform adapter for Phase 2 IPC verification.
struct DemoPlatform {
    storage: MockSecureStorage,
}

#[async_trait]
impl PlatformAdapter for DemoPlatform {
    async fn request_permissions(&self, _request: PermissionRequest) -> PermissionState {
        PermissionState::Granted
    }

    async fn observe_target(&self, scope: &Scope) -> Result<Observation, PlatformError> {
        Ok(Observation {
            protocol_version: PROTOCOL_VERSION,
            scope: scope.clone(),
            active_window_proof: scope.scope_id.clone(),
            semantic_elements: vec![
                SemanticElement {
                    locator: "search-input".into(),
                    role: "textbox".into(),
                    label: "Search query".into(),
                    bounds: Some(Bounds {
                        x: 120,
                        y: 80,
                        width: 400,
                        height: 36,
                    }),
                    is_secret: false,
                    is_enabled: true,
                },
                SemanticElement {
                    locator: "submit-btn".into(),
                    role: "button".into(),
                    label: "Submit".into(),
                    bounds: Some(Bounds {
                        x: 530,
                        y: 80,
                        width: 80,
                        height: 36,
                    }),
                    is_secret: false,
                    is_enabled: true,
                },
            ],
            visual_context_summary: Some(format!("Observing {}", scope.window_title)),
            observation_hash: format!("obs-{}", scope.scope_id),
            observed_at: Utc::now(),
        })
    }

    async fn execute_action(&self, action: &ApprovedAction) -> Result<ActionResult, PlatformError> {
        Ok(ActionResult {
            action_id: action.action_id.clone(),
            executed_at: Utc::now(),
            summary: format!(
                "Executed {:?} on {}",
                action.kind,
                action
                    .target
                    .semantic_locator
                    .as_deref()
                    .unwrap_or("target")
            ),
        })
    }

    async fn verify_result(&self, expected: &ExpectedState) -> Verification {
        Verification::Confirmed {
            evidence: expected.expected_postcondition.clone(),
        }
    }

    fn secure_storage(&self) -> &dyn SecureStorage {
        &self.storage
    }
}

/// Demo provider adapter that proposes a safe step first, then a consequential step.
struct DemoProvider;

#[async_trait]
impl ProviderAdapter for DemoProvider {
    async fn propose(&self, input: AgentInput) -> Result<AgentProposal, ProviderError> {
        let hash = input.observation.observation_hash.clone();
        if input.recent_steps.is_empty() {
            Ok(AgentProposal {
                status: ProposalStatus::Ready,
                user_message: "Focusing target input".into(),
                actions: vec![ProposedAction {
                    kind: ActionKind::Focus,
                    target: TargetLocator {
                        observation_hash: hash,
                        semantic_locator: Some("search-input".into()),
                        coordinate_hint: None,
                    },
                    arguments: json!({}),
                    risk: RiskCategory::Navigate,
                    reason: "Focus input before typing".into(),
                    expected_postcondition: "Input field has focus".into(),
                }],
            })
        } else {
            Ok(AgentProposal {
                status: ProposalStatus::Ready,
                user_message: "Ready to submit".into(),
                actions: vec![ProposedAction {
                    kind: ActionKind::Invoke,
                    target: TargetLocator {
                        observation_hash: hash,
                        semantic_locator: Some("submit-btn".into()),
                        coordinate_hint: None,
                    },
                    arguments: json!({}),
                    risk: RiskCategory::ExternalImpact,
                    reason: "Submit requested action".into(),
                    expected_postcondition: "Action submitted in target window".into(),
                }],
            })
        }
    }
}

struct AppState {
    store: Arc<SqliteSessionStore>,
    runner: AgentRunner<DemoPlatform, DemoProvider, SqliteSessionStore>,
    cancel_flag: Arc<AtomicBool>,
    pending_approvals: Mutex<HashMap<String, ApprovedAction>>,
}

#[derive(Serialize)]
struct StepResponse {
    session: SessionRecord,
    outcome: StepOutcome,
}

#[tauri::command]
fn list_windows() -> Vec<Scope> {
    platform_windows::list_visible_windows()
}

#[tauri::command]
async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionRecord>, String> {
    state.store.list_sessions().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<SessionRecord, String> {
    state
        .store
        .get_session(&session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_session(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    state
        .store
        .delete_session(&session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_and_step(
    state: State<'_, AppState>,
    task: String,
    scope: Scope,
) -> Result<StepResponse, String> {
    state.cancel_flag.store(false, Ordering::SeqCst);
    let session = state
        .store
        .create_session(task, scope)
        .await
        .map_err(|e| e.to_string())?;
    step_session_internal(&state, &session.session_id).await
}

#[tauri::command]
async fn step_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<StepResponse, String> {
    step_session_internal(&state, &session_id).await
}

async fn step_session_internal(state: &AppState, session_id: &str) -> Result<StepResponse, String> {
    let outcome = state
        .runner
        .step_once(session_id, &state.cancel_flag)
        .await?;
    if let StepOutcome::AwaitingApproval { ref action, .. } = outcome {
        state
            .pending_approvals
            .lock()
            .await
            .insert(action.action_id.clone(), (**action).clone());
    }
    let session = state
        .store
        .get_session(session_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(StepResponse { session, outcome })
}

#[tauri::command]
async fn approve_action(
    state: State<'_, AppState>,
    session_id: String,
    action_id: String,
) -> Result<StepResponse, String> {
    let approved = state
        .pending_approvals
        .lock()
        .await
        .remove(&action_id)
        .ok_or_else(|| "pending approval not found or already consumed".to_string())?;
    let outcome = state
        .runner
        .execute_approved(&session_id, approved, &state.cancel_flag)
        .await?;
    if matches!(outcome, StepOutcome::Executed { .. }) {
        let _ = state
            .store
            .update_status(&session_id, SessionStatus::Complete)
            .await;
    }
    let session = state
        .store
        .get_session(&session_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(StepResponse { session, outcome })
}

#[tauri::command]
async fn reject_action(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<SessionRecord, String> {
    state
        .store
        .update_status(&session_id, SessionStatus::Blocked)
        .await
        .map_err(|e| e.to_string())?;
    state
        .store
        .get_session(&session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn stop_task(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<SessionRecord, String> {
    state.cancel_flag.store(true, Ordering::SeqCst);
    state
        .store
        .update_status(&session_id, SessionStatus::Stopped)
        .await
        .map_err(|e| e.to_string())?;
    state
        .store
        .get_session(&session_id)
        .await
        .map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("snippy"));
            let db_path = data_dir.join("sessions.db");

            let win_storage = platform_windows::WindowsSecureStorage;
            let store = tauri::async_runtime::block_on(async {
                SqliteSessionStore::open_with_secure_storage(
                    &db_path,
                    &win_storage,
                    "snippy.session_db.key",
                )
                .await
            })
            .expect("failed to initialize encrypted session store");

            let store = Arc::new(store);
            let platform = Arc::new(DemoPlatform {
                storage: MockSecureStorage::default(),
            });
            let provider = Arc::new(DemoProvider);
            let runner = AgentRunner::new(platform, provider, store.clone());

            app.manage(AppState {
                store,
                runner,
                cancel_flag: Arc::new(AtomicBool::new(false)),
                pending_approvals: Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_windows,
            list_sessions,
            get_session,
            delete_session,
            start_and_step,
            step_session,
            approve_action,
            reject_action,
            stop_task,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Snippy");
}
