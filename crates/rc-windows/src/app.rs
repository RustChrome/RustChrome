//! Application state, tab management, event wiring and the browser commands.
//!
//! There is exactly one App per process and it lives on the UI thread inside a
//! `thread_local` Rc, because every WebView2 callback runs on that thread
//! anyway. Handlers capture a tab id, never `self`.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

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

// ---------------------------------------------------------------- global state

thread_local! {
    static APP: RefCell<Option<Rc<RefCell<App>>>> = RefCell::new(None);
}

pub fn install(app: Rc<RefCell<App>>) {
    APP.with(|c| *c.borrow_mut() = Some(app));
}

/// Run `f` with the app borrowed. Skipped (not panicked) if a handler fires
/// while we are already inside a borrow — losing one redraw beats crashing.
pub fn with_app<F: FnOnce(&mut App)>(f: F) {
    let rc = APP.with(|c| c.borrow().clone());
    if let Some(rc) = rc {
        if let Ok(mut a) = rc.try_borrow_mut() {
            f(&mut a);
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
    pending: Option<String>,
    loading: bool,
    is_new_tab: bool,
}

// ------------------------------------------------------------------------ app

pub struct App {
    pub hwnd: HWND,
    pub env: ICoreWebView2Environment,
    pub prefs: Prefs,
    pub chrome: Option<chrome::Chrome>,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub next_id: u64,
    pub closed: Vec<String>,
    pub bookmarks: Vec<Bookmark>,
    pub downloads: Vec<downloads::Item>,
    pub next_dl: u64,
    pub find_open: bool,
    pub find_count: i32,
    pub find_index: i32,
    pub find_last: String,
    pub closing: bool,
    pub smoke: bool,
    pub runtime: String,
}

impl App {
    pub fn new(hwnd: HWND, env: ICoreWebView2Environment, smoke: bool) -> Self {
        let runtime = webview::runtime_version().unwrap_or_else(|| "unknown".into());
        Self {
            hwnd,
            env,
            prefs: Prefs::load(),
            chrome: None,
            tabs: Vec::new(),
            active: 0,
            next_id: 1,
            closed: Vec::new(),
            bookmarks: store::load_bookmarks(),
            downloads: Vec::new(),
            next_dl: 1,
            find_open: false,
            find_count: 0,
            find_index: 0,
            find_last: String::new(),
            closing: false,
            smoke,
            runtime,
        }
    }

    /// Kick off the UI webview and the first tab.
    pub fn boot(&mut self) {
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

    fn fatal(&self, msg: &str) {
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

            // The UI must never navigate anywhere.
            let mut token = 0i64;
            let h = NavigationStartingEventHandler::create(Box::new(|_s, args| {
                if let Some(args) = args {
                    let _ = args.SetCancel(true);
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
            height_css: if self.prefs.show_bookmarks { 118.0 } else { 88.0 },
        });
        chrome::load(&created.webview);
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
            pending: Some(url),
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
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        match res {
            Err(e) => {
                self.tabs[idx].title = "Tab failed".to_string();
                self.push_state();
                window::message_box(
                    self.hwnd,
                    &format!("A tab could not be created.\n\n{e}"),
                    "RustChrome",
                );
            }
            Ok(created) => {
                let is_active = idx == self.active;
                privacy::configure(&created.webview, &self.prefs, false);
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

                let pending = self.tabs[idx].pending.take();
                if let Some(url) = pending {
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
            let h = PermissionRequestedEventHandler::create(Box::new(|_s, args| {
                if let Some(args) = args {
                    let _ = args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY);
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
            privacy::add_script(&wv, privacy::antifp_js(), move |sid| {
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

    fn show_new_tab(&mut self, idx: usize) {
        let Some(wv) = self.tabs.get(idx).and_then(|t| t.webview.clone()) else {
            return;
        };
        let html = pages::new_tab_html(&self.prefs.engine, self.prefs.is_dark());
        self.tabs[idx].url.clear();
        self.tabs[idx].is_new_tab = true;
        self.tabs[idx].title = "New Tab".to_string();
        unsafe {
            let _ = wv.NavigateToString(&HSTRING::from(html));
        }
    }

    fn load_in(&mut self, idx: usize, url: &str) {
        let Some(wv) = self.tabs.get(idx).and_then(|t| t.webview.clone()) else {
            return;
        };
        self.tabs[idx].url = url.to_string();
        self.tabs[idx].is_new_tab = false;
        unsafe {
            let _ = wv.Navigate(&HSTRING::from(url));
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
                    t.pending = Some(url);
                }
                return;
            }
        }
        self.load_in(idx, &url);
        self.push_state();
    }

    fn on_nav_starting(&mut self, id: u64, uri: String, allowed: bool) {
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
        self.push_state();
    }

    fn on_title(&mut self, id: u64, title: String) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.title = title;
        }
        self.push_state();
    }

    fn on_nav_completed(&mut self, id: u64, ok: bool, status: i32) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.loading = false;
        }
        // WebView2 cancels the initial about:blank navigation when a controller
        // is created and we immediately load a page. That is not a failure, and
        // it must not produce an error page or fail the smoke test.
        let canceled = status == COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED.0;
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
        if self.smoke && id == self.tabs.first().map(|t| t.id).unwrap_or(0) && (ok || !canceled) {
            self.finish_smoke(ok, status);
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
            self.closed.push(tab.url);
            if self.closed.len() > 10 {
                self.closed.remove(0);
            }
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
        self.push_state();
    }

    fn reopen_closed(&mut self) {
        if let Some(url) = self.closed.pop() {
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
        let title = if t.title.is_empty() { url.clone() } else { t.title.clone() };
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
            if args.SetResultFilePath(&HSTRING::from(dest_str.as_str())).is_err() {
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
            if let Ok(op) = unsafe { args.DownloadOperation() } {
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

                let h = webview2_com::StateChangedEventHandler::create(Box::new(
                    move |sender, _a| {
                        if let Some(op) = sender {
                            let mut st = COREWEBVIEW2_DOWNLOAD_STATE(0);
                            let _ = op.State(&mut st);
                            with_app(move |a| a.update_download(id, -1, -1, Some(st)));
                        }
                        Ok(())
                    },
                ));
                let mut t = 0i64;
                let _ = op.add_StateChanged(&h, &mut t);
            }
        }
        self.push_state();
    }

    fn update_download(&mut self, id: u64, got: i64, total: i64, state: Option<COREWEBVIEW2_DOWNLOAD_STATE>) {
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
                for idx in 0..self.tabs.len() {
                    if key == "antifp" {
                        if let Some(wv) = self.tabs[idx].webview.clone() {
                            privacy::set_user_agent(&wv, self.prefs.antifp);
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
                    ch.height_css = if self.prefs.show_bookmarks { 118.0 } else { 88.0 };
                }
                self.relayout();
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------- ui io

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

        let engines: Vec<Value> = rc_core::SEARCH_ENGINES
            .iter()
            .map(|e| json!({ "id": e.0, "label": e.1 }))
            .collect();

        json!({
            "tabs": tabs,
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
                "keep_session": self.prefs.keep_session,
                "show_bookmarks": self.prefs.show_bookmarks,
                "engine": self.prefs.engine,
                "zoom": self.prefs.zoom,
            },
            "version": env!("CARGO_PKG_VERSION"),
            "runtime": self.runtime,
            "downloads_dir": downloads::downloads_dir().to_string_lossy(),
            "userscripts_dir": store::userscript_dir().to_string_lossy(),
        })
    }

    fn on_ui_message(&mut self, raw: &str) {
        let Ok(msg) = serde_json::from_str::<chrome::Msg>(raw) else {
            return;
        };
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
            "back" => self.with_active(|wv| unsafe { let _ = wv.GoBack(); }),
            "forward" => self.with_active(|wv| unsafe { let _ = wv.GoForward(); }),
            "reload" => self.with_active(|wv| unsafe { let _ = wv.Reload(); }),
            "stop" => self.with_active(|wv| unsafe { let _ = wv.Stop(); }),
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
                        let _ = ch.controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
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
        // Close every tab, drop the closed-tab list, clear data, start fresh.
        let ids: Vec<u64> = self.tabs.iter().map(|t| t.id).collect();
        for id in ids {
            self.close_tab(id);
        }
        self.closed.clear();
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

        if let Some(ch) = &self.chrome {
            let rect = RECT { left: 0, top: 0, right: w, bottom: chrome_px };
            webview::set_rasterization(&ch.controller, scale);
            unsafe {
                let _ = ch.controller.SetBounds(rect);
            }
        }
        let content = RECT { left: 0, top: chrome_px, right: w, bottom: h };
        for t in &self.tabs {
            if let Some(c) = &t.controller {
                webview::set_rasterization(c, scale);
                unsafe {
                    let _ = c.SetBounds(content);
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
        let url = self
            .tabs
            .first()
            .map(|t| t.url.clone())
            .unwrap_or_default();
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
        if let Some(c) = self.tabs.get(self.active).and_then(|t| t.controller.clone()) {
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

    for c in controllers {
        unsafe {
            let _ = c.Close();
        }
    }
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}
