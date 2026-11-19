use async_trait::async_trait;
use chrono::Utc;
use platform_contract::{
    ActionResult, ApprovedAction, ExpectedState, Observation, PROTOCOL_VERSION, PermissionRequest,
    PermissionState, PlatformAdapter, PlatformError, Scope, SecureStorage, TargetKind,
    Verification,
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
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible,
    };
    use windows::core::{PCWSTR, PWSTR};

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
        // Verify the selected window still exists if it's a real hwnd scope
        if scope.scope_id.starts_with("hwnd:") {
            let still_open = list_visible_windows()
                .iter()
                .any(|w| w.scope_id == scope.scope_id);
            if !still_open {
                return Err(PlatformError::TargetNotFound(format!(
                    "window {} is no longer visible",
                    scope.window_title
                )));
            }
        }

        // shortcut: full UIA semantic tree walk deferred to Phase 3; hash scope + timestamp for now
        let mut hasher = Sha256::new();
        hasher.update(scope.scope_id.as_bytes());
        hasher.update(scope.window_title.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        Ok(Observation {
            protocol_version: PROTOCOL_VERSION,
            scope: scope.clone(),
            active_window_proof: scope.scope_id.clone(),
            semantic_elements: Vec::new(),
            visual_context_summary: Some(format!("Selected window: {}", scope.window_title)),
            observation_hash: hash,
            observed_at: Utc::now(),
        })
    }

    async fn execute_action(&self, _action: &ApprovedAction) -> Result<ActionResult, PlatformError> {
        // shortcut: Windows UIA / input execution deferred to Phase 3
        Err(PlatformError::UnsupportedControl(
            "Windows UIA action execution not yet enabled in Phase 2".into(),
        ))
    }

    async fn verify_result(&self, _expected: &ExpectedState) -> Verification {
        Verification::Blocked {
            reason: "Windows UIA verification not yet enabled in Phase 2".into(),
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
