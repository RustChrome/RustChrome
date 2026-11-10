//! The browser UI: a WebView2 that renders the tab strip, omnibox, toolbar,
//! menus and panels as HTML/CSS, and talks to Rust over `postMessage`.

use serde::Deserialize;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::HSTRING;

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

pub fn load(webview: &ICoreWebView2) {
    unsafe {
        let _ = webview.NavigateToString(&HSTRING::from(HTML));
    }
}

pub fn post(webview: &ICoreWebView2, json: &str) {
    unsafe {
        let _ = webview.PostWebMessageAsJson(&HSTRING::from(json));
    }
}
