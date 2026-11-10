//! WebView2 environment and controller creation.
//!
//! Everything here is explicit about failure: the runtime can be missing, the
//! environment can refuse to start, or a controller can fail to attach to our
//! window. All three are reported as `Err(String)` with the HRESULT, so the UI
//! can say something useful instead of dying.

use std::path::PathBuf;

use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{
    CoreWebView2EnvironmentOptions, CreateCoreWebView2ControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler,
};
use windows::core::{Interface, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::CoTaskMemFree;

pub struct Created {
    pub controller: ICoreWebView2Controller,
    pub webview: ICoreWebView2,
}

/// Free a CoTaskMem string returned by WebView2 and copy it into a `String`.
pub unsafe fn take(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = p.to_string().unwrap_or_default();
    CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
    s
}

/// Version of the installed Evergreen runtime, or `None` when it is missing.
pub fn runtime_version() -> Option<String> {
    unsafe {
        let mut out = PWSTR::null();
        if GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut out).is_err() {
            return None;
        }
        if out.is_null() {
            return None;
        }
        let v = take(out);
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    }
}

/// WebView2 user data folder. Kept inside our own data dir so uninstalling the
/// browser is a single directory delete.
pub fn user_data_dir() -> PathBuf {
    let d = rc_core::data_dir().join("UserData");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn create_environment() -> Result<ICoreWebView2Environment, String> {
    unsafe {
        let folder = HSTRING::from(user_data_dir().to_string_lossy().to_string());
        let options = CoreWebView2EnvironmentOptions::default();
        // Strip Edge-only chrome we do not implement and never show.
        options.set_additional_browser_arguments(String::from(
            "--disable-features=msEdgeSidebarV2,msEdgeShoppingAssistant,msFeedback,msEdgeCollections",
        ));
        // Tracking prevention is on by default in this crate; make it explicit.
        options.set_enable_tracking_prevention(true);
        let base: ICoreWebView2EnvironmentOptions = options.into();

        let (tx, rx) = std::sync::mpsc::channel();
        let handler = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
            move |hr, env| {
                let res = match (hr, env) {
                    (Ok(()), Some(e)) => Ok(e),
                    (Err(e), _) => Err(format!("WebView2 environment failed: {e}")),
                    _ => Err("WebView2 returned no environment".to_string()),
                };
                let _ = tx.send(res);
                Ok(())
            },
        ));

        CreateCoreWebView2EnvironmentWithOptions(PCWSTR::null(), &folder, &base, &handler)
            .map_err(|e| format!("CreateCoreWebView2EnvironmentWithOptions: {e}"))?;

        match webview2_com::wait_with_pump(rx) {
            Ok(res) => res,
            Err(e) => Err(format!("environment creation did not complete: {e}")),
        }
    }
}

/// Create a controller + webview asynchronously.
///
/// Returns `Err` only for failures that happen before the async request is
/// issued; everything later is delivered to `on_ready`.
pub fn create_controller(
    env: &ICoreWebView2Environment,
    hwnd: HWND,
    in_private: bool,
    on_ready: impl Fn(Result<Created, String>) + 'static,
) -> Result<(), String> {
    unsafe {
        let env10 = env
            .cast::<ICoreWebView2Environment10>()
            .map_err(|e| format!("WebView2 runtime is too old for per-tab profiles: {e}"))?;
        let options = env10
            .CreateCoreWebView2ControllerOptions()
            .map_err(|e| format!("controller options: {e}"))?;
        options
            .SetIsInPrivateModeEnabled(in_private)
            .map_err(|e| format!("in-private mode: {e}"))?;

        let handler = CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
            move |hr, ctrl| {
                let res = match (hr, ctrl) {
                    (Ok(()), Some(c)) => match c.CoreWebView2() {
                        Ok(wv) => Ok(Created { controller: c, webview: wv }),
                        Err(e) => Err(format!("CoreWebView2: {e}")),
                    },
                    (Err(e), _) => Err(format!("CreateCoreWebView2Controller: {e}")),
                    _ => Err("WebView2 returned no controller".to_string()),
                };
                on_ready(res);
                Ok(())
            },
        ));

        env10
            .CreateCoreWebView2ControllerWithOptions(hwnd, &options, &handler)
            .map_err(|e| format!("CreateCoreWebView2ControllerWithOptions: {e}"))
    }
}
