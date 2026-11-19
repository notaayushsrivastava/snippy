use async_trait::async_trait;
use chrono::Utc;
use platform_contract::{
    ActionKind, ActionResult, ApprovedAction, Bounds, ExpectedState, Observation,
    PROTOCOL_VERSION, PermissionRequest, PermissionState, PlatformAdapter, PlatformError, Scope,
    SecureStorage, SemanticElement, TargetKind, Verification,
};
use sha2::{Digest, Sha256};

#[derive(Debug, Default)]
pub struct WindowsSecureStorage;

#[cfg(windows)]
mod win_impl {
    use super::*;
    use std::ffi::c_void;
    use std::ptr;
    use windows::Win32::Foundation::{BOOL, ERROR_NOT_FOUND, HWND, LPARAM};
    use windows::Win32::Security::Credentials::{
        CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree,
        CredReadW, CredWriteW,
    };
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern,
        IUIAutomationTreeWalker, IUIAutomationValuePattern, UIA_InvokePatternId,
        UIA_ValuePatternId,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible,
    };
    use windows::core::{BSTR, Interface, PCWSTR, PWSTR};

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn cred_read(key: &str) -> Result<Option<String>, PlatformError> {
        let target = to_wide(key);
        let mut pcred: *mut CREDENTIALW = ptr::null_mut();
        let ok = unsafe { CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, 0, &mut pcred) };
        if let Err(err) = ok {
            if err.code() == ERROR_NOT_FOUND.to_hresult() {
                return Ok(None);
            }
            return Err(PlatformError::SecureStorage(err.to_string()));
        }
        if pcred.is_null() {
            return Ok(None);
        }
        let result = unsafe {
            let cred = &*pcred;
            let bytes =
                std::slice::from_raw_parts(cred.CredentialBlob, cred.CredentialBlobSize as usize);
            String::from_utf8(bytes.to_vec())
                .map(Some)
                .map_err(|e| PlatformError::SecureStorage(e.to_string()))
        };
        unsafe { CredFree(pcred as *const c_void) };
        result
    }

    pub fn cred_write(key: &str, value: &str) -> Result<(), PlatformError> {
        let mut target = to_wide(key);
        let mut blob = value.as_bytes().to_vec();
        let cred = CREDENTIALW {
            Flags: Default::default(),
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            Comment: PWSTR::null(),
            LastWritten: Default::default(),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: ptr::null_mut(),
            TargetAlias: PWSTR::null(),
            UserName: PWSTR::null(),
        };
        unsafe { CredWriteW(&cred, 0) }.map_err(|e| PlatformError::SecureStorage(e.to_string()))
    }

    pub fn cred_delete(key: &str) -> Result<(), PlatformError> {
        let target = to_wide(key);
        match unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, 0) } {
            Ok(()) => Ok(()),
            Err(err) if err.code() == ERROR_NOT_FOUND.to_hresult() => Ok(()),
            Err(err) => Err(PlatformError::SecureStorage(err.to_string())),
        }
    }

    unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return BOOL(1);
        }
        let len = unsafe { GetWindowTextLengthW(hwnd) };
        if len <= 0 {
            return BOOL(1);
        }
        let mut buf = vec![0u16; (len as usize) + 1];
        let read = unsafe { GetWindowTextW(hwnd, &mut buf) };
        if read <= 0 {
            return BOOL(1);
        }
        let title = String::from_utf16_lossy(&buf[..read as usize]);
        let mut pid: u32 = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };

        let list = unsafe { &mut *(lparam.0 as *mut Vec<Scope>) };
        list.push(Scope {
            scope_id: format!("hwnd:{}", hwnd.0 as usize),
            target_kind: TargetKind::Window,
            app_name: title.clone(),
            window_title: title,
            process_id: Some(pid),
        });
        BOOL(1)
    }

    pub fn list_windows() -> Vec<Scope> {
        let mut scopes: Vec<Scope> = Vec::new();
        let lparam = LPARAM((&mut scopes as *mut Vec<Scope>) as isize);
        let _ = unsafe { EnumWindows(Some(enum_window_proc), lparam) };
        scopes
    }

    fn parse_hwnd(scope_id: &str) -> Result<HWND, PlatformError> {
        let raw = scope_id
            .strip_prefix("hwnd:")
            .and_then(|s| s.parse::<usize>().ok())
            .ok_or_else(|| PlatformError::TargetNotFound("invalid hwnd scope_id".into()))?;
        Ok(HWND(raw as *mut c_void))
    }

    fn init_uia() -> Result<IUIAutomation, PlatformError> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| PlatformError::UnsupportedControl(e.to_string()))
        }
    }

    fn element_locator(el: &IUIAutomationElement, index: usize) -> String {
        if let Ok(id) = unsafe { el.CurrentAutomationId() } {
            let s = id.to_string();
            if !s.is_empty() {
                return format!("uia-id:{s}");
            }
        }
        format!("uia-idx:{index}")
    }

    fn collect_elements(
        walker: &IUIAutomationTreeWalker,
        el: &IUIAutomationElement,
        depth: usize,
        out: &mut Vec<SemanticElement>,
    ) {
        if depth > 5 || out.len() >= 80 {
            return;
        }
        let name = unsafe { el.CurrentName() }
            .map(|b| b.to_string())
            .unwrap_or_default();
        let is_enabled = unsafe { el.CurrentIsEnabled() }
            .map(|b| b.as_bool())
            .unwrap_or(false);
        let is_secret = unsafe { el.CurrentIsPassword() }
            .map(|b| b.as_bool())
            .unwrap_or(false);
        let control_type = unsafe { el.CurrentControlType() }.map(|c| c.0).unwrap_or(0);
        let rect = unsafe { el.CurrentBoundingRectangle() }.ok();

        if !name.is_empty() || is_secret {
            let idx = out.len();
            out.push(SemanticElement {
                locator: element_locator(el, idx),
                role: format!("uia:{control_type}"),
                label: if is_secret {
                    "Sensitive field".into()
                } else {
                    name
                },
                bounds: rect.map(|r| Bounds {
                    x: r.left,
                    y: r.top,
                    width: (r.right - r.left).max(0) as u32,
                    height: (r.bottom - r.top).max(0) as u32,
                }),
                is_secret,
                is_enabled,
            });
        }

        if let Ok(mut child) = unsafe { walker.GetFirstChildElement(el) } {
            loop {
                collect_elements(walker, &child, depth + 1, out);
                if out.len() >= 80 {
                    break;
                }
                match unsafe { walker.GetNextSiblingElement(&child) } {
                    Ok(next) => child = next,
                    Err(_) => break,
                }
            }
        }
    }

    fn find_element_by_locator(
        walker: &IUIAutomationTreeWalker,
        el: &IUIAutomationElement,
        target_locator: &str,
        depth: usize,
        counter: &mut usize,
    ) -> Option<IUIAutomationElement> {
        if depth > 5 || *counter >= 80 {
            return None;
        }
        let name = unsafe { el.CurrentName() }
            .map(|b| b.to_string())
            .unwrap_or_default();
        let is_secret = unsafe { el.CurrentIsPassword() }
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if !name.is_empty() || is_secret {
            let loc = element_locator(el, *counter);
            *counter += 1;
            if loc == target_locator {
                return Some(el.clone());
            }
        }
        if let Ok(mut child) = unsafe { walker.GetFirstChildElement(el) } {
            loop {
                if let Some(found) =
                    find_element_by_locator(walker, &child, target_locator, depth + 1, counter)
                {
                    return Some(found);
                }
                match unsafe { walker.GetNextSiblingElement(&child) } {
                    Ok(next) => child = next,
                    Err(_) => break,
                }
            }
        }
        None
    }

    pub fn observe_hwnd(scope: &Scope) -> Result<Vec<SemanticElement>, PlatformError> {
        let hwnd = parse_hwnd(&scope.scope_id)?;
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return Err(PlatformError::TargetNotFound(format!(
                "window {} is no longer visible",
                scope.window_title
            )));
        }
        let uia = init_uia()?;
        let root = unsafe { uia.ElementFromHandle(hwnd) }
            .map_err(|e| PlatformError::TargetNotFound(e.to_string()))?;
        let walker = unsafe { uia.ControlViewWalker() }
            .map_err(|e| PlatformError::UnsupportedControl(e.to_string()))?;
        let mut elements = Vec::new();
        collect_elements(&walker, &root, 0, &mut elements);
        Ok(elements)
    }

    pub fn execute_uia(action: &ApprovedAction) -> Result<String, PlatformError> {
        let hwnd = parse_hwnd(&action.scope_id)?;
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return Err(PlatformError::FocusChanged(
                "target window is no longer visible".into(),
            ));
        }
        let locator = action
            .target
            .semantic_locator
            .as_deref()
            .ok_or_else(|| PlatformError::StaleLocator("semantic_locator required".into()))?;

        let uia = init_uia()?;
        let root = unsafe { uia.ElementFromHandle(hwnd) }
            .map_err(|e| PlatformError::TargetNotFound(e.to_string()))?;
        let walker = unsafe { uia.ControlViewWalker() }
            .map_err(|e| PlatformError::UnsupportedControl(e.to_string()))?;
        let mut counter = 0usize;
        let el = find_element_by_locator(&walker, &root, locator, 0, &mut counter)
            .ok_or_else(|| PlatformError::StaleLocator(format!("locator not found: {locator}")))?;

        let is_secret = unsafe { el.CurrentIsPassword() }
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if is_secret {
            return Err(PlatformError::PermissionDenied(
                "refusing to automate password/secret field".into(),
            ));
        }

        match action.kind {
            ActionKind::Focus => {
                unsafe { el.SetFocus() }
                    .map_err(|e| PlatformError::UnsupportedControl(e.to_string()))?;
                Ok(format!("Focused {locator}"))
            }
            ActionKind::Invoke => {
                let pattern: IUIAutomationInvokePattern = unsafe {
                    el.GetCurrentPattern(UIA_InvokePatternId)
                        .and_then(|u| u.cast())
                }
                .map_err(|e| PlatformError::UnsupportedControl(e.to_string()))?;
                unsafe { pattern.Invoke() }
                    .map_err(|e| PlatformError::UnsupportedControl(e.to_string()))?;
                Ok(format!("Invoked {locator}"))
            }
            ActionKind::SetNonSecretValue => {
                let text = action
                    .arguments
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        PlatformError::UnsupportedControl("missing arguments.text".into())
                    })?;
                let pattern: IUIAutomationValuePattern = unsafe {
                    el.GetCurrentPattern(UIA_ValuePatternId)
                        .and_then(|u| u.cast())
                }
                .map_err(|e| PlatformError::UnsupportedControl(e.to_string()))?;
                let bstr = BSTR::from(text);
                unsafe { pattern.SetValue(&bstr) }
                    .map_err(|e| PlatformError::UnsupportedControl(e.to_string()))?;
                Ok(format!("Updated non-secret value on {locator}"))
            }
            other => Err(PlatformError::UnsupportedControl(format!(
                "{other:?} not yet implemented in Phase 3"
            ))),
        }
    }
}

pub fn list_visible_windows() -> Vec<Scope> {
    #[cfg(windows)]
    {
        win_impl::list_windows()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[async_trait]
impl SecureStorage for WindowsSecureStorage {
    async fn get_secret(&self, key: &str) -> Result<Option<String>, PlatformError> {
        #[cfg(windows)]
        {
            win_impl::cred_read(key)
        }
        #[cfg(not(windows))]
        {
            let _ = key;
            Err(PlatformError::UnsupportedControl("windows only".into()))
        }
    }

    async fn set_secret(&self, key: &str, value: &str) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            win_impl::cred_write(key, value)
        }
        #[cfg(not(windows))]
        {
            let _ = (key, value);
            Err(PlatformError::UnsupportedControl("windows only".into()))
        }
    }

    async fn delete_secret(&self, key: &str) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            win_impl::cred_delete(key)
        }
        #[cfg(not(windows))]
        {
            let _ = key;
            Err(PlatformError::UnsupportedControl("windows only".into()))
        }
    }
}

#[derive(Debug, Default)]
pub struct WindowsPlatformAdapter {
    storage: WindowsSecureStorage,
}

impl WindowsPlatformAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PlatformAdapter for WindowsPlatformAdapter {
    async fn request_permissions(&self, _request: PermissionRequest) -> PermissionState {
        PermissionState::Granted
    }

    async fn observe_target(&self, scope: &Scope) -> Result<Observation, PlatformError> {
        #[cfg(windows)]
        let semantic_elements = if scope.scope_id.starts_with("hwnd:") {
            win_impl::observe_hwnd(scope)?
        } else {
            Vec::new()
        };
        #[cfg(not(windows))]
        let semantic_elements = Vec::new();

        let mut hasher = Sha256::new();
        hasher.update(scope.scope_id.as_bytes());
        hasher.update(scope.window_title.as_bytes());
        if let Ok(json_bytes) = serde_json::to_vec(&semantic_elements) {
            hasher.update(json_bytes);
        }
        let hash = format!("{:x}", hasher.finalize());

        Ok(Observation {
            protocol_version: PROTOCOL_VERSION,
            scope: scope.clone(),
            active_window_proof: scope.scope_id.clone(),
            semantic_elements,
            visual_context_summary: Some(format!("Selected window: {}", scope.window_title)),
            observation_hash: hash,
            observed_at: Utc::now(),
        })
    }

    async fn execute_action(&self, action: &ApprovedAction) -> Result<ActionResult, PlatformError> {
        #[cfg(windows)]
        let summary = win_impl::execute_uia(action)?;
        #[cfg(not(windows))]
        let summary = {
            let _ = action;
            return Err(PlatformError::UnsupportedControl("windows only".into()));
        };

        Ok(ActionResult {
            action_id: action.action_id.clone(),
            executed_at: Utc::now(),
            summary,
        })
    }

    async fn verify_result(&self, expected: &ExpectedState) -> Verification {
        // Re-check that the target window is still visible after action execution
        if expected.scope_id.starts_with("hwnd:") {
            let still_open = list_visible_windows()
                .iter()
                .any(|w| w.scope_id == expected.scope_id);
            if !still_open {
                return Verification::NotConfirmed {
                    reason: "target window closed or lost".into(),
                };
            }
        }
        Verification::Confirmed {
            evidence: expected.expected_postcondition.clone(),
        }
    }

    fn secure_storage(&self) -> &dyn SecureStorage {
        &self.storage
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn credential_manager_round_trip_and_delete() {
        let storage = WindowsSecureStorage;
        let key = "snippy-test-key-phase2";
        storage.set_secret(key, "test-secret-value").await.unwrap();
        let read = storage.get_secret(key).await.unwrap();
        assert_eq!(read.as_deref(), Some("test-secret-value"));
        storage.delete_secret(key).await.unwrap();
        let after_delete = storage.get_secret(key).await.unwrap();
        assert_eq!(after_delete, None);
    }

    #[test]
    fn list_visible_windows_runs_without_panicking() {
        let _windows = list_visible_windows();
    }
}
