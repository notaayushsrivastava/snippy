#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_core::mocks::MockSecureStorage;
use agent_core::provider::{
    AgentInput, AgentProposal, ProposalStatus, ProposedAction, ProviderAdapter, ProviderError,
    ScreenAnnotation,
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
use provider_gemini::live::GeminiLiveProvider;
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
                annotations: Vec::new(),
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
                annotations: Vec::new(),
            })
        }
    }
}

struct AppState {
    store: Arc<SqliteSessionStore>,
    demo_runner: AgentRunner<DemoPlatform, DemoProvider, SqliteSessionStore>,
    cancel_flag: Arc<AtomicBool>,
    video_stream_active: Arc<AtomicBool>,
    live_provider: Mutex<Option<Arc<GeminiLiveProvider>>>,
    pending_approvals: Mutex<HashMap<String, ApprovedAction>>,
}

#[derive(Serialize)]
struct StepResponse {
    session: SessionRecord,
    outcome: StepOutcome,
}

fn secret_key_name(provider: &str) -> Result<String, String> {
    match provider {
        "openai" => Ok("snippy.openai.api_key".into()),
        "gemini" | "gemini_live" => Ok("snippy.gemini.api_key".into()),
        _ => Err(format!("unsupported provider: {provider}")),
    }
}

#[tauri::command]
fn list_windows() -> Vec<Scope> {
    platform_windows::list_visible_windows()
}

fn env_key_name(provider: &str) -> Option<&'static str> {
    match provider {
        "openai" => Some("OPENAI_API_KEY"),
        "gemini" | "gemini_live" => Some("GEMINI_API_KEY"),
        _ => None,
    }
}

fn read_env_file_key(var_name: &str) -> Option<String> {
    let manifest_env = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../.env");
    for candidate in [manifest_env, ".env", "../../.env", "../../../.env"] {
        if let Ok(content) = std::fs::read_to_string(candidate) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') || trimmed.is_empty() {
                    continue;
                }
                if let Some((k, v)) = trimmed.split_once('=') {
                    if k.trim() == var_name {
                        let val = v.trim().trim_matches('"').trim_matches('\'');
                        if !val.is_empty() {
                            return Some(val.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

async fn resolve_provider_key(provider: &str) -> Result<String, String> {
    if cfg!(debug_assertions) {
        if let Some(env_var) = env_key_name(provider) {
            if let Some(val) = read_env_file_key(env_var) {
                return Ok(val);
            }
            if let Ok(val) = std::env::var(env_var) {
                let cleaned = val.trim().trim_matches('"').trim_matches('\'');
                if !cleaned.is_empty() {
                    return Ok(cleaned.to_string());
                }
            }
        }
    }
    let key_name = secret_key_name(provider)?;
    if let Some(val) = WindowsSecureStorage
        .get_secret(&key_name)
        .await
        .map_err(|e| e.to_string())?
    {
        let cleaned = val.trim().trim_matches('"').trim_matches('\'');
        if !cleaned.is_empty() {
            return Ok(cleaned.to_string());
        }
    }
    if let Some(env_var) = env_key_name(provider) {
        if let Ok(val) = std::env::var(env_var) {
            let cleaned = val.trim().trim_matches('"').trim_matches('\'');
            if !cleaned.is_empty() {
                return Ok(cleaned.to_string());
            }
        }
        if let Some(val) = read_env_file_key(env_var) {
            return Ok(val);
        }
    }
    Err(format!("{provider} API key not configured"))
}

#[tauri::command]
async fn get_default_provider() -> String {
    if resolve_provider_key("gemini").await.is_ok() {
        "gemini_live".into()
    } else if resolve_provider_key("openai").await.is_ok() {
        "openai".into()
    } else {
        "demo".into()
    }
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
    Ok(resolve_provider_key(&provider).await.is_ok())
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

async fn get_or_init_live_provider(state: &AppState) -> Result<Arc<GeminiLiveProvider>, String> {
    let mut guard = state.live_provider.lock().await;
    if let Some(p) = guard.as_ref() {
        return Ok(p.clone());
    }
    let key = resolve_provider_key("gemini").await?;
    let capture_fn = Arc::new(|scope_id: &str| {
        platform_windows::capture_scope_jpeg_base64(scope_id).ok()
    });
    let live = Arc::new(GeminiLiveProvider::new(key, capture_fn));
    *guard = Some(live.clone());
    Ok(live)
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
    let prov = provider.as_deref().unwrap_or("demo");
    if prov == "gemini_live" {
        if let Ok(live) = get_or_init_live_provider(&state).await {
            state.video_stream_active.store(true, Ordering::SeqCst);
            let active = state.video_stream_active.clone();
            let cancel = state.cancel_flag.clone();
            let scope_id = scope.scope_id.clone();
            tauri::async_runtime::spawn(async move {
                while active.load(Ordering::SeqCst) && !cancel.load(Ordering::SeqCst) {
                    if let Ok(frame) = platform_windows::capture_scope_jpeg_base64(&scope_id) {
                        if live.push_video_frame(&frame).await.is_err() {
                            break;
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                }
            });
        }
    }
    let session = state
        .store
        .create_session(task, scope)
        .await
        .map_err(|e| e.to_string())?;
    step_session_internal(&app, &state, &session.session_id, prov).await
}

#[tauri::command]
async fn continue_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    follow_up: String,
    provider: Option<String>,
) -> Result<StepResponse, String> {
    state.cancel_flag.store(false, Ordering::SeqCst);
    state
        .store
        .update_task(&session_id, follow_up)
        .await
        .map_err(|e| e.to_string())?;
    step_session_internal(
        &app,
        &state,
        &session_id,
        provider.as_deref().unwrap_or("demo"),
    )
    .await
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
            let key = resolve_provider_key("openai").await?;
            let runner = AgentRunner::new(
                Arc::new(WindowsPlatformAdapter::new()),
                Arc::new(OpenAiProvider::new(key)),
                state.store.clone(),
            );
            runner.step_once(session_id, &state.cancel_flag).await?
        }
        "gemini" => {
            let key = resolve_provider_key("gemini").await?;
            let runner = AgentRunner::new(
                Arc::new(WindowsPlatformAdapter::new()),
                Arc::new(GeminiProvider::new(key)),
                state.store.clone(),
            );
            runner.step_once(session_id, &state.cancel_flag).await?
        }
        "gemini_live" => {
            let live = get_or_init_live_provider(state).await?;
            let runner = AgentRunner::new(
                Arc::new(WindowsPlatformAdapter::new()),
                live,
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

    let outcome_annotations = match &outcome {
        StepOutcome::Executed { annotations, .. }
        | StepOutcome::AwaitingApproval { annotations, .. }
        | StepOutcome::Complete { annotations, .. } => annotations.clone(),
        _ => Vec::new(),
    };
    if !outcome_annotations.is_empty() {
        let _ = draw_annotations(app.clone(), outcome_annotations);
    }

    if !matches!(outcome, StepOutcome::Executed { .. }) {
        state.video_stream_active.store(false, Ordering::SeqCst);
    }

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
        "openai" | "gemini" | "gemini_live" => {
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

    if matches!(outcome, StepOutcome::Executed { .. }) && provider.as_deref().unwrap_or("demo") == "demo" {
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
fn draw_annotations(app: AppHandle, annotations: Vec<ScreenAnnotation>) -> Result<(), String> {
    let Some(overlay) = app.get_webview_window("overlay") else {
        return Ok(());
    };
    let (origin_x, origin_y, scale) = if let Ok(Some(mon)) = overlay.current_monitor() {
        let size = mon.size();
        let pos = mon.position();
        let _ = overlay.set_position(PhysicalPosition::new(pos.x, pos.y));
        let _ = overlay.set_size(PhysicalSize::new(size.width, size.height));
        (pos.x, pos.y, mon.scale_factor().max(0.5))
    } else {
        (0, 0, 1.0)
    };
    let logical_annotations: Vec<ScreenAnnotation> = annotations
        .into_iter()
        .map(|mut ann| {
            ann.x = ((ann.x - origin_x) as f64 / scale).round() as i32;
            ann.y = ((ann.y - origin_y) as f64 / scale).round() as i32;
            ann.width = ann.width.map(|w| (w as f64 / scale).round() as i32);
            ann.height = ann.height.map(|h| (h as f64 / scale).round() as i32);
            ann.x2 = ann.x2.map(|x| ((x - origin_x) as f64 / scale).round() as i32);
            ann.y2 = ann.y2.map(|y| ((y - origin_y) as f64 / scale).round() as i32);
            ann
        })
        .collect();
    let payload = serde_json::to_string(&logical_annotations).map_err(|e| e.to_string())?;
    let _ = overlay.eval(format!(
        "window.dispatchEvent(new CustomEvent('snippy-overlay-draw', {{ detail: {payload} }}));"
    ));
    overlay.show().map_err(|e| e.to_string())
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
    state.video_stream_active.store(false, Ordering::SeqCst);
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
                video_stream_active: Arc::new(AtomicBool::new(false)),
                live_provider: Mutex::new(None),
                pending_approvals: Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_windows,
            get_default_provider,
            set_provider_key,
            has_provider_key,
            delete_provider_key,
            show_overlay_highlight,
            draw_annotations,
            hide_overlay_highlight,
            list_sessions,
            get_session,
            delete_session,
            start_and_step,
            continue_session,
            step_session,
            approve_action,
            reject_action,
            stop_task,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Snippy");
}
