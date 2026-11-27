//! The browser UI: a WebView2 that renders the tab strip, omnibox, toolbar,
//! menus and panels as HTML/CSS, and talks to Rust over `postMessage`.

use serde::Deserialize;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::{Interface, HSTRING};

pub const HTML: &str = include_str!("../assets/chrome.html");

pub struct Chrome {
    pub controller: ICoreWebView2Controller,
    pub webview: ICoreWebView2,
    /// Height of the UI in CSS pixels, reported by the page itself.
    pub height_css: f64,
}

/// Message sent from the UI to Rust. Unknown fields are ignored.
#[derive(Deserialize, Debug)]
pub struct Msg {
    pub cmd: String,
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default)]
    pub back: Option<bool>,
    #[serde(default)]
    pub which: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub allow: Option<bool>,
    #[serde(default)]
    pub index: Option<usize>,
}

impl Msg {
    /// A free-text field that doubles as a path for the bookmark importer.
    pub fn path_or_text(&self) -> Option<String> {
        self.text.clone().filter(|t| !t.trim().is_empty())
    }
}

/// Load the chrome into its webview.
///
/// `NavigateToString` was used here and it never worked: the navigation started
/// and never committed, so the UI webview held no document — a window with no
/// tabs, no toolbar and no address bar, while every WebView2 call returned
/// success and the built-in error page was disabled so nothing was shown. A
/// virtual host serving the same HTML from a real file is the reliable path for
/// a document this size.
///
/// Before writing the file we inject every registered theme's palette block so
/// `chrome.html` can flip themes from the persisted id (`data-theme`) without
/// restarting and without carrying a second copy of the colours.
pub fn load(webview: &ICoreWebView2) {
    let dir = rc_core::data_dir().join("ui");
    let html = themed_chrome_html();
    let written = std::fs::create_dir_all(&dir).is_ok()
        && std::fs::write(dir.join("chrome.html"), &html).is_ok();

    if written {
        if let Ok(w3) = webview.cast::<ICoreWebView2_3>() {
            unsafe {
                let ok = w3
                    .SetVirtualHostNameToFolderMapping(
                        &HSTRING::from("rustchrome.internal"),
                        &HSTRING::from(dir.to_string_lossy().as_ref()),
                        COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_DENY_CORS,
                    )
                    .is_ok();
                if ok {
                    let _ =
                        webview.Navigate(&HSTRING::from("https://rustchrome.internal/chrome.html"));
                    return;
                }
            }
        }
    }

    unsafe {
        // Fall back to the raw (light) chrome if the themed write failed.
        let _ = webview.NavigateToString(&HSTRING::from(HTML));
    }
}

/// The chrome document with every theme's palette block injected before
/// `</style>`, right after the hardcoded light `:root` block. The light values
/// stay as the base; each `html[data-theme="…"]` block overrides them.
fn themed_chrome_html() -> String {
    let mut blocks = String::new();
    for theme in rc_core::themes::THEMES {
        blocks.push_str(&theme.palette.windows_chrome_block(theme.id));
    }
    // Anchor onto the same closing tag the template's own `<style>` uses.
    HTML.replacen("</style>", &format!("{blocks}</style>"), 1)
}

pub fn post(webview: &ICoreWebView2, json: &str) {
    unsafe {
        let _ = webview.PostWebMessageAsJson(&HSTRING::from(json));
    }
}
