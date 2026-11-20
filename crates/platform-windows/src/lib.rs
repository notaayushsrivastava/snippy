pub mod office;

use async_trait::async_trait;
use chrono::Utc;
use platform_contract::{
    ActionKind, ActionResult, ApprovedAction, Bounds, ExpectedState, Observation,
    PermissionRequest, PermissionState, PlatformAdapter, PlatformError, Scope, SecureStorage,
    SemanticElement, TargetKind, Verification, PROTOCOL_VERSION,
};
use sha2::{Digest, Sha256};

#[derive(Debug, Default)]
pub struct WindowsSecureStorage;

#[cfg(windows)]
mod win_impl {
    use super::*;
    use std::ffi::c_void;
    use std::ptr;
    use windows::core::{Interface, BSTR, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{BOOL, ERROR_NOT_FOUND, HWND, LPARAM, POINT, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        SRCCOPY,
    };
    use windows::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
        CRED_TYPE_GENERIC,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern,
        IUIAutomationTreeWalker, IUIAutomationValuePattern, TreeScope_Descendants,
        UIA_InvokePatternId, UIA_ValuePatternId,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
        KEYEVENTF_UNICODE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
        MOUSEEVENTF_MOVE, MOUSEEVENTF_WHEEL, MOUSEINPUT, VIRTUAL_KEY, VK_BACK, VK_CONTROL,
        VK_ESCAPE, VK_RETURN, VK_TAB,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, EnumWindows, GetForegroundWindow, GetSystemMetrics, GetWindowRect,
        GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
        SetForegroundWindow, ShowWindow, SM_CXSCREEN, SM_CYSCREEN, SW_RESTORE,
    };

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
        if matches!(
            title.as_str(),
            "Snippy"
                | "Snippy Overlay"
                | "Program Manager"
                | "Windows Input Experience"
                | "Microsoft Text Input Application"
        ) {
            return BOOL(1);
        }
        let mut rect = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok()
            && ((rect.right - rect.left) < 50 || (rect.bottom - rect.top) < 50)
        {
            return BOOL(1);
        }
        let mut cloaked: u32 = 0;
        if unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED,
                &mut cloaked as *mut u32 as *mut c_void,
                std::mem::size_of::<u32>() as u32,
            )
        }
        .is_ok()
            && cloaked != 0
        {
            return BOOL(1);
        }
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

    pub fn capture_screen_jpeg(scope_id: &str) -> Result<String, PlatformError> {
        use base64::Engine as _;
        use image::codecs::jpeg::JpegEncoder;

        let (x, y, w, h) = if scope_id.starts_with("hwnd:") {
            let hwnd = parse_hwnd(scope_id)?;
            let mut rect = RECT::default();
            if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok()
                && rect.right > rect.left
                && rect.bottom > rect.top
            {
                (
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                )
            } else {
                (
                    0,
                    0,
                    unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(1),
                    unsafe { GetSystemMetrics(SM_CYSCREEN) }.max(1),
                )
            }
        } else {
            (
                0,
                0,
                unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(1),
                unsafe { GetSystemMetrics(SM_CYSCREEN) }.max(1),
            )
        };

        let hdc_screen = unsafe { GetDC(None) };
        let hdc_mem = unsafe { CreateCompatibleDC(hdc_screen) };
        let hbm = unsafe { CreateCompatibleBitmap(hdc_screen, w, h) };
        let old = unsafe { SelectObject(hdc_mem, hbm) };

        let ok = unsafe { BitBlt(hdc_mem, 0, 0, w, h, hdc_screen, x, y, SRCCOPY) }.is_ok();
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bgra = vec![0u8; (w as usize) * (h as usize) * 4];
        if ok {
            unsafe {
                GetDIBits(
                    hdc_mem,
                    hbm,
                    0,
                    h as u32,
                    Some(bgra.as_mut_ptr() as *mut c_void),
                    &mut bmi,
                    DIB_RGB_COLORS,
                )
            };
        }

        unsafe {
            SelectObject(hdc_mem, old);
            let _ = DeleteObject(hbm);
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);
        }

        if !ok {
            return Err(PlatformError::UnsupportedControl(
                "BitBlt capture failed".into(),
            ));
        }

        let mut rgb = Vec::with_capacity((w as usize) * (h as usize) * 3);
        for px in bgra.as_chunks::<4>().0 {
            rgb.push(px[2]);
            rgb.push(px[1]);
            rgb.push(px[0]);
        }

        let img = image::RgbImage::from_raw(w as u32, h as u32, rgb)
            .ok_or_else(|| PlatformError::UnsupportedControl("invalid capture buffer".into()))?;
        let resized = if w > 1024 {
            let new_h = ((h as f32) * (1024.0 / w as f32)).round() as u32;
            image::imageops::resize(&img, 1024, new_h, image::imageops::FilterType::Triangle)
        } else {
            img
        };

        let mut jpeg_bytes = Vec::new();
        let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, 65);
        encoder
            .encode_image(&resized)
            .map_err(|e| PlatformError::UnsupportedControl(e.to_string()))?;

        Ok(base64::engine::general_purpose::STANDARD.encode(jpeg_bytes))
    }

    pub fn list_windows() -> Vec<Scope> {
        let mut scopes: Vec<Scope> = vec![Scope {
            scope_id: "desktop:all".into(),
            target_kind: TargetKind::Window,
            app_name: "Desktop".into(),
            window_title: "All visible windows (Full access)".into(),
            process_id: None,
        }];
        let lparam = LPARAM((&mut scopes as *mut Vec<Scope>) as isize);
        let _ = unsafe { EnumWindows(Some(enum_window_proc), lparam) };
        scopes
    }

    pub(crate) fn resolve_target_hwnd(scope_id: &str) -> Result<HWND, PlatformError> {
        if scope_id == "desktop:all" {
            let fg = unsafe { GetForegroundWindow() };
            let mut buf = [0u16; 128];
            let len = unsafe { GetWindowTextW(fg, &mut buf) };
            let fg_title = if len > 0 {
                String::from_utf16_lossy(&buf[..len as usize])
            } else {
                String::new()
            };
            if !fg.0.is_null() && fg_title != "Snippy" && fg_title != "Snippy Overlay" {
                return Ok(fg);
            }
            for w in list_windows() {
                if w.scope_id.starts_with("hwnd:") {
                    return parse_hwnd(&w.scope_id);
                }
            }
            return Err(PlatformError::TargetNotFound(
                "no visible target window found on desktop".into(),
            ));
        }
        parse_hwnd(scope_id)
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
        if let Ok(r) = unsafe { el.CurrentBoundingRectangle() } {
            if r.right > r.left && r.bottom > r.top {
                let cx = (r.left + r.right) / 2;
                let cy = (r.top + r.bottom) / 2;
                return format!("uia-pos:{cx},{cy}");
            }
        }
        format!("uia-idx:{index}")
    }

    pub(crate) fn resolve_locator_bounds(scope_id: &str, locator: &str) -> Option<Bounds> {
        let hwnd = resolve_target_hwnd(scope_id).ok()?;
        let uia = init_uia().ok()?;
        let root = unsafe { uia.ElementFromHandle(hwnd) }.ok()?;
        let walker = unsafe { uia.ControlViewWalker() }.ok()?;
        let mut counter = 0usize;
        let el = find_element_by_locator(&walker, &root, locator, 0, &mut counter)?;
        let r = unsafe { el.CurrentBoundingRectangle() }.ok()?;
        if r.right > r.left && r.bottom > r.top {
            Some(Bounds {
                x: r.left,
                y: r.top,
                width: (r.right - r.left) as u32,
                height: (r.bottom - r.top) as u32,
            })
        } else {
            None
        }
    }

    pub(crate) fn parse_pos_locator(locator: &str) -> Option<(i32, i32)> {
        let coords = locator.strip_prefix("uia-pos:")?;
        let (xs, ys) = coords.split_once(',')?;
        let x = xs.trim().parse::<i32>().ok()?;
        let y = ys.trim().parse::<i32>().ok()?;
        Some((x, y))
    }

    fn collect_elements(
        walker: &IUIAutomationTreeWalker,
        el: &IUIAutomationElement,
        depth: usize,
        out: &mut Vec<SemanticElement>,
    ) {
        if depth > 18 || out.len() >= 100 {
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

        // Include named elements, secret fields, or editable/document controls (Edit=50004, Document=50030)
        let is_editable = control_type == 50004 || control_type == 50030;
        if !name.is_empty() || is_secret || is_editable {
            let idx = out.len();
            out.push(SemanticElement {
                locator: element_locator(el, idx),
                role: format!("uia:{control_type}"),
                label: if is_secret {
                    "Sensitive field".into()
                } else if name.is_empty() {
                    "Editable area".into()
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
                if out.len() >= 100 {
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
        if depth > 18 || *counter >= 100 {
            return None;
        }
        let name = unsafe { el.CurrentName() }
            .map(|b| b.to_string())
            .unwrap_or_default();
        let is_secret = unsafe { el.CurrentIsPassword() }
            .map(|b| b.as_bool())
            .unwrap_or(false);
        let control_type = unsafe { el.CurrentControlType() }.map(|c| c.0).unwrap_or(0);
        let is_editable = control_type == 50004 || control_type == 50030;
        if !name.is_empty() || is_secret || is_editable {
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
        let hwnd = resolve_target_hwnd(&scope.scope_id)?;
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return Err(PlatformError::TargetNotFound(format!(
                "window {} is no longer visible",
                scope.window_title
            )));
        }
        if unsafe { IsIconic(hwnd) }.as_bool() {
            let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
        let uia = init_uia()?;
        let root = unsafe { uia.ElementFromHandle(hwnd) }
            .map_err(|e| PlatformError::TargetNotFound(e.to_string()))?;

        // Use FindAll(TreeScope_Descendants) first so UWP/ApplicationFrameWindow children (e.g. Settings) are included
        if let Ok(cond) = unsafe { uia.CreateTrueCondition() } {
            if let Ok(arr) = unsafe { root.FindAll(TreeScope_Descendants, &cond) } {
                let len = unsafe { arr.Length() }.unwrap_or(0).min(160);
                let mut elements = Vec::new();
                for i in 0..len {
                    let Ok(el) = (unsafe { arr.GetElement(i) }) else {
                        continue;
                    };
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
                    let is_editable = control_type == 50004 || control_type == 50030;
                    if !name.is_empty() || is_secret || is_editable {
                        let idx = elements.len();
                        elements.push(SemanticElement {
                            locator: element_locator(&el, idx),
                            role: format!("uia:{control_type}"),
                            label: if is_secret {
                                "Sensitive field".into()
                            } else if name.is_empty() {
                                "Editable area".into()
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
                    if elements.len() >= 120 {
                        break;
                    }
                }
                if !elements.is_empty() {
                    return Ok(elements);
                }
            }
        }

        let walker = unsafe { uia.ControlViewWalker() }
            .map_err(|e| PlatformError::UnsupportedControl(e.to_string()))?;
        let mut elements = Vec::new();
        collect_elements(&walker, &root, 0, &mut elements);
        Ok(elements)
    }

    fn ensure_foreground(hwnd: HWND) -> Result<(), PlatformError> {
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return Err(PlatformError::FocusChanged(
                "target window is no longer visible".into(),
            ));
        }
        if unsafe { IsIconic(hwnd) }.as_bool() {
            let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
        }
        let _ = unsafe { BringWindowToTop(hwnd) };
        let _ = unsafe { SetForegroundWindow(hwnd) };
        for _ in 0..6 {
            if unsafe { GetForegroundWindow() } == hwnd {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        Err(PlatformError::FocusChanged(
            "target window did not retain foreground focus (is it running as Administrator?)"
                .into(),
        ))
    }

    fn send_key(vk: VIRTUAL_KEY, key_up: bool) -> INPUT {
        let mut flags = Default::default();
        if key_up {
            flags = KEYEVENTF_KEYUP;
        }
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn send_key_press(key_spec: &str) -> Result<(), PlatformError> {
        let lower = key_spec.trim().to_lowercase();
        let inputs: Vec<INPUT> = match lower.as_str() {
            "enter" | "return" => vec![send_key(VK_RETURN, false), send_key(VK_RETURN, true)],
            "tab" => vec![send_key(VK_TAB, false), send_key(VK_TAB, true)],
            "escape" | "esc" => vec![send_key(VK_ESCAPE, false), send_key(VK_ESCAPE, true)],
            "backspace" => vec![send_key(VK_BACK, false), send_key(VK_BACK, true)],
            "ctrl+a" | "ctrl+c" | "ctrl+v" | "ctrl+s" => {
                let ch = lower.as_bytes()[5].to_ascii_uppercase();
                let vk = VIRTUAL_KEY(ch as u16);
                vec![
                    send_key(VK_CONTROL, false),
                    send_key(vk, false),
                    send_key(vk, true),
                    send_key(VK_CONTROL, true),
                ]
            }
            _ => {
                return Err(PlatformError::UnsupportedControl(format!(
                    "unsupported key_press: {key_spec}"
                )));
            }
        };
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize != inputs.len() {
            return Err(PlatformError::UnsupportedControl(
                "SendInput failed to send all key events".into(),
            ));
        }
        Ok(())
    }

    fn send_scroll(delta_y: i32) -> Result<(), PlatformError> {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: delta_y as u32,
                    dwFlags: MOUSEEVENTF_WHEEL,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
        if sent != 1 {
            return Err(PlatformError::UnsupportedControl(
                "SendInput scroll failed".into(),
            ));
        }
        Ok(())
    }

    fn send_pointer_click(x: i32, y: i32) -> Result<(), PlatformError> {
        let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(1);
        let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) }.max(1);
        let norm_x = ((x as i64 * 65535) / sw as i64).clamp(0, 65535) as i32;
        let norm_y = ((y as i64 * 65535) / sh as i64).clamp(0, 65535) as i32;
        let inputs = [
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: norm_x,
                        dy: norm_y,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: norm_x,
                        dy: norm_y,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTDOWN | MOUSEEVENTF_ABSOLUTE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: norm_x,
                        dy: norm_y,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTUP | MOUSEEVENTF_ABSOLUTE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize != inputs.len() {
            return Err(PlatformError::UnsupportedControl(
                "SendInput pointer click failed".into(),
            ));
        }
        Ok(())
    }

    pub fn execute_uia(action: &ApprovedAction) -> Result<String, PlatformError> {
        if let Some(loc) = action.target.semantic_locator.as_deref() {
            if crate::office::is_office_locator(loc) {
                return crate::office::execute_office_action(action);
            }
        }
        if action.kind == ActionKind::Wait {
            let ms = action
                .arguments
                .get("ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(500)
                .min(2000);
            std::thread::sleep(std::time::Duration::from_millis(ms));
            return Ok(format!("Waited {ms}ms"));
        }

        let hwnd = resolve_target_hwnd(&action.scope_id)?;
        ensure_foreground(hwnd)?;

        match action.kind {
            ActionKind::KeyPress => {
                let key = action
                    .arguments
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        PlatformError::UnsupportedControl("missing arguments.key".into())
                    })?;
                send_key_press(key)?;
                return Ok(format!("Pressed {key}"));
            }
            ActionKind::Scroll => {
                let delta = action
                    .arguments
                    .get("delta_y")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(-240) as i32;
                send_scroll(delta)?;
                return Ok(format!("Scrolled by {delta}"));
            }
            ActionKind::Pointer => {
                let hint = action.target.coordinate_hint.ok_or_else(|| {
                    PlatformError::StaleLocator("coordinate_hint required for pointer".into())
                })?;
                send_pointer_click(hint.x, hint.y)?;
                return Ok(format!("Clicked at ({}, {})", hint.x, hint.y));
            }
            ActionKind::Wait => {
                let ms = action
                    .arguments
                    .get("ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(500)
                    .min(2000);
                std::thread::sleep(std::time::Duration::from_millis(ms));
                return Ok(format!("Waited {ms}ms"));
            }
            _ => {}
        }

        let Some(locator) = action.target.semantic_locator.as_deref() else {
            let Some(hint) = action.target.coordinate_hint else {
                return Err(PlatformError::StaleLocator(
                    "semantic_locator or coordinate_hint required".into(),
                ));
            };
            match action.kind {
                ActionKind::Focus | ActionKind::Invoke | ActionKind::Pointer => {
                    send_pointer_click(hint.x, hint.y)?;
                    return Ok(format!("Clicked ({}, {})", hint.x, hint.y));
                }
                ActionKind::SetNonSecretValue => {
                    send_pointer_click(hint.x, hint.y)?;
                    let text = action
                        .arguments
                        .get("text")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            PlatformError::UnsupportedControl("missing arguments.text".into())
                        })?;
                    let mut inputs = Vec::with_capacity(text.encode_utf16().count() * 2);
                    for code_unit in text.encode_utf16() {
                        inputs.push(INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: VIRTUAL_KEY(0),
                                    wScan: code_unit,
                                    dwFlags: KEYEVENTF_UNICODE,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        });
                        inputs.push(INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: VIRTUAL_KEY(0),
                                    wScan: code_unit,
                                    dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        });
                    }
                    let _ = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
                    return Ok(format!("Typed text at ({}, {})", hint.x, hint.y));
                }
                _ => {
                    return Err(PlatformError::UnsupportedControl(
                        "action requires semantic_locator".into(),
                    ));
                }
            }
        };

        let uia = init_uia()?;
        let root = unsafe { uia.ElementFromHandle(hwnd) }
            .map_err(|e| PlatformError::TargetNotFound(e.to_string()))?;
        let mut maybe_el = None;
        if let Some((cx, cy)) = parse_pos_locator(locator) {
            if let Ok(el) = unsafe { uia.ElementFromPoint(POINT { x: cx, y: cy }) } {
                maybe_el = Some(el);
            }
        }
        if maybe_el.is_none() {
            if let Ok(cond) = unsafe { uia.CreateTrueCondition() } {
                if let Ok(arr) = unsafe { root.FindAll(TreeScope_Descendants, &cond) } {
                    let len = unsafe { arr.Length() }.unwrap_or(0).min(160);
                    let mut idx = 0usize;
                    for i in 0..len {
                        let Ok(el) = (unsafe { arr.GetElement(i) }) else {
                            continue;
                        };
                        let name = unsafe { el.CurrentName() }
                            .map(|b| b.to_string())
                            .unwrap_or_default();
                        let is_secret = unsafe { el.CurrentIsPassword() }
                            .map(|b| b.as_bool())
                            .unwrap_or(false);
                        let control_type =
                            unsafe { el.CurrentControlType() }.map(|c| c.0).unwrap_or(0);
                        let is_editable = control_type == 50004 || control_type == 50030;
                        if !name.is_empty() || is_secret || is_editable {
                            if element_locator(&el, idx) == locator {
                                maybe_el = Some(el);
                                break;
                            }
                            idx += 1;
                        }
                    }
                }
            }
        }
        if maybe_el.is_none() {
            let walker = unsafe { uia.ControlViewWalker() }
                .map_err(|e| PlatformError::UnsupportedControl(e.to_string()))?;
            let mut counter = 0usize;
            maybe_el = find_element_by_locator(&walker, &root, locator, 0, &mut counter);
        }
        let fallback_point = parse_pos_locator(locator)
            .or_else(|| action.target.coordinate_hint.map(|h| (h.x, h.y)));
        let Some(el) = maybe_el else {
            if let Some((cx, cy)) = fallback_point {
                match action.kind {
                    ActionKind::Focus | ActionKind::Invoke | ActionKind::Pointer => {
                        send_pointer_click(cx, cy)?;
                        return Ok(format!("Clicked ({cx}, {cy})"));
                    }
                    ActionKind::SetNonSecretValue => {
                        send_pointer_click(cx, cy)?;
                        let text = action
                            .arguments
                            .get("text")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                PlatformError::UnsupportedControl("missing arguments.text".into())
                            })?;
                        let mut inputs = Vec::with_capacity(text.encode_utf16().count() * 2);
                        for code_unit in text.encode_utf16() {
                            inputs.push(INPUT {
                                r#type: INPUT_KEYBOARD,
                                Anonymous: INPUT_0 {
                                    ki: KEYBDINPUT {
                                        wVk: VIRTUAL_KEY(0),
                                        wScan: code_unit,
                                        dwFlags: KEYEVENTF_UNICODE,
                                        time: 0,
                                        dwExtraInfo: 0,
                                    },
                                },
                            });
                            inputs.push(INPUT {
                                r#type: INPUT_KEYBOARD,
                                Anonymous: INPUT_0 {
                                    ki: KEYBDINPUT {
                                        wVk: VIRTUAL_KEY(0),
                                        wScan: code_unit,
                                        dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                                        time: 0,
                                        dwExtraInfo: 0,
                                    },
                                },
                            });
                        }
                        let _ = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
                        return Ok(format!("Typed text at ({cx}, {cy})"));
                    }
                    _ => {}
                }
            }
            return Err(PlatformError::StaleLocator(format!(
                "locator not found: {locator}"
            )));
        };

        let is_secret = unsafe { el.CurrentIsPassword() }
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if is_secret {
            return Err(PlatformError::PermissionDenied(
                "refusing to automate password/secret field".into(),
            ));
        }

        let el_center = unsafe { el.CurrentBoundingRectangle() }.ok().and_then(|r| {
            if r.right > r.left && r.bottom > r.top {
                Some(((r.left + r.right) / 2, (r.top + r.bottom) / 2))
            } else {
                None
            }
        });

        match action.kind {
            ActionKind::Focus => {
                if unsafe { el.SetFocus() }.is_err() {
                    if let Some((cx, cy)) = el_center.or(fallback_point) {
                        send_pointer_click(cx, cy)?;
                        return Ok(format!("Focused via click ({cx}, {cy})"));
                    }
                    return Err(PlatformError::UnsupportedControl("SetFocus failed".into()));
                }
                Ok(format!("Focused {locator}"))
            }
            ActionKind::Invoke => {
                if let Ok(pattern) = unsafe {
                    el.GetCurrentPattern(UIA_InvokePatternId)
                        .and_then(|u| u.cast::<IUIAutomationInvokePattern>())
                } {
                    if unsafe { pattern.Invoke() }.is_ok() {
                        return Ok(format!("Invoked {locator}"));
                    }
                }
                if let Some((cx, cy)) = el_center.or(fallback_point) {
                    send_pointer_click(cx, cy)?;
                    return Ok(format!("Clicked {locator} at ({cx}, {cy})"));
                }
                Err(PlatformError::UnsupportedControl(
                    "InvokePattern unsupported and no bounds available".into(),
                ))
            }
            ActionKind::SetNonSecretValue => {
                let text = action
                    .arguments
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        PlatformError::UnsupportedControl("missing arguments.text".into())
                    })?;
                if let Ok(pattern) = unsafe {
                    el.GetCurrentPattern(UIA_ValuePatternId)
                        .and_then(|u| u.cast::<IUIAutomationValuePattern>())
                } {
                    let bstr = BSTR::from(text);
                    if unsafe { pattern.SetValue(&bstr) }.is_ok() {
                        return Ok(format!("Updated non-secret value on {locator}"));
                    }
                }
                // Fallback for RichEdit / Document controls that don't expose ValuePattern: focus + Unicode SendInput
                let _ = unsafe { el.SetFocus() };
                let mut inputs = Vec::with_capacity(text.encode_utf16().count() * 2);
                for code_unit in text.encode_utf16() {
                    inputs.push(INPUT {
                        r#type: INPUT_KEYBOARD,
                        Anonymous: INPUT_0 {
                            ki: KEYBDINPUT {
                                wVk: VIRTUAL_KEY(0),
                                wScan: code_unit,
                                dwFlags: KEYEVENTF_UNICODE,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    });
                    inputs.push(INPUT {
                        r#type: INPUT_KEYBOARD,
                        Anonymous: INPUT_0 {
                            ki: KEYBDINPUT {
                                wVk: VIRTUAL_KEY(0),
                                wScan: code_unit,
                                dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    });
                }
                let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
                if sent as usize != inputs.len() {
                    return Err(PlatformError::UnsupportedControl(
                        "failed to type text into target control".into(),
                    ));
                }
                Ok(format!("Typed non-secret text into {locator}"))
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

pub fn capture_scope_jpeg_base64(scope_id: &str) -> Result<String, PlatformError> {
    #[cfg(windows)]
    {
        win_impl::capture_screen_jpeg(scope_id)
    }
    #[cfg(not(windows))]
    {
        let _ = scope_id;
        Err(PlatformError::UnsupportedControl("windows only".into()))
    }
}

pub fn exclude_window_from_capture(hwnd_raw: usize) {
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
        };
        let _ = unsafe {
            SetWindowDisplayAffinity(HWND(hwnd_raw as *mut c_void), WDA_EXCLUDEFROMCAPTURE)
        };
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd_raw;
    }
}

pub fn get_window_bounds(scope_id: &str) -> Option<Bounds> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
        let hwnd = win_impl::resolve_target_hwnd(scope_id).ok()?;
        let mut rect = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok()
            && rect.right > rect.left
            && rect.bottom > rect.top
        {
            return Some(Bounds {
                x: rect.left,
                y: rect.top,
                width: (rect.right - rect.left) as u32,
                height: (rect.bottom - rect.top) as u32,
            });
        }
        None
    }
    #[cfg(not(windows))]
    {
        let _ = scope_id;
        None
    }
}

pub fn get_element_bounds(scope_id: &str, locator: &str) -> Option<Bounds> {
    #[cfg(windows)]
    {
        if let Some((cx, cy)) = win_impl::parse_pos_locator(locator) {
            return Some(Bounds {
                x: cx - 70,
                y: cy - 22,
                width: 140,
                height: 44,
            });
        }
        win_impl::resolve_locator_bounds(scope_id, locator)
    }
    #[cfg(not(windows))]
    {
        let _ = (scope_id, locator);
        None
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
        let mut semantic_elements =
            if scope.scope_id.starts_with("hwnd:") || scope.scope_id == "desktop:all" {
                win_impl::observe_hwnd(scope)?
            } else {
                Vec::new()
            };
        #[cfg(not(windows))]
        let mut semantic_elements = Vec::new();

        let mut summary = format!("Selected window: {}", scope.window_title);
        if let Some(office_app) = office::detect_office_app(&scope.window_title) {
            semantic_elements.extend(office::office_virtual_elements(office_app));
            if let Some(office_info) = office::observe_office_summary(office_app) {
                summary = format!("{summary} | {office_info}");
            }
        } else if scope.scope_id == "desktop:all" {
            for w in list_visible_windows() {
                if let Some(office_app) = office::detect_office_app(&w.window_title) {
                    semantic_elements.extend(office::office_virtual_elements(office_app));
                    if let Some(office_info) = office::observe_office_summary(office_app) {
                        summary = format!("{summary} | {office_info}");
                    }
                    break;
                }
            }
        }

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
            visual_context_summary: Some(summary),
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
