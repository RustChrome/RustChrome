//! RustChrome for Windows — native Win32 window, Microsoft Edge WebView2 engine.
//!
//! `--smoke-test` starts the browser, loads the new-tab page, and exits with
//! code 0 once the engine reports a completed navigation. The CI job uses it as
//! a real runtime check (it exercises environment creation, controller creation,
//! event wiring and shutdown), not just a compile.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod chrome;
mod downloads;
mod pages;
mod privacy;
mod webview;
mod window;

use std::cell::RefCell;
use std::rc::Rc;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::*;

fn main() {
    // First thing in the process: the startup measurement is relative to this.
    let _ = app::STARTUP.set(std::time::Instant::now());
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let args: Vec<String> = std::env::args().collect();
    let smoke = args.iter().any(|a| a == "--smoke-test");
    let probe = args
        .iter()
        .position(|a| a == "--privacy-probe")
        .and_then(|i| args.get(i + 1))
        .cloned();

    if let Err(e) = run(smoke, probe) {
        let out = std::env::var("RUSTCHROME_SMOKE_OUT").unwrap_or_default();
        if !out.is_empty() {
            let _ = std::fs::write(&out, format!("FAIL {e}\n"));
        }
        if !smoke {
            window::message_box(HWND::default(), &e, "RustChrome");
        }
        std::process::exit(1);
    }
}

fn run(smoke: bool, probe: Option<String>) -> Result<(), String> {
    let runtime = webview::runtime_version().ok_or_else(|| {
        "The Microsoft Edge WebView2 Runtime is not installed.\n\n\
         RustChrome needs it to render pages. Download the Evergreen Bootstrapper from\n\
         https://developer.microsoft.com/microsoft-edge/webview2/\n\
         install it, then start RustChrome again."
            .to_string()
    })?;

    let compat = std::env::var_os("RUSTCHROME_COMPAT").is_some();
    if !smoke && probe.is_none() && !compat {
        ensure_single_instance();
    }

    let hinst = window::instance();
    window::register(hinst).map_err(|e| format!("could not register the window class: {e}"))?;
    let hwnd = window::create(hinst, "RustChrome", 1280, 800)
        .map_err(|e| format!("could not create the window: {e}"))?;

    let env = webview::create_environment()?;
    let app = Rc::new(RefCell::new(app::App::new(hwnd, env, smoke, probe)));
    app::install(app.clone());
    app::with_app(|a| a.boot());

    window::show(hwnd);

    if smoke {
        println!("RustChrome smoke test — runtime {runtime}");
        let out = std::env::var("RUSTCHROME_SMOKE_OUT").unwrap_or_else(|_| "smoke.txt".into());
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(60));
            let _ = std::fs::write(&out, "FAIL timeout waiting for navigation\n");
            std::process::exit(2);
        });
    }

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

/// One browser per user session; a second launch focuses the existing window.
fn ensure_single_instance() {
    unsafe {
        match CreateMutexW(None, false, w!("Local\\RustChrome.SingleInstance.v1")) {
            Ok(handle) => {
                // GetLastError must be checked immediately after the call.
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    if let Ok(existing) = FindWindowW(window::CLASS, PCWSTR::null()) {
                        let _ = ShowWindow(existing, SW_RESTORE);
                        let _ = SetForegroundWindow(existing);
                    }
                    std::process::exit(0);
                }
                // Hold the handle for the process lifetime; the OS closes it.
                let _ = handle;
            }
            Err(_) => {}
        }
    }
}
