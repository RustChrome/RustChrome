//! Application state, tab management, event wiring and the browser commands.
//!
//! There is exactly one App per process and it lives on the UI thread inside a
//! `thread_local` Rc, because every WebView2 callback runs on that thread
//! anyway. Handlers capture a tab id, never `self`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rc_core::dashboard::{Dashboard, Row};
use rc_core::features::{self, SessionState};
use rc_core::privacy::SessionSeed;
use rc_core::store::{self, Bookmark, Prefs};
use serde_json::{json, Value};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{
    AcceleratorKeyPressedEventHandler, ClearBrowsingDataCompletedHandler,
    DocumentTitleChangedEventHandler, DownloadStartingEventHandler, ExecuteScriptCompletedHandler,
    HistoryChangedEventHandler, NavigationCompletedEventHandler, NavigationStartingEventHandler,
    NewWindowRequestedEventHandler, PermissionRequestedEventHandler, ProcessFailedEventHandler,
    SourceChangedEventHandler, WebMessageReceivedEventHandler,
};
use windows::core::{Interface, BOOL, HSTRING, PWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::{chrome, downloads, pages, privacy, webview, window};

/// Process start, for the startup measurement.
pub static STARTUP: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

// ---------------------------------------------------------------- global state

thread_local! {
    static APP: RefCell<Option<Rc<RefCell<App>>>> = const { RefCell::new(None) };
}

pub fn install(app: Rc<RefCell<App>>) {
    APP.with(|c| *c.borrow_mut() = Some(app));
}

/// Run `f` with the app borrowed. Skipped (not panicked) if a handler fires
/// while we are already inside a borrow — losing one redraw beats crashing.
/// Callbacks the UI bridge could not deliver because `App` was already borrowed.
/// Dropping a click silently is how an invisible bug survives; the count is
/// reported by the smoke trace.
pub static DROPPED_MESSAGES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Failed `SetBounds` calls. Non-zero means a webview is probably invisible.
pub static BOUNDS_FAILURES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn with_app<F: FnOnce(&mut App)>(f: F) {
    let rc = APP.with(|c| c.borrow().clone());
    if let Some(rc) = rc {
        match rc.try_borrow_mut() {
            Ok(mut a) => f(&mut a),
            Err(_) => {
                // Re-entrant callback: the previous handler is still holding the
                // App. The action is lost, so at least make it countable.
                DROPPED_MESSAGES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
}

// ------------------------------------------------------------------------ tab

struct Tab {
    id: u64,
    controller: Option<ICoreWebView2Controller>,
    webview: Option<ICoreWebView2>,
    title: String,
    url: String,
    zoom: f64,
    scripts: Vec<String>,
    /// The navigation the tab is supposed to end up on. `Some("")` means the
    /// new-tab page. Kept until a load succeeds so a cancelled navigation can
    /// be re-issued instead of leaving the tab blank.
    wants: Option<String>,
    retries: u8,
    loading: bool,
    is_new_tab: bool,
}

// ------------------------------------------------------------------------ app

pub struct App {
    pub hwnd: HWND,
    pub env: ICoreWebView2Environment,
    pub prefs: Prefs,
    pub chrome: Option<chrome::Chrome>,
    tabs: Vec<Tab>,
    pub active: usize,
    pub next_id: u64,
    pub session: SessionState,
    pub tab_search_open: bool,
    pub tab_search_query: String,
    pub sites: Vec<rc_core::features::SiteRule>,
    /// Live download operations, so the UI can cancel them.
    pub dl_ops: Vec<(u64, ICoreWebView2DownloadOperation)>,
    /// One-shot message shown as a toast in the UI.
    pub notice: Option<String>,
    pub bookmarks: Vec<Bookmark>,
    pub downloads: Vec<downloads::Item>,
    pub next_dl: u64,
    pub find_open: bool,
    pub find_count: i32,
    pub find_index: i32,
    pub find_last: String,
    pub closing: bool,
    pub smoke: bool,
    /// Messages received from the HTML chrome. Zero means it never loaded.
    ui_msgs: u64,
    pub runtime: String,
    /// Canvas-noise key for this session; rotated by New identity.
    pub seed: SessionSeed,
    /// User agent derived from the installed runtime (no `Edg/` token).
    pub ua: String,
    /// `--privacy-probe <path>`: run the probe once and write the report there.
    pub probe: Option<String>,
    /// `RUSTCHROME_COMPAT`: load a fixed list of sites, record the outcome and
    /// exit. Used by the QA workflow to check real-world compatibility.
    pub compat: Option<String>,
    pub compat_queue: Vec<String>,
    pub compat_results: Vec<Value>,
}

impl App {
    pub fn new(
        hwnd: HWND,
        env: ICoreWebView2Environment,
        smoke: bool,
        probe: Option<String>,
    ) -> Self {
        let runtime = webview::runtime_version().unwrap_or_else(|| "unknown".into());
        let ua = privacy::user_agent(&runtime);
        Self {
            hwnd,
            env,
            prefs: Prefs::load(),
            chrome: None,
            tabs: Vec::new(),
            active: 0,
            next_id: 1,
            session: SessionState::load(),
            tab_search_open: false,
            tab_search_query: String::new(),
            sites: features::load_sites(),
            dl_ops: Vec::new(),
            notice: None,
            bookmarks: store::load_bookmarks(),
            downloads: Vec::new(),
            next_dl: 1,
            find_open: false,
            find_count: 0,
            find_index: 0,
            find_last: String::new(),
            closing: false,
            smoke,
            ui_msgs: 0,
            runtime,
            seed: SessionSeed::new(),
            ua,
            probe,
            compat: std::env::var("RUSTCHROME_COMPAT").ok(),
            compat_queue: if std::env::var("RUSTCHROME_COMPAT").is_ok() {
                vec![
                    "https://example.com/".to_string(),
                    "https://en.wikipedia.org/wiki/WebKit".to_string(),
                    "https://news.ycombinator.com/".to_string(),
                    "https://www.rust-lang.org/".to_string(),
                ]
            } else {
                Vec::new()
            },
            compat_results: Vec::new(),
        }
    }

    /// Kick off the UI webview and the first tab. If the previous run ended
    /// badly, the tabs it had are restored first.
    pub fn boot(&mut self) {
        let prev = self.session.clone();
        if self.compat.is_none() && self.prefs.restore_session && features::crashed(&prev) {
            self.trace(&format!(
                "restoring {} tab(s) after a crash",
                prev.tabs.len()
            ));
            for url in prev.tabs.iter().take(20) {
                self.new_tab(Some(url.clone()), true);
            }
        }
        self.trace(&format!(
            "boot: hwnd={:?} runtime={}",
            self.hwnd.0, self.runtime
        ));
        window::apply_frame_theme(self.hwnd, self.prefs.is_dark());
        let env = self.env.clone();
        let hwnd = self.hwnd;
        let in_private = !self.prefs.keep_session;
        let r = webview::create_controller(&env, hwnd, in_private, move |res| {
            with_app(|a| a.on_chrome_ready(res));
        });
        if let Err(e) = r {
            self.fatal(&e);
            return;
        }
        self.new_tab(None, true);
    }

    /// In `--smoke-test` mode write progress to a log next to the result file
    /// instead of showing dialogs, which would block a headless CI session.
    pub fn trace(&self, msg: &str) {
        if !self.smoke {
            return;
        }
        let path = std::env::var("RUSTCHROME_SMOKE_OUT").unwrap_or_else(|_| "smoke.txt".into());
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(format!("{path}.log"))
        {
            let _ = writeln!(f, "{msg}");
        }
    }

    fn fatal(&self, msg: &str) {
        self.trace(&format!("fatal: {msg}"));
        if self.smoke {
            let out = std::env::var("RUSTCHROME_SMOKE_OUT").unwrap_or_else(|_| "smoke.txt".into());
            let _ = std::fs::write(out, format!("FAIL fatal: {msg}\n"));
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
            return;
        }
        let text = format!(
            "RustChrome could not start its browser engine.\n\n{msg}\n\n\
             Install or repair the Microsoft Edge WebView2 Runtime, then start RustChrome again:\n\
             https://developer.microsoft.com/microsoft-edge/webview2/",
        );
        window::message_box(self.hwnd, &text, "RustChrome");
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }

    // ------------------------------------------------------------- chrome view

    fn on_chrome_ready(&mut self, res: Result<webview::Created, String>) {
        self.trace(&format!("chrome ready: ok={}", res.is_ok()));
        let created = match res {
            Ok(c) => c,
            Err(e) => {
                self.fatal(&e);
                return;
            }
        };
        privacy::configure(&created.webview, &self.prefs, true);

        unsafe {
            let mut token = 0i64;
            let h = WebMessageReceivedEventHandler::create(Box::new(move |_sender, args| {
                if let Some(args) = args {
                    let mut p = PWSTR::null();
                    if args.WebMessageAsJson(&mut p).is_ok() {
                        let raw = webview::take(p);
                        with_app(move |a| a.on_ui_message(&raw));
                    }
                }
                Ok(())
            }));
            let _ = created.webview.add_WebMessageReceived(&h, &mut token);

            // The UI must never navigate anywhere — but NavigateToString (our
            // own bootstrap page) reports an empty URI and must be allowed.
            let mut token = 0i64;
            let h = NavigationStartingEventHandler::create(Box::new(|_s, args| {
                if let Some(args) = args {
                    let mut p = PWSTR::null();
                    let uri = if args.Uri(&mut p).is_ok() {
                        webview::take(p)
                    } else {
                        String::new()
                    };
                    // Log what the UI actually tries to load; the browser's own
                    // chrome is the only thing this webview may show.
                    if let Ok(p) = std::env::var("RUSTCHROME_SMOKE_OUT") {
                        use std::io::Write;
                        if let Ok(mut f) = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(format!("{p}.log"))
                        {
                            let _ = writeln!(f, "ui nav: \"{}\"", uri);
                        }
                    }
                    // Never cancel the bootstrap load. Cancelling it produced a
                    // window with a page and no tabs, no toolbar and no address
                    // bar, while every WebView2 call returned success.
                    let _ = &uri;
                }
                Ok(())
            }));
            let _ = created.webview.add_NavigationStarting(&h, &mut token);

            // Belt and braces: deny everything the UI asks for.
            let mut token = 0i64;
            let h = PermissionRequestedEventHandler::create(Box::new(|_s, args| {
                if let Some(args) = args {
                    let _ = args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY);
                }
                Ok(())
            }));
            let _ = created.webview.add_PermissionRequested(&h, &mut token);
        }

        unsafe {
            let _ = created.controller.SetZoomFactor(1.0);
            let _ = created.controller.SetIsVisible(true);
        }
        self.chrome = Some(chrome::Chrome {
            controller: created.controller,
            webview: created.webview.clone(),
            height_css: if self.prefs.show_bookmarks {
                118.0
            } else {
                88.0
            },
        });
        chrome::load(&created.webview);
        window::apply_frame_theme(self.hwnd, self.prefs.is_dark());
        self.relayout();
        self.push_state();
    }

    // ------------------------------------------------------------------- tabs

    pub fn new_tab(&mut self, url: Option<String>, activate: bool) {
        let id = self.next_id;
        self.next_id += 1;
        let url = url.unwrap_or_default();
        self.tabs.push(Tab {
            id,
            controller: None,
            webview: None,
            title: String::new(),
            url: url.clone(),
            zoom: self.prefs.zoom,
            scripts: Vec::new(),
            wants: Some(url),
            retries: 0,
            loading: false,
            is_new_tab: false,
        });
        if activate || self.tabs.len() == 1 {
            self.active = self.tabs.len() - 1;
        }

        let env = self.env.clone();
        let hwnd = self.hwnd;
        let in_private = !self.prefs.keep_session;
        let r = webview::create_controller(&env, hwnd, in_private, move |res| {
            with_app(|a| a.on_controller(id, res));
        });
        if let Err(e) = r {
            self.on_controller(id, Err(e));
        }
        self.relayout();
        self.push_state();
    }

    fn on_controller(&mut self, id: u64, res: Result<webview::Created, String>) {
        self.trace(&format!("tab {id} controller: ok={}", res.is_ok()));
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        match res {
            Err(e) => {
                self.trace(&format!("tab {id} failed: {e}"));
                self.tabs[idx].title = "Tab failed".to_string();
                self.push_state();
                if self.smoke {
                    let out = std::env::var("RUSTCHROME_SMOKE_OUT")
                        .unwrap_or_else(|_| "smoke.txt".into());
                    let _ = std::fs::write(out, format!("FAIL controller: {e}\n"));
                    unsafe {
                        let _ = DestroyWindow(self.hwnd);
                    }
                    return;
                }
                window::message_box(
                    self.hwnd,
                    &format!("A tab could not be created.\n\n{e}"),
                    "RustChrome",
                );
            }
            Ok(created) => {
                let is_active = idx == self.active;
                privacy::configure(&created.webview, &self.prefs, false);
                let ua = if self.prefs.antifp {
                    Some(self.ua.clone())
                } else {
                    None
                };
                privacy::set_user_agent(&created.webview, ua.as_deref());
                let zoom = self.tabs[idx].zoom;
                unsafe {
                    let _ = created.controller.SetZoomFactor(zoom);
                    let _ = created.controller.SetIsVisible(is_active);
                    webview::set_rasterization(&created.controller, window::dpi_scale(self.hwnd));
                }
                self.wire_events(&created.webview, &created.controller, id);
                self.tabs[idx].controller = Some(created.controller);
                self.tabs[idx].webview = Some(created.webview);
                self.reinstall_scripts(idx);

                let wants = self.tabs[idx].wants.clone();
                if let Some(url) = wants {
                    if url.is_empty() {
                        self.show_new_tab(idx);
                    } else {
                        self.load_in(idx, &url);
                    }
                }
                self.relayout();
                self.push_state();
            }
        }
    }

    fn wire_events(&self, wv: &ICoreWebView2, ctrl: &ICoreWebView2Controller, id: u64) {
        unsafe {
            // -- title
            let h = DocumentTitleChangedEventHandler::create(Box::new(move |sender, _a| {
                if let Some(sv) = sender {
                    let mut p = PWSTR::null();
                    if sv.DocumentTitle(&mut p).is_ok() {
                        let title = webview::take(p);
                        with_app(move |a| a.on_title(id, title));
                    }
                }
                Ok(())
            }));
            let mut t = 0i64;
            let _ = wv.add_DocumentTitleChanged(&h, &mut t);

            // -- url
            let h = SourceChangedEventHandler::create(Box::new(move |sender, _a| {
                if let Some(sv) = sender {
                    let mut p = PWSTR::null();
                    if sv.Source(&mut p).is_ok() {
                        let url = webview::take(p);
                        with_app(move |a| a.on_source(id, url));
                    }
                }
                Ok(())
            }));
            let mut t = 0i64;
            let _ = wv.add_SourceChanged(&h, &mut t);

            // -- history (back/forward buttons)
            let h = HistoryChangedEventHandler::create(Box::new(move |_s, _a| {
                with_app(move |a| a.push_state());
                Ok(())
            }));
            let mut t = 0i64;
            let _ = wv.add_HistoryChanged(&h, &mut t);

            // -- navigation gate
            let h = NavigationStartingEventHandler::create(Box::new(move |_s, args| {
                if let Some(args) = args {
                    let mut p = PWSTR::null();
                    let uri = if args.Uri(&mut p).is_ok() {
                        webview::take(p)
                    } else {
                        String::new()
                    };
                    let allowed = rc_core::is_allowed_navigation(&uri);
                    if !allowed {
                        let _ = args.SetCancel(true);
                    }
                    with_app(move |a| a.on_nav_starting(id, uri, allowed));
                }
                Ok(())
            }));
            let mut t = 0i64;
            let _ = wv.add_NavigationStarting(&h, &mut t);

            // -- navigation finished
            let h = NavigationCompletedEventHandler::create(Box::new(move |_s, args| {
                let (ok, status) = match &args {
                    Some(a) => {
                        let mut b = BOOL(0);
                        let _ = a.IsSuccess(&mut b);
                        let mut s = COREWEBVIEW2_WEB_ERROR_STATUS(0);
                        let _ = a.WebErrorStatus(&mut s);
                        (b.as_bool(), s.0)
                    }
                    None => (false, 0),
                };
                with_app(move |a| a.on_nav_completed(id, ok, status));
                Ok(())
            }));
            let mut t = 0i64;
            let _ = wv.add_NavigationCompleted(&h, &mut t);

            // -- target=_blank and window.open
            let h = NewWindowRequestedEventHandler::create(Box::new(move |_s, args| {
                if let Some(args) = args {
                    let mut p = PWSTR::null();
                    if args.Uri(&mut p).is_ok() {
                        let uri = webview::take(p);
                        let _ = args.SetHandled(true);
                        if !uri.is_empty() {
                            with_app(move |a| a.new_tab(Some(uri), true));
                        }
                    } else {
                        let _ = args.SetHandled(true);
                    }
                }
                Ok(())
            }));
            let mut t = 0i64;
            let _ = wv.add_NewWindowRequested(&h, &mut t);

            // -- permissions: deny everything, like the Linux build
            // Deny unless the user has an explicit per-site allow rule.
            let h = PermissionRequestedEventHandler::create(Box::new(|_s, args| {
                if let Some(args) = args {
                    let mut p = PWSTR::null();
                    let uri = if args.Uri(&mut p).is_ok() {
                        webview::take(p)
                    } else {
                        String::new()
                    };
                    let mut kind = COREWEBVIEW2_PERMISSION_KIND(0);
                    let _ = args.PermissionKind(&mut kind);
                    let allow = with_app_value(|a| {
                        features::site_decision(&a.sites, &uri, permission_name(kind)) == Some(true)
                    })
                    .unwrap_or(false);
                    let _ = args.SetState(if allow {
                        COREWEBVIEW2_PERMISSION_STATE_ALLOW
                    } else {
                        COREWEBVIEW2_PERMISSION_STATE_DENY
                    });
                }
                Ok(())
            }));
            let mut t = 0i64;
            let _ = wv.add_PermissionRequested(&h, &mut t);

            // -- downloads
            let h = DownloadStartingEventHandler::create(Box::new(move |_s, args| {
                if let Some(args) = args {
                    with_app(move |a| a.on_download(args));
                }
                Ok(())
            }));
            if let Ok(w4) = wv.cast::<ICoreWebView2_4>() {
                let mut t = 0i64;
                let _ = w4.add_DownloadStarting(&h, &mut t);
            }

            // -- renderer crash
            let h = ProcessFailedEventHandler::create(Box::new(move |_s, _a| {
                with_app(move |a| a.on_process_failed(id));
                Ok(())
            }));
            let mut t = 0i64;
            let _ = wv.add_ProcessFailed(&h, &mut t);

            // -- keyboard
            let h = AcceleratorKeyPressedEventHandler::create(Box::new(move |_s, args| {
                if let Some(args) = args {
                    let mut vk = 0u32;
                    let _ = args.VirtualKey(&mut vk);
                    if with_app_handled(move |a| a.on_key(vk)) {
                        let _ = args.SetHandled(true);
                    }
                }
                Ok(())
            }));
            let mut t = 0i64;
            let _ = ctrl.add_AcceleratorKeyPressed(&h, &mut t);
        }
    }

    // --------------------------------------------------------------- scripts

    fn reinstall_scripts(&mut self, idx: usize) {
        let (Some(wv), id) = (
            self.tabs.get(idx).and_then(|t| t.webview.clone()),
            self.tabs.get(idx).map(|t| t.id).unwrap_or(0),
        ) else {
            return;
        };
        let old = std::mem::take(&mut self.tabs[idx].scripts);
        privacy::remove_scripts(&wv, &old);

        if self.prefs.antifp {
            let js = privacy::antifp_js(self.seed, self.prefs.strict);
            privacy::add_script(&wv, &js, move |sid| {
                with_app(move |a| a.track_script(id, sid))
            });
        }
        if self.prefs.force_dark {
            privacy::add_script(&wv, &privacy::force_dark_js(), move |sid| {
                with_app(move |a| a.track_script(id, sid))
            });
        }
        for us in store::load_userscripts() {
            privacy::add_script(&wv, &us.code, move |sid| {
                with_app(move |a| a.track_script(id, sid))
            });
        }
    }

    fn track_script(&mut self, tab_id: u64, script_id: String) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            t.scripts.push(script_id);
        }
    }

    // ------------------------------------------------------------- navigation

    /// Render one of the internal pages the GTK build also has
    /// (about:rustchrome-bookmarks / -extensions).
    fn open_internal_page(&mut self, which: &str) {
        let Some(wv) = self.tabs.get(self.active).and_then(|t| t.webview.clone()) else {
            return;
        };
        let html = match which {
            "bookmarks" => pages::bookmarks_html(&self.bookmarks),
            "extensions" => pages::extensions_html(
                &store::userscript_files(),
                &store::userscript_dir().to_string_lossy(),
            ),
            "privacy" => rc_core::dashboard::dashboard_html(&self.build_dashboard()),
            _ => return,
        };
        let idx = self.active;
        self.tabs[idx].is_new_tab = true;
        self.tabs[idx].url.clear();
        self.tabs[idx].wants = Some(String::new());
        self.tabs[idx].title = match which {
            "bookmarks" => "Bookmarks",
            "extensions" => "Extensions",
            _ => "Privacy",
        }
        .to_string();
        unsafe {
            let _ = wv.NavigateToString(&HSTRING::from(html));
        }
        self.push_state();
    }

    fn finish_compat(&mut self) {
        let Some(path) = self.compat.clone() else {
            return;
        };
        let body = serde_json::json!({
            "total": self.compat_results.len(),
            "ok": self.compat_results.iter().filter(|r| r["ok"] == true).count(),
            "results": self.compat_results,
        });
        let _ = std::fs::write(
            path,
            serde_json::to_string_pretty(&body).unwrap_or_default(),
        );
        self.closing = true;
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }

    /// The probe page publishes its report as the document title, so this needs
    /// no script evaluation and behaves identically on both shells.
    fn run_probe(&mut self) {
        let Some(path) = self.probe.clone() else {
            return;
        };
        let Some(wv) = self.tabs.first().and_then(|t| t.webview.clone()) else {
            return;
        };
        let mut p = PWSTR::null();
        let title = unsafe {
            if wv.DocumentTitle(&mut p).is_ok() {
                webview::take(p)
            } else {
                String::new()
            }
        };
        // The first completion is the blank page a controller starts on.
        if !title.starts_with('{') && !title.starts_with("FAIL") {
            return;
        }
        let _ = std::fs::write(&path, title);
        self.closing = true;
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }

    /// Everything the dashboard shows comes from real state.
    fn build_dashboard(&self) -> Dashboard {
        let on = |b: bool| if b { "on" } else { "off" }.to_string();
        let strict = self.prefs.strict;
        let antifp = self.prefs.antifp;
        let protections = vec![
            Row {
                name: "User agent".into(),
                state: on(antifp),
                note: format!("engine setting, Chrome-like, Edg/ stripped; client hints disabled ({})", self.ua),
            },
            Row {
                name: "Canvas".into(),
                state: if antifp { "partial".into() } else { "off".into() },
                note: "per-session noise: same pixels within a session, different every launch".into(),
            },
            Row {
                name: "WebGL".into(),
                state: if !antifp { "off" } else if strict { "on" } else { "partial" }.into(),
                note: if strict {
                    "contexts denied".into()
                } else {
                    "context available, GPU vendor/renderer coerced".to_string()
                },
            },
            Row {
                name: "WebRTC".into(),
                state: if !antifp { "off" } else if strict { "on" } else { "partial" }.into(),
                note: if strict {
                    "API removed".into()
                } else {
                    "API available, relay-only candidates (no LAN address), non-proxied UDP blocked".to_string()
                },
            },
            Row {
                name: "Time zone".into(),
                state: on(antifp),
                note: "UTC in getTimezoneOffset, Intl and Date#toString".into(),
            },
            Row {
                name: "Languages".into(),
                state: on(antifp),
                note: "en-US,en in script and in Accept-Language".into(),
            },
            Row {
                name: "Screen size".into(),
                state: "off".into(),
                note: "reported as-is: a pinned size would contradict the real window".into(),
            },
            Row {
                name: "Cookies / third-party storage".into(),
                state: "on".into(),
                note: "tracking prevention level: strict".into(),
            },
            Row {
                name: "Permissions".into(),
                state: "on".into(),
                note: "every request denied before a prompt is shown".into(),
            },
            Row {
                name: "DNS prefetch / preconnect".into(),
                state: "on".into(),
                note: "disabled at the engine".into(),
            },
            Row {
                name: "Hyperlink auditing (a ping)".into(),
                state: "on".into(),
                note: "disabled at the engine".into(),
            },
            Row {
                name: "Tracker blocking".into(),
                state: "partial".into(),
                note: "Microsoft's tracking-prevention list; the engine exposes no blocked count, so none is shown".into(),
            },
            Row {
                name: "Ad blocking".into(),
                state: "off".into(),
                note: "not implemented".into(),
            },
        ];

        Dashboard {
            engine: format!("WebView2 {}", self.runtime),
            session_mode: if self.prefs.keep_session {
                "persistent (data kept between launches)".into()
            } else {
                "in-private (nothing written to disk)".into()
            },
            settings: vec![
                ("Anti-fingerprinting".into(), on(antifp)),
                ("Strict mode".into(), on(strict)),
                ("Keep session data".into(), on(self.prefs.keep_session)),
                ("Search engine".into(), self.prefs.engine.clone()),
                ("Theme".into(), self.prefs.theme.clone()),
            ],
            protections,
            permissions: vec![
                "Microphone, Camera, Geolocation, Notifications".into(),
                "Sensors (ambient light, accelerometer, gyroscope, magnetometer)".into(),
                "Clipboard read, File read/write, Multiple automatic downloads".into(),
                "Autoplay, MIDI sysex, Window management, Unknown".into(),
            ],
            limitations: vec![
                "Not anonymity and not Tor. A page that runs JavaScript can still infer things about the machine.".into(),
                "WebGL and WebRTC cannot be switched off in the engine; they are coerced (Strict removes them in script).".into(),
                "Font metrics and the GPU model cannot be normalised.".into(),
                "The WebView2 runtime auto-updates, so the Chromium fingerprint surface can change without a RustChrome release.".into(),
                "No blocker statistics: the engine does not report how many requests tracking prevention stopped.".into(),
            ],
        }
    }

    fn show_new_tab(&mut self, idx: usize) {
        let Some(wv) = self.tabs.get(idx).and_then(|t| t.webview.clone()) else {
            return;
        };
        if let Some(first) = self.compat_queue.first().cloned() {
            self.compat_queue.remove(0);
            self.load_in(idx, &first);
            return;
        }
        let html = if self.probe.is_some() {
            rc_core::privacy::probe_html()
        } else {
            pages::new_tab_html(&self.prefs.engine, self.prefs.is_dark())
        };
        self.tabs[idx].wants = Some(String::new());
        self.tabs[idx].url.clear();
        self.tabs[idx].is_new_tab = true;
        self.tabs[idx].title = "New Tab".to_string();
        let hr = unsafe { wv.NavigateToString(&HSTRING::from(html)) };
        self.trace(&format!(
            "tab {} new tab page: {:?}",
            self.tabs[idx].id,
            hr.is_ok()
        ));
    }

    fn load_in(&mut self, idx: usize, url: &str) {
        let Some(wv) = self.tabs.get(idx).and_then(|t| t.webview.clone()) else {
            return;
        };
        self.tabs[idx].wants = Some(url.to_string());
        self.tabs[idx].url = url.to_string();
        self.tabs[idx].is_new_tab = false;
        let hr = unsafe { wv.Navigate(&HSTRING::from(url)) };
        self.trace(&format!(
            "tab {} navigate {url}: {:?}",
            self.tabs[idx].id,
            hr.is_ok()
        ));
    }

    /// A cancelled navigation leaves the tab on about:blank. Re-issue the
    /// intended target a bounded number of times.
    fn retry_navigation(&mut self, idx: usize) {
        let wants = self.tabs[idx].wants.clone();
        let Some(target) = wants else { return };
        if self.tabs[idx].retries >= 2 {
            self.trace(&format!(
                "tab {} giving up after 2 retries",
                self.tabs[idx].id
            ));
            return;
        }
        self.tabs[idx].retries += 1;
        self.trace(&format!(
            "tab {} retry {}: {:?}",
            self.tabs[idx].id, self.tabs[idx].retries, target
        ));
        if target.is_empty() {
            self.show_new_tab(idx);
        } else {
            self.load_in(idx, &target);
        }
    }

    pub fn navigate_active(&mut self, input: &str) {
        let url = rc_core::normalize(input, &self.prefs.engine);
        if url.is_empty() {
            return;
        }
        let idx = self.active;
        if let Some(t) = self.tabs.get(idx) {
            if t.webview.is_none() {
                if let Some(t) = self.tabs.get_mut(idx) {
                    t.wants = Some(url);
                }
                return;
            }
        }
        self.load_in(idx, &url);
        self.push_state();
    }

    fn on_nav_starting(&mut self, id: u64, uri: String, allowed: bool) {
        self.trace(&format!(
            "tab {id} nav starting: allowed={allowed} uri={uri:?}"
        ));
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.loading = true;
            if allowed && !uri.is_empty() && uri != "about:blank" {
                t.url = uri;
                t.is_new_tab = false;
            }
        }
        self.push_state();
    }

    fn on_source(&mut self, id: u64, uri: String) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            if uri != "about:blank" {
                t.url = uri;
                t.is_new_tab = false;
            }
        }
        self.persist_session();
        self.push_state();
    }

    fn on_title(&mut self, id: u64, title: String) {
        let mut active_title = None;
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.title = title;
        }
        if self.tabs.get(self.active).map(|t| t.id) == Some(id) {
            active_title = self.tabs.get(self.active).map(|t| t.title.clone());
        }
        if let Some(title) = active_title {
            let title = if title.is_empty() {
                "RustChrome".to_string()
            } else {
                format!("{title} - RustChrome")
            };
            window::set_title(self.hwnd, &title);
        }
        self.push_state();
    }

    fn on_nav_completed(&mut self, id: u64, ok: bool, status: i32) {
        self.trace(&format!("tab {id} nav completed: ok={ok} status={status}"));
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.loading = false;
        }
        // WebView2 cancels the initial about:blank navigation when a controller
        // is created and we immediately load a page. That is not a failure, and
        // it must not produce an error page or fail the smoke test.
        let canceled = status == COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED.0;
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            if ok {
                self.tabs[idx].wants = None;
                self.tabs[idx].retries = 0;
            } else if canceled && self.tabs[idx].wants.is_some() {
                self.retry_navigation(idx);
            }
        }
        if !ok && !canceled {
            if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
                let url = self.tabs[idx].url.clone();
                if !url.is_empty() && url != "about:blank" {
                    let html = pages::error_html(
                        &url,
                        &format!("Network error (status {status})."),
                        self.prefs.is_dark(),
                    );
                    if let Some(wv) = self.tabs[idx].webview.clone() {
                        unsafe {
                            let _ = wv.NavigateToString(&HSTRING::from(html));
                        }
                    }
                }
            }
        }
        if ok && id == self.tabs.first().map(|t| t.id).unwrap_or(0) {
            if let (Some(start), Ok(path)) =
                (STARTUP.get(), std::env::var("RUSTCHROME_STARTUP_LOG"))
            {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    let _ = writeln!(f, "startup_ms={}", start.elapsed().as_millis());
                }
            }
        }
        if self.smoke && id == self.tabs.first().map(|t| t.id).unwrap_or(0) && (ok || !canceled) {
            // Report whether the HTML chrome ever talked to us: if it did not,
            // the window has no controls even though every WebView2 call
            // succeeded. Gated behind an env var until the UI is confirmed to
            // render on a real desktop, so packaging still runs.
            self.trace(&format!(
                "ui messages: {} dropped: {} bounds_failures: {}",
                self.ui_msgs,
                DROPPED_MESSAGES.load(std::sync::atomic::Ordering::Relaxed),
                BOUNDS_FAILURES.load(std::sync::atomic::Ordering::Relaxed)
            ));
            let strict_ui = std::env::var("RUSTCHROME_STRICT_UI").is_ok();
            self.finish_smoke(ok && (!strict_ui || self.ui_msgs > 0), status);
        }
        if self.compat.is_some() && !canceled && id == self.tabs.first().map(|t| t.id).unwrap_or(0)
        {
            let url = self.tabs.first().map(|t| t.url.clone()).unwrap_or_default();
            let title = self
                .tabs
                .first()
                .map(|t| t.title.clone())
                .unwrap_or_default();
            let elapsed = STARTUP
                .get()
                .map(|s| s.elapsed().as_millis() as u64)
                .unwrap_or(0);
            self.compat_results.push(serde_json::json!({
                "url": url,
                "title": title,
                "ok": ok,
                "status": status,
                "elapsed_ms": elapsed,
            }));
            if let Some(next) = self.compat_queue.first().cloned() {
                self.compat_queue.remove(0);
                self.load_in(0, &next);
            } else {
                self.finish_compat();
            }
            self.push_state();
            return;
        }
        if ok
            && self.probe.is_some()
            && id == self.tabs.first().map(|t| t.id).unwrap_or(0)
            && !self.closing
        {
            self.run_probe();
        }
        self.push_state();
    }

    fn on_process_failed(&mut self, id: u64) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.loading = false;
            t.title = "Page crashed".to_string();
        }
        self.push_state();
    }

    fn activate(&mut self, id: u64) {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        self.active = idx;
        for (i, t) in self.tabs.iter().enumerate() {
            if let Some(c) = &t.controller {
                unsafe {
                    let _ = c.SetIsVisible(i == idx);
                }
            }
        }
        if let Some(z) = self.tabs.get(idx).map(|t| t.zoom) {
            if let Some(c) = self.tabs[idx].controller.clone() {
                unsafe {
                    let _ = c.SetZoomFactor(z);
                }
            }
        }
        self.relayout();
        self.push_state();
    }

    fn close_tab(&mut self, id: u64) {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        let tab = self.tabs.remove(idx);
        if !tab.url.is_empty() {
            self.session.push_closed(&tab.url);
        }
        if let Some(c) = &tab.controller {
            unsafe {
                let _ = c.Close();
            }
        }
        if self.tabs.is_empty() {
            self.active = 0;
            self.new_tab(None, true);
        } else {
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len() - 1;
            }
            let id = self.tabs[self.active].id;
            self.activate(id);
        }
        self.persist_session();
        self.push_state();
    }

    fn reopen_closed(&mut self) {
        if let Some(url) = self.session.take_closed() {
            self.new_tab(Some(url), true);
        }
    }

    fn cycle_tab(&mut self, delta: i32) {
        if self.tabs.len() < 2 {
            return;
        }
        let n = self.tabs.len() as i32;
        let mut i = self.active as i32 + delta;
        if i < 0 {
            i = n - 1;
        }
        if i >= n {
            i = 0;
        }
        let id = self.tabs[i as usize].id;
        self.activate(id);
    }

    // ------------------------------------------------------------------- zoom

    fn set_zoom(&mut self, z: f64) {
        let z = z.clamp(0.25, 5.0);
        self.prefs.zoom = z;
        let _ = self.prefs.save();
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.zoom = z;
            if let Some(c) = &t.controller {
                unsafe {
                    let _ = c.SetZoomFactor(z);
                }
            }
        }
        self.push_state();
    }

    // -------------------------------------------------------------------- find

    fn find(&mut self, text: String, advance: bool) {
        let Some(wv) = self.tabs.get(self.active).and_then(|t| t.webview.clone()) else {
            return;
        };
        self.find_last = text.clone();
        let js = build_find_js(&text, self.prefs.find_case, advance);
        unsafe {
            let h = ExecuteScriptCompletedHandler::create(Box::new(move |hr, result: String| {
                if hr.is_ok() {
                    if let Ok(v) = serde_json::from_str::<Value>(&result) {
                        let count = v.get("count").and_then(|c| c.as_i64()).unwrap_or(0) as i32;
                        let index = v.get("index").and_then(|c| c.as_i64()).unwrap_or(0) as i32;
                        with_app(move |a| a.on_find_result(count, index));
                    }
                }
                Ok(())
            }));
            let _ = wv.ExecuteScript(&HSTRING::from(js), &h);
        }
    }

    fn on_find_result(&mut self, count: i32, index: i32) {
        self.find_count = count;
        self.find_index = index;
        self.push_state();
    }

    fn close_find(&mut self) {
        self.find_open = false;
        self.find_count = 0;
        self.find_index = 0;
        self.find_last.clear();
        if let Some(wv) = self.tabs.get(self.active).and_then(|t| t.webview.clone()) {
            unsafe {
                let _ = wv.ExecuteScript(
                    &HSTRING::from("try{CSS.highlights.delete('rc-find')}catch(e){}"),
                    &ExecuteScriptCompletedHandler::create(Box::new(|_h, _r: String| Ok(()))),
                );
            }
        }
        self.push_state();
    }

    // -------------------------------------------------------------- bookmarks

    fn toggle_bookmark(&mut self) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let url = t.url.clone();
        if url.is_empty() || t.is_new_tab {
            return;
        }
        let title = if t.title.is_empty() {
            url.clone()
        } else {
            t.title.clone()
        };
        if let Some(pos) = self.bookmarks.iter().position(|b| b.url == url) {
            self.bookmarks.remove(pos);
        } else {
            self.bookmarks.push(Bookmark { title, url });
        }
        let _ = store::save_bookmarks(&self.bookmarks);
        self.push_state();
    }

    // -------------------------------------------------------------- downloads

    fn on_download(&mut self, args: ICoreWebView2DownloadStartingEventArgs) {
        let mut uri = String::new();
        unsafe {
            if let Ok(op) = args.DownloadOperation() {
                let mut p = PWSTR::null();
                if op.Uri(&mut p).is_ok() {
                    uri = webview::take(p);
                }
            }
        }
        if uri.is_empty() {
            uri = self
                .tabs
                .get(self.active)
                .map(|t| t.url.clone())
                .unwrap_or_default();
        }
        let name = downloads::suggested_name(&args, &uri);
        let dest = downloads::unique_path(&downloads::downloads_dir(), &name);
        let dest_str = dest.to_string_lossy().to_string();

        unsafe {
            if args
                .SetResultFilePath(&HSTRING::from(dest_str.as_str()))
                .is_err()
            {
                let _ = args.SetCancel(true);
                return;
            }
            let _ = args.SetHandled(true);
        }

        let id = self.next_dl;
        self.next_dl += 1;
        self.downloads.insert(
            0,
            downloads::Item {
                id,
                name: dest
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| name.clone()),
                path: dest_str,
                state: "progressing".into(),
                received: 0,
                total: 0,
            },
        );

        // Attach progress/state reporting.
        unsafe {
            if let Ok(op) = args.DownloadOperation() {
                self.dl_ops.push((id, op.clone()));
                let h = webview2_com::BytesReceivedChangedEventHandler::create(Box::new(
                    move |sender, _a| {
                        if let Some(op) = sender {
                            let mut got = 0i64;
                            let mut total = 0i64;
                            let _ = op.BytesReceived(&mut got);
                            let _ = op.TotalBytesToReceive(&mut total);
                            with_app(move |a| a.update_download(id, got, total, None));
                        }
                        Ok(())
                    },
                ));
                let mut t = 0i64;
                let _ = op.add_BytesReceivedChanged(&h, &mut t);

                let h =
                    webview2_com::StateChangedEventHandler::create(Box::new(move |sender, _a| {
                        if let Some(op) = sender {
                            let mut st = COREWEBVIEW2_DOWNLOAD_STATE(0);
                            let _ = op.State(&mut st);
                            with_app(move |a| a.update_download(id, -1, -1, Some(st)));
                        }
                        Ok(())
                    }));
                let mut t = 0i64;
                let _ = op.add_StateChanged(&h, &mut t);
            }
        }
        self.push_state();
    }

    fn update_download(
        &mut self,
        id: u64,
        got: i64,
        total: i64,
        state: Option<COREWEBVIEW2_DOWNLOAD_STATE>,
    ) {
        if let Some(d) = self.downloads.iter_mut().find(|d| d.id == id) {
            if got >= 0 {
                d.received = got;
            }
            if total >= 0 {
                d.total = total;
            }
            if let Some(s) = state {
                d.state = downloads::map_state(s).to_string();
            }
        }
        self.push_state();
    }

    // ------------------------------------------------------------------ theme

    fn apply_pref_side_effects(&mut self, key: &str) {
        match key {
            "theme" | "force_dark" | "antifp" => {
                if key == "theme" {
                    window::apply_frame_theme(self.hwnd, self.prefs.is_dark());
                }
                for idx in 0..self.tabs.len() {
                    if key == "antifp" {
                        let ua = if self.prefs.antifp {
                            Some(self.ua.clone())
                        } else {
                            None
                        };
                        if let Some(wv) = self.tabs[idx].webview.clone() {
                            privacy::set_user_agent(&wv, ua.as_deref());
                        }
                    }
                    self.reinstall_scripts(idx);
                    if let Some(wv) = self.tabs[idx].webview.clone() {
                        unsafe {
                            let _ = wv.Reload();
                        }
                    }
                }
                if let Some(ch) = &self.chrome {
                    privacy::configure(&ch.webview, &self.prefs, true);
                }
            }
            "show_bookmarks" => {
                if let Some(ch) = &mut self.chrome {
                    ch.height_css = if self.prefs.show_bookmarks {
                        118.0
                    } else {
                        88.0
                    };
                }
                self.relayout();
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------- ui io

    fn tab_search_hits(&self) -> Vec<u64> {
        let pairs: Vec<(String, String)> = self
            .tabs
            .iter()
            .map(|t| (t.title.clone(), t.url.clone()))
            .collect();
        features::match_tabs(&self.tab_search_query, &pairs)
            .into_iter()
            .filter_map(|i| self.tabs.get(i).map(|t| t.id))
            .collect()
    }

    fn import_bookmarks(&mut self, path: Option<String>) {
        let source = match path {
            Some(p) => PathBuf::from(p.trim().trim_matches('"')),
            None => match features::chrome_bookmarks_path() {
                Some(p) => p,
                None => {
                    self.notice =
                        Some("No Chrome profile found. Paste a bookmarks file path.".into());
                    self.push_state();
                    return;
                }
            },
        };
        match features::parse_bookmarks_file(&source) {
            Ok(items) => {
                let (added, skipped) = features::merge_bookmarks(&mut self.bookmarks, items);
                let _ = store::save_bookmarks(&self.bookmarks);
                self.notice = Some(format!(
                    "Imported {added} bookmark(s), skipped {skipped} duplicate(s) from {}",
                    source
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                ));
            }
            Err(e) => self.notice = Some(format!("Import failed: {e}")),
        }
        self.push_state();
    }

    fn export_bookmarks(&mut self) {
        let dest = downloads::downloads_dir().join("rustchrome-bookmarks.html");
        let html = features::export_netscape(&self.bookmarks);
        match std::fs::write(&dest, html) {
            Ok(()) => {
                self.notice = Some(format!(
                    "Exported {} bookmark(s) to {}",
                    self.bookmarks.len(),
                    dest.display()
                ));
                downloads::reveal(&dest.to_string_lossy());
            }
            Err(e) => self.notice = Some(format!("Export failed: {e}")),
        }
        self.push_state();
    }

    fn add_engine(&mut self, label: &str, template: &str) {
        let Some(t) = features::normalize_engine_template(template) else {
            self.notice =
                Some("Search engine template must be an http(s) URL containing {q}".into());
            self.push_state();
            return;
        };
        let label = if label.trim().is_empty() {
            "Custom"
        } else {
            label.trim()
        };
        let id = format!(
            "custom-{}",
            label
                .to_ascii_lowercase()
                .replace(|c: char| !c.is_alphanumeric(), "-")
        );
        self.prefs.engines.retain(|e| e.id != id);
        self.prefs.engines.push(features::CustomEngine {
            id: id.clone(),
            label: label.to_string(),
            template: t,
        });
        let _ = self.prefs.save();
        self.notice = Some(format!("Added search engine {label}"));
        self.push_state();
    }

    fn remove_engine(&mut self, idx: usize) {
        if idx < self.prefs.engines.len() {
            let removed = self.prefs.engines.remove(idx);
            if self.prefs.engine == removed.id {
                self.prefs.engine = "google".to_string();
            }
            let _ = self.prefs.save();
        }
        self.push_state();
    }

    fn add_site_rule(&mut self, host: &str, kind: &str, allow: bool) {
        let host = host.trim().to_ascii_lowercase();
        let host = features::host_of(&format!("https://{host}")).unwrap_or(host);
        if host.is_empty() {
            self.notice = Some("Site rule needs a host name".into());
            self.push_state();
            return;
        }
        self.sites.retain(|r| !(r.host == host && r.kind == kind));
        self.sites.push(features::SiteRule {
            host: host.clone(),
            kind: kind.to_string(),
            allow,
        });
        let _ = features::save_sites(&self.sites);
        self.notice = Some(format!(
            "{} {} for {host}",
            if allow { "Allowed" } else { "Denied" },
            kind
        ));
        self.push_state();
    }

    fn cancel_download(&mut self, id: u64) {
        if let Some((_, op)) = self.dl_ops.iter().find(|(i, _)| *i == id) {
            unsafe {
                let _ = op.Cancel();
            }
        }
        self.push_state();
    }

    /// Mirror the live tab set into `session.json`. Cheap (a few hundred
    /// bytes) and called on every change, which is what makes crash recovery
    /// accurate without a timer.
    fn persist_session(&mut self) {
        let urls: Vec<String> = self
            .tabs
            .iter()
            .map(|t| t.url.clone())
            .filter(|u| !u.is_empty())
            .collect();
        if urls.is_empty() {
            return;
        }
        self.session.tabs = urls;
        self.session.active = self.active;
        self.session.dirty = true;
        let _ = self.session.save();
    }

    pub fn push_state(&self) {
        let Some(ch) = &self.chrome else { return };
        let json = self.snapshot().to_string();
        chrome::post(&ch.webview, &json);
    }

    fn snapshot(&self) -> Value {
        let tabs: Vec<Value> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                json!({
                    "id": t.id,
                    "title": if t.title.is_empty() { "New Tab" } else { t.title.as_str() },
                    "loading": t.loading,
                    "active": i == self.active,
                })
            })
            .collect();

        let active = self.tabs.get(self.active);
        let (can_back, can_forward, loading) = match active {
            Some(t) => {
                let (mut b, mut f) = (false, false);
                if let Some(wv) = &t.webview {
                    let mut bb = BOOL(0);
                    let mut ff = BOOL(0);
                    unsafe {
                        let _ = wv.CanGoBack(&mut bb);
                        let _ = wv.CanGoForward(&mut ff);
                    }
                    b = bb.as_bool();
                    f = ff.as_bool();
                }
                (b, f, t.loading)
            }
            None => (false, false, false),
        };

        let url = match active {
            Some(t) if !t.is_new_tab => t.url.clone(),
            _ => String::new(),
        };

        let marked = self
            .bookmarks
            .iter()
            .any(|b| !url.is_empty() && b.url == url);

        let bms: Vec<Value> = self
            .bookmarks
            .iter()
            .map(|b| json!({ "title": b.title, "url": b.url }))
            .collect();

        let downloads: Vec<Value> = self
            .downloads
            .iter()
            .take(20)
            .map(|d| {
                json!({
                    "id": d.id, "name": d.name, "path": d.path,
                    "state": d.state, "received": d.received, "total": d.total
                })
            })
            .collect();

        let mut engines: Vec<Value> = rc_core::SEARCH_ENGINES
            .iter()
            .map(|e| json!({ "id": e.0, "label": e.1 }))
            .collect();
        let custom: Vec<Value> = self
            .prefs
            .engines
            .iter()
            .enumerate()
            .map(|(i, e)| json!({ "id": e.id, "label": e.label, "custom": true, "index": i }))
            .collect();
        for c in &custom {
            engines.push(c.clone());
        }

        let closed: Vec<Value> = self
            .session
            .closed
            .iter()
            .rev()
            .map(|u| json!({ "url": u }))
            .collect();
        let hits: Vec<Value> = self
            .tab_search_hits()
            .iter()
            .enumerate()
            .filter_map(|(i, id)| {
                self.tabs.iter().find(|t| t.id == *id).map(|t| {
                    json!({ "index": i, "id": t.id, "title": if t.title.is_empty() { "New Tab" } else { t.title.as_str() }, "url": t.url })
                })
            })
            .collect();
        let site_rules: Vec<Value> = self
            .sites
            .iter()
            .enumerate()
            .map(|(i, r)| json!({ "index": i, "host": r.host, "kind": r.kind, "allow": r.allow }))
            .collect();

        let engine_label = rc_core::SEARCH_ENGINES
            .iter()
            .find(|e| e.0 == self.prefs.engine)
            .map(|e| e.1)
            .unwrap_or("Google");

        json!({
            "tabs": tabs,
            "engine_label": engine_label,
            "url": url,
            "can_back": can_back,
            "can_forward": can_forward,
            "loading": loading,
            "bookmarked": marked,
            "bookmarks": bms,
            "downloads": downloads,
            "engines": engines,
            "find": { "open": self.find_open, "count": self.find_count, "index": self.find_index },
            "prefs": {
                "theme": self.prefs.theme,
                "force_dark": self.prefs.force_dark,
                "antifp": self.prefs.antifp,
                "strict": self.prefs.strict,
                "keep_session": self.prefs.keep_session,
                "show_bookmarks": self.prefs.show_bookmarks,
                "engine": self.prefs.engine,
                "zoom": self.prefs.zoom,
            },
            "version": env!("CARGO_PKG_VERSION"),
            "runtime": self.runtime,
            "closed": closed,
            "tabsearch": { "open": self.tab_search_open, "query": self.tab_search_query, "hits": hits },
            "site_rules": site_rules,
            "custom_engines": custom,
            "notice": self.notice,
            "downloads_dir": downloads::downloads_dir().to_string_lossy(),
            "userscripts_dir": store::userscript_dir().to_string_lossy(),
        })
    }

    fn on_ui_message(&mut self, raw: &str) {
        let Ok(msg) = serde_json::from_str::<chrome::Msg>(raw) else {
            return;
        };
        self.ui_msgs += 1;
        match msg.cmd.as_str() {
            "navigate" => {
                if let Some(text) = msg.text {
                    self.navigate_active(&text);
                }
            }
            "newtab" => self.new_tab(None, true),
            "closetab" => {
                let id = msg.id.unwrap_or(0);
                self.close_tab(id);
            }
            "activate" => {
                if let Some(id) = msg.id {
                    self.activate(id);
                }
            }
            "reopen" => self.reopen_closed(),
            "reopenat" => {
                if let Some(i) = msg.index {
                    if let Some(url) = self.session.closed.get(i).cloned() {
                        self.session.closed.remove(i);
                        self.new_tab(Some(url), true);
                    }
                }
            }
            "clearsession" => {
                self.session.closed.clear();
                let _ = self.session.save();
                self.push_state();
            }
            "tabsearch" => {
                self.tab_search_open = true;
                self.tab_search_query = msg.text.unwrap_or_default();
                self.push_state();
            }
            "tabsearchclose" => {
                self.tab_search_open = false;
                self.tab_search_query.clear();
                self.push_state();
            }
            "tabsearchpick" => {
                if let Some(i) = msg.index {
                    let hits = self.tab_search_hits();
                    if let Some(id) = hits.get(i).copied() {
                        self.tab_search_open = false;
                        self.tab_search_query.clear();
                        self.activate(id);
                    }
                }
            }
            "importbookmarks" => self.import_bookmarks(msg.path_or_text()),
            "exportbookmarks" => self.export_bookmarks(),
            "addengine" => {
                let label = msg.label.unwrap_or_default();
                let template = msg.template.unwrap_or_default();
                self.add_engine(&label, &template);
            }
            "removeengine" => {
                if let Some(i) = msg.index {
                    self.remove_engine(i);
                }
            }
            "addrule" => {
                let host = msg.text.unwrap_or_default();
                let kind = msg.kind.unwrap_or_else(|| "camera".into());
                let allow = msg.allow.unwrap_or(true);
                self.add_site_rule(&host, &kind, allow);
            }
            "removerule" => {
                if let Some(i) = msg.index {
                    self.sites.remove(i);
                    let _ = features::save_sites(&self.sites);
                    self.push_state();
                }
            }
            "canceldownload" => {
                if let Some(id) = msg.id {
                    self.cancel_download(id);
                }
            }
            "back" => self.with_active(|wv| unsafe {
                let _ = wv.GoBack();
            }),
            "forward" => self.with_active(|wv| unsafe {
                let _ = wv.GoForward();
            }),
            "reload" => self.with_active(|wv| unsafe {
                let _ = wv.Reload();
            }),
            "stop" => self.with_active(|wv| unsafe {
                let _ = wv.Stop();
            }),
            "home" => {
                let idx = self.active;
                self.show_new_tab(idx);
                self.push_state();
            }
            "findopen" => {
                self.find_open = true;
                self.push_state();
            }
            "find" => {
                self.find_open = true;
                let text = msg.text.unwrap_or_default();
                let advance = msg.back.unwrap_or(false);
                if !text.is_empty() {
                    self.find(text, advance);
                }
                self.push_state();
            }
            "findclose" => self.close_find(),
            "zoom" => {
                let delta = msg.value.as_ref().and_then(|v| v.as_f64()).unwrap_or(0.0);
                let base = self.tabs.get(self.active).map(|t| t.zoom).unwrap_or(1.0);
                self.set_zoom(base + delta);
            }
            "zoomreset" => self.set_zoom(1.0),
            "bookmark" => self.toggle_bookmark(),
            "setpref" => {
                let key = msg.key.unwrap_or_default();
                if let Some(v) = msg.value {
                    if self.prefs.set_json(&key, &v) {
                        let _ = self.prefs.save();
                        self.apply_pref_side_effects(&key);
                        self.push_state();
                    }
                }
            }
            "height" => {
                let mut changed = false;
                if let (Some(h), Some(ch)) = (msg.height, &mut self.chrome) {
                    if h > 20.0 && (h - ch.height_css).abs() > 0.5 {
                        ch.height_css = h;
                        changed = true;
                    }
                }
                if changed {
                    self.relayout();
                }
            }
            "reveal" => {
                if let Some(id) = msg.id {
                    if let Some(d) = self.downloads.iter().find(|d| d.id == id) {
                        downloads::reveal(&d.path);
                    }
                }
            }
            "opendownloads" => downloads::open_dir(&downloads::downloads_dir()),
            "openuserscripts" => downloads::open_dir(&store::userscript_dir()),
            "openfolder" => {
                let which = msg.which.unwrap_or_default();
                let dir = if which == "userscripts" {
                    store::userscript_dir()
                } else {
                    downloads::downloads_dir()
                };
                downloads::open_dir(&dir);
            }
            "page" => {
                let which = msg.which.unwrap_or_default();
                self.open_internal_page(&which);
            }
            "reloaduserscripts" => {
                for idx in 0..self.tabs.len() {
                    self.reinstall_scripts(idx);
                    if let Some(wv) = self.tabs[idx].webview.clone() {
                        unsafe {
                            let _ = wv.Reload();
                        }
                    }
                }
                self.push_state();
            }
            "newidentity" => self.new_identity(),
            "focusurl" => {
                if let Some(ch) = &self.chrome {
                    unsafe {
                        let _ = ch
                            .controller
                            .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
                    }
                }
            }
            "quit" => {
                self.closing = true;
                unsafe {
                    let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
            _ => {}
        }
    }

    fn with_active(&mut self, f: impl FnOnce(&ICoreWebView2)) {
        if let Some(wv) = self.tabs.get(self.active).and_then(|t| t.webview.clone()) {
            f(&wv);
        }
        self.push_state();
    }

    fn new_identity(&mut self) {
        // A new identity means a new canvas key as well as fresh storage.
        self.seed.rotate();
        // Close every tab, drop the closed-tab list, clear data, start fresh.
        let ids: Vec<u64> = self.tabs.iter().map(|t| t.id).collect();
        for id in ids {
            self.close_tab(id);
        }
        self.session.closed.clear();
        self.bookmarks = store::load_bookmarks();
        self.downloads.clear();
        self.find_open = false;
        self.clear_browsing_data();
        self.new_tab(None, true);
        self.push_state();
    }

    fn clear_browsing_data(&self) {
        let Some(wv) = self.tabs.first().and_then(|t| t.webview.clone()) else {
            return;
        };
        unsafe {
            if let Ok(w13) = wv.cast::<ICoreWebView2_13>() {
                if let Ok(p) = w13.Profile() {
                    if let Ok(p2) = p.cast::<ICoreWebView2Profile2>() {
                        let (tx, rx) = std::sync::mpsc::channel::<()>();
                        let h = ClearBrowsingDataCompletedHandler::create(Box::new(move |_hr| {
                            let _ = tx.send(());
                            Ok(())
                        }));
                        if p2.ClearBrowsingDataAll(&h).is_ok() {
                            pump_until(rx, Duration::from_secs(2));
                        }
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- keyboard

    fn on_key(&mut self, vk: u32) -> bool {
        let (ctrl, shift, alt) = unsafe {
            (
                GetKeyState(0x11) < 0,
                GetKeyState(0x10) < 0,
                GetKeyState(0x12) < 0,
            )
        };
        const F5: u32 = 0x74;
        const ESC: u32 = 0x1B;
        const TAB: u32 = 0x09;
        const LEFT: u32 = 0x25;
        const RIGHT: u32 = 0x27;
        const PLUS: u32 = 0xBB;
        const MINUS: u32 = 0xBD;
        const ADD: u32 = 0x6B;
        const SUB: u32 = 0x6D;
        const ZERO: u32 = 0x30;

        if ctrl && shift && (vk == 0x54 || vk == 0x09) {
            // Ctrl+Shift+T (reopen) / Ctrl+Shift+Tab (previous tab)
            if vk == 0x54 {
                self.reopen_closed();
            } else {
                self.cycle_tab(-1);
            }
            return true;
        }
        if ctrl && !shift {
            match vk {
                0x54 => {
                    self.new_tab(None, true);
                    return true;
                }
                0x57 => {
                    let id = self.tabs.get(self.active).map(|t| t.id).unwrap_or(0);
                    if id != 0 {
                        self.close_tab(id);
                    }
                    return true;
                }
                0x4C => {
                    self.focus_omnibox();
                    return true;
                }
                0x52 => {
                    self.with_active(|wv| unsafe {
                        let _ = wv.Reload();
                    });
                    return true;
                }
                0x46 => {
                    self.find_open = true;
                    self.push_state();
                    return true;
                }
                0x44 => {
                    self.toggle_bookmark();
                    return true;
                }
                TAB => {
                    self.cycle_tab(1);
                    return true;
                }
                PLUS | ADD => {
                    let base = self.tabs.get(self.active).map(|t| t.zoom).unwrap_or(1.0);
                    self.set_zoom(base + 0.1);
                    return true;
                }
                MINUS | SUB => {
                    let base = self.tabs.get(self.active).map(|t| t.zoom).unwrap_or(1.0);
                    self.set_zoom(base - 0.1);
                    return true;
                }
                ZERO => {
                    self.set_zoom(1.0);
                    return true;
                }
                _ => {}
            }
        }
        if !ctrl && !alt {
            match vk {
                F5 => {
                    self.with_active(|wv| unsafe {
                        let _ = wv.Reload();
                    });
                    return true;
                }
                ESC => {
                    if self.find_open {
                        self.close_find();
                        return true;
                    }
                    return false;
                }
                _ => {}
            }
        }
        if alt && vk == LEFT {
            self.with_active(|wv| unsafe {
                let _ = wv.GoBack();
            });
            return true;
        }
        if alt && vk == RIGHT {
            self.with_active(|wv| unsafe {
                let _ = wv.GoForward();
            });
            return true;
        }
        false
    }

    fn focus_omnibox(&mut self) {
        self.push_state();
        if let Some(ch) = &self.chrome {
            unsafe {
                let _ = ch
                    .controller
                    .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
                let _ = ch.webview.ExecuteScript(
                    &HSTRING::from(
                        "(()=>{const e=document.getElementById('omnibox');if(e){e.focus();e.select();}})()",
                    ),
                    &ExecuteScriptCompletedHandler::create(Box::new(|_h, _r: String| Ok(()))),
                );
            }
        }
    }

    // ------------------------------------------------------------------ layout

    pub fn relayout(&self) {
        let (w, h) = window::client_size(self.hwnd);
        let scale = window::dpi_scale(self.hwnd);
        let chrome_px = ((self.chrome.as_ref().map(|c| c.height_css).unwrap_or(96.0)) * scale)
            .round()
            .clamp(0.0, h as f64) as i32;

        if self.smoke {
            self.trace(&format!(
                "relayout: client={}x{} scale={} chrome_px={} chrome_present={} ui_msgs={}",
                w,
                h,
                scale,
                chrome_px,
                self.chrome.is_some(),
                self.ui_msgs
            ));
        }

        if let Some(ch) = &self.chrome {
            let rect = RECT {
                left: 0,
                top: 0,
                right: w,
                bottom: chrome_px,
            };
            webview::set_rasterization(&ch.controller, scale);
            // SAFETY: WebView2 controllers are apartment-threaded and every call
            // here happens on the UI thread that created them; `rect` is a plain
            // POD struct with no interior pointers.
            let hr = unsafe { ch.controller.SetBounds(rect) };
            // A failed SetBounds used to be indistinguishable from a working one:
            // the chrome webview existed, was visible, reported success to its
            // caller, and painted nothing. Count the failures instead.
            if hr.is_err() {
                BOUNDS_FAILURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            if self.smoke {
                let title = unsafe {
                    let mut p = PWSTR::null();
                    if ch.webview.DocumentTitle(&mut p).is_ok() {
                        webview::take(p)
                    } else {
                        String::new()
                    }
                };
                self.trace(&format!(
                    "chrome bounds {{0,0,{},{}}} hr={:?} title={:?}",
                    w,
                    chrome_px,
                    hr.map(|_| 0i32),
                    title
                ));
            }
        }
        let content = RECT {
            left: 0,
            top: chrome_px,
            right: w,
            bottom: h,
        };
        for t in &self.tabs {
            if let Some(c) = &t.controller {
                webview::set_rasterization(c, scale);
                // SAFETY: same apartment and thread as the controller above.
                let hr = unsafe { c.SetBounds(content) };
                if hr.is_err() {
                    BOUNDS_FAILURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                if self.smoke {
                    self.trace(&format!(
                        "tab {} bounds {{0,{},,{}}} hr={:?}",
                        t.id,
                        chrome_px,
                        h,
                        hr.map(|_| 0i32)
                    ));
                }
            }
        }
    }

    pub fn on_dpi_changed(&mut self, lp: LPARAM) {
        // lParam points at a RECT suggested by Windows; adopt it.
        if lp.0 != 0 {
            let r = unsafe { *(lp.0 as *const RECT) };
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
        self.relayout();
    }

    fn finish_smoke(&self, ok: bool, status: i32) {
        let out = std::env::var("RUSTCHROME_SMOKE_OUT").unwrap_or_else(|_| "smoke.txt".into());
        let url = self.tabs.first().map(|t| t.url.clone()).unwrap_or_default();
        let text = if ok {
            format!("OK url={url} runtime={}\n", self.runtime)
        } else {
            format!("FAIL status={status} url={url}\n")
        };
        let _ = std::fs::write(out, text);
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

fn with_app_value<T, F: FnOnce(&mut App) -> T>(f: F) -> Option<T> {
    let mut out = None;
    with_app(|a| out = Some(f(a)));
    out
}

/// Permission kinds as stable names, used by the per-site rules.
fn permission_name(kind: COREWEBVIEW2_PERMISSION_KIND) -> &'static str {
    match kind.0 {
        1 => "microphone",
        2 => "camera",
        3 => "geolocation",
        4 => "notifications",
        5 => "sensors",
        6 => "clipboard",
        7 => "downloads",
        8 => "files",
        9 => "autoplay",
        10 => "midi",
        11 => "windows",
        _ => "unknown",
    }
}

fn with_app_handled<F: FnOnce(&mut App) -> bool>(f: F) -> bool {
    let mut out = false;
    with_app(|a| out = f(a));
    out
}

fn build_find_js(term: &str, case: bool, advance: bool) -> String {
    let t = serde_json::to_string(term).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"(() => {{
  try {{
    const term = {t}, cs = {cs}, advance = {adv};
    if (!term) {{ CSS.highlights.delete('rc-find'); return JSON.stringify({{count:0,index:0}}); }}
    if (!document.getElementById('rc-find-style')) {{
      const st = document.createElement('style');
      st.id = 'rc-find-style';
      st.textContent = '::highlight(rc-find){{background:#ffe066;color:#000}}';
      (document.head || document.documentElement).appendChild(st);
    }}
    if (window.__rcTerm !== term || window.__rcCase !== cs) {{
      window.__rcTerm = term; window.__rcCase = cs; window.__rcIdx = 0;
      const ranges = [];
      const walker = document.createTreeWalker(document.body || document.documentElement, NodeFilter.SHOW_TEXT);
      const needle = cs ? term : term.toLowerCase();
      let node;
      while ((node = walker.nextNode())) {{
        const text = node.nodeValue || '';
        const hay = cs ? text : text.toLowerCase();
        let from = 0, i;
        while ((i = hay.indexOf(needle, from)) !== -1) {{
          const r = document.createRange();
          r.setStart(node, i); r.setEnd(node, i + needle.length);
          ranges.push(r); from = i + Math.max(1, needle.length);
          if (ranges.length >= 3000) break;
        }}
        if (ranges.length >= 3000) break;
      }}
      window.__rcRanges = ranges;
      CSS.highlights.set('rc-find', new Highlight(...ranges));
    }}
    const ranges = window.__rcRanges || [];
    if (advance && ranges.length) window.__rcIdx = (window.__rcIdx + 1) % ranges.length;
    const r = ranges[window.__rcIdx];
    if (r && r.startContainer && r.startContainer.parentElement) {{
      r.startContainer.parentElement.scrollIntoView({{block:'center', behavior:'instant'}});
    }}
    return JSON.stringify({{count: ranges.length, index: ranges.length ? window.__rcIdx + 1 : 0}});
  }} catch (e) {{ return JSON.stringify({{count:0,index:0,error:String(e)}}); }}
}})()"#,
        t = t,
        cs = if case { "true" } else { "false" },
        adv = if advance { "true" } else { "false" },
    )
}

/// Dispatch messages while waiting for an async completion, with a deadline so
/// shutdown can never hang on a stuck WebView2 call.
fn pump_until(rx: std::sync::mpsc::Receiver<()>, timeout: Duration) {
    let start = Instant::now();
    loop {
        if rx.try_recv().is_ok() {
            return;
        }
        if start.elapsed() > timeout {
            return;
        }
        let mut msg = MSG::default();
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
                if rx.try_recv().is_ok() {
                    return;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

// ------------------------------------------------------------- window messages

pub unsafe extern "system" fn dispatch(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            with_app(|a| a.relayout());
            LRESULT(0)
        }
        WM_DPICHANGED => {
            with_app(|a| a.on_dpi_changed(lp));
            LRESULT(0)
        }
        WM_SETFOCUS => {
            with_app(|a| a.focus_active_tab());
            LRESULT(0)
        }
        WM_CLOSE => {
            let mut first = false;
            with_app(|a| {
                if !a.closing {
                    a.closing = true;
                    first = true;
                }
            });
            if first {
                run_shutdown(hwnd);
            } else {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

impl App {
    fn focus_active_tab(&mut self) {
        if let Some(c) = self
            .tabs
            .get(self.active)
            .and_then(|t| t.controller.clone())
        {
            unsafe {
                let _ = c.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
            }
        }
    }
}

/// Close controllers, clear browsing data (bounded), then destroy the window.
fn run_shutdown(hwnd: HWND) {
    let mut profile: Option<ICoreWebView2Profile> = None;
    let mut controllers: Vec<ICoreWebView2Controller> = Vec::new();

    with_app(|a| {
        for t in &a.tabs {
            if let Some(c) = &t.controller {
                controllers.push(c.clone());
            }
        }
        if let Some(wv) = a.tabs.first().and_then(|t| t.webview.clone()) {
            if let Ok(w13) = wv.cast::<ICoreWebView2_13>() {
                profile = unsafe { w13.Profile().ok() };
            }
        }
    });

    if let Some(p) = profile {
        if let Ok(p2) = p.cast::<ICoreWebView2Profile2>() {
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            unsafe {
                let h = ClearBrowsingDataCompletedHandler::create(Box::new(move |_hr| {
                    let _ = tx.send(());
                    Ok(())
                }));
                if p2.ClearBrowsingDataAll(&h).is_ok() {
                    pump_until(rx, Duration::from_secs(3));
                }
            }
        }
    }

    // A clean exit: the session is no longer "dirty", so the next start does
    // not treat this as a crash.
    with_app(|a| {
        a.session.dirty = false;
        a.session.tabs.clear();
        let _ = a.session.save();
    });

    for c in controllers {
        unsafe {
            let _ = c.Close();
        }
    }
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}
