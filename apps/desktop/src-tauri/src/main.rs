#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_core::mocks::MockSecureStorage;
use agent_core::provider::{
    AgentInput, AgentProposal, ProposalStatus, ProposedAction, ProviderAdapter, ProviderError,
};
use agent_core::runner::{AgentRunner, StepOutcome};
use async_trait::async_trait;
use chrono::Utc;
use platform_contract::{
    ActionKind, ActionResult, ApprovedAction, Bounds, ExpectedState, Observation,
    PROTOCOL_VERSION, PermissionRequest, PermissionState, PlatformAdapter, PlatformError,
    RiskCategory, Scope, SecureStorage, SemanticElement, TargetLocator, Verification,
};
use platform_windows::{WindowsPlatformAdapter, WindowsSecureStorage};
use provider_gemini::GeminiProvider;
use provider_openai::OpenAiProvider;
use serde::Serialize;
use serde_json::json;
use session_store::{SessionRecord, SessionStatus, SessionStore, SqliteSessionStore};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, State};
use tokio::sync::Mutex;

/// Demo platform adapter when running without a live provider key.
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
    demo_runner: AgentRunner<DemoPlatform, DemoProvider, SqliteSessionStore>,
    cancel_flag: Arc<AtomicBool>,
    pending_approvals: Mutex<HashMap<String, ApprovedAction>>,
}

#[derive(Serialize)]
struct StepResponse {
    session: SessionRecord,
    outcome: StepOutcome,
}

fn secret_key_name(provider: &str) -> Result<String, String> {
    match provider {
        "openai" | "gemini" => Ok(format!("snippy.{provider}.api_key")),
        _ => Err(format!("unsupported provider: {provider}")),
    }
}

#[tauri::command]
fn list_windows() -> Vec<Scope> {
    platform_windows::list_visible_windows()
}

#[tauri::command]
async fn set_provider_key(provider: String, api_key: String) -> Result<(), String> {
    let key_name = secret_key_name(&provider)?;
    WindowsSecureStorage
        .set_secret(&key_name, api_key.trim())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn has_provider_key(provider: String) -> Result<bool, String> {
    let key_name = secret_key_name(&provider)?;
    let val = WindowsSecureStorage
        .get_secret(&key_name)
        .await
        .map_err(|e| e.to_string())?;
    Ok(val.is_some_and(|s| !s.is_empty()))
}

#[tauri::command]
async fn delete_provider_key(provider: String) -> Result<(), String> {
    let key_name = secret_key_name(&provider)?;
    WindowsSecureStorage
        .delete_secret(&key_name)
        .await
        .map_err(|e| e.to_string())
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
    app: AppHandle,
    state: State<'_, AppState>,
    task: String,
    scope: Scope,
    provider: Option<String>,
) -> Result<StepResponse, String> {
    state.cancel_flag.store(false, Ordering::SeqCst);
    let session = state
        .store
        .create_session(task, scope)
        .await
        .map_err(|e| e.to_string())?;
    step_session_internal(&app, &state, &session.session_id, provider.as_deref().unwrap_or("demo")).await
}

#[tauri::command]
async fn step_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    provider: Option<String>,
) -> Result<StepResponse, String> {
    step_session_internal(&app, &state, &session_id, provider.as_deref().unwrap_or("demo")).await
}

async fn step_session_internal(
    app: &AppHandle,
    state: &AppState,
    session_id: &str,
    provider: &str,
) -> Result<StepResponse, String> {
    let outcome = match provider {
        "openai" => {
            let key = WindowsSecureStorage
                .get_secret(&secret_key_name("openai")?)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "OpenAI API key not configured".to_string())?;
            let runner = AgentRunner::new(
                Arc::new(WindowsPlatformAdapter::new()),
                Arc::new(OpenAiProvider::new(key)),
                state.store.clone(),
            );
            runner.step_once(session_id, &state.cancel_flag).await?
        }
        "gemini" => {
            let key = WindowsSecureStorage
                .get_secret(&secret_key_name("gemini")?)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "Gemini API key not configured".to_string())?;
            let runner = AgentRunner::new(
                Arc::new(WindowsPlatformAdapter::new()),
                Arc::new(GeminiProvider::new(key)),
                state.store.clone(),
            );
            runner.step_once(session_id, &state.cancel_flag).await?
        }
        _ => {
            state
                .demo_runner
                .step_once(session_id, &state.cancel_flag)
                .await?
        }
    };

    if let StepOutcome::AwaitingApproval { ref action, .. } = outcome {
        state
            .pending_approvals
            .lock()
            .await
            .insert(action.action_id.clone(), (**action).clone());
        if let Some(hint) = action.target.coordinate_hint {
            let _ = show_overlay_highlight(
                app.clone(),
                Bounds {
                    x: hint.x.saturating_sub(40),
                    y: hint.y.saturating_sub(20),
                    width: 180,
                    height: 56,
                },
                action.reason.clone(),
                1,
            );
        }
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
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    action_id: String,
    provider: Option<String>,
) -> Result<StepResponse, String> {
    let _ = hide_overlay_highlight(app);
    let approved = state
        .pending_approvals
        .lock()
        .await
        .remove(&action_id)
        .ok_or_else(|| "pending approval not found or already consumed".to_string())?;

    let outcome = match provider.as_deref().unwrap_or("demo") {
        "openai" | "gemini" => {
            let runner = AgentRunner::new(
                Arc::new(WindowsPlatformAdapter::new()),
                Arc::new(DemoProvider),
                state.store.clone(),
            );
            runner
                .execute_approved(&session_id, approved, &state.cancel_flag)
                .await?
        }
        _ => {
            state
                .demo_runner
                .execute_approved(&session_id, approved, &state.cancel_flag)
                .await?
        }
    };

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
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<SessionRecord, String> {
    let _ = hide_overlay_highlight(app);
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
fn show_overlay_highlight(
    app: AppHandle,
    bounds: Bounds,
    label: String,
    step_number: u32,
) -> Result<(), String> {
    let Some(overlay) = app.get_webview_window("overlay") else {
        return Ok(());
    };
    let _ = overlay.set_position(PhysicalPosition::new(bounds.x, bounds.y));
    let _ = overlay.set_size(PhysicalSize::new(
        bounds.width.max(120),
        bounds.height.max(40),
    ));
    let payload = json!({ "label": label, "step_number": step_number });
    let _ = overlay.eval(format!(
        "window.dispatchEvent(new CustomEvent('snippy-overlay-update', {{ detail: {payload} }}));"
    ));
    overlay.show().map_err(|e| e.to_string())
}

#[tauri::command]
fn hide_overlay_highlight(app: AppHandle) -> Result<(), String> {
    if let Some(overlay) = app.get_webview_window("overlay") {
        overlay.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn stop_task(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<SessionRecord, String> {
    state.cancel_flag.store(true, Ordering::SeqCst);
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.hide();
    }
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

            let win_storage = WindowsSecureStorage;
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
            let demo_runner = AgentRunner::new(
                Arc::new(DemoPlatform {
                    storage: MockSecureStorage::default(),
                }),
                Arc::new(DemoProvider),
                store.clone(),
            );

            if let Some(overlay) = app.get_webview_window("overlay") {
                let _ = overlay.set_ignore_cursor_events(true);
            }

            app.manage(AppState {
                store,
                demo_runner,
                cancel_flag: Arc::new(AtomicBool::new(false)),
                pending_approvals: Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_windows,
            set_provider_key,
            has_provider_key,
            delete_provider_key,
            show_overlay_highlight,
            hide_overlay_highlight,
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
