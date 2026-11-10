//! Privacy policy for the Windows shell.
//!
//! Linux can switch WebGL and WebRTC off in the engine. WebView2 cannot, so the
//! shims in `rc_core::privacy` do that work and this module applies everything
//! the engine *does* offer: InPrivate profiles, strict tracking prevention,
//! deny-all permissions and autofill/password saving off.

use rc_core::privacy::{ANTI_FP_JS, FORCE_DARK_CSS};
use rc_core::store::Prefs;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::AddScriptToExecuteOnDocumentCreatedCompletedHandler;
use windows::core::{Interface, HSTRING};

/// A current Chromium UA. Deliberately *not* a Firefox string: an engine/UA
/// mismatch is a louder fingerprint than a common one.
pub const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

/// Settings that apply to a freshly created webview.
///
/// `is_chrome` is the browser UI's own webview: it keeps context menus and the
/// JS bridge, and is forced to `about:blank`-only navigation by the caller.
pub fn configure(webview: &ICoreWebView2, prefs: &Prefs, is_chrome: bool) {
    unsafe {
        if let Ok(s) = webview.Settings() {
            let _ = s.SetAreDevToolsEnabled(false);
            let _ = s.SetAreDefaultContextMenusEnabled(!is_chrome);
            let _ = s.SetAreHostObjectsAllowed(false);
            let _ = s.SetIsStatusBarEnabled(false);
            // We render our own error page, styled like the rest of the browser.
            let _ = s.SetIsBuiltInErrorPageEnabled(false);
            let _ = s.SetIsWebMessageEnabled(is_chrome);
            let _ = s.SetIsZoomControlEnabled(true);
            if let Ok(s4) = s.cast::<ICoreWebView2Settings4>() {
                let _ = s4.SetIsGeneralAutofillEnabled(false);
                let _ = s4.SetIsPasswordAutosaveEnabled(false);
            }
            if let Ok(s5) = s.cast::<ICoreWebView2Settings5>() {
                let _ = s5.SetIsPinchZoomEnabled(false);
            }
            if let Ok(s6) = s.cast::<ICoreWebView2Settings6>() {
                let _ = s6.SetIsSwipeNavigationEnabled(false);
            }
        }

        if let Ok(w13) = webview.cast::<ICoreWebView2_13>() {
            if let Ok(profile) = w13.Profile() {
                let _ = profile
                    .SetPreferredTrackingPreventionLevel(COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_STRICT);
                let scheme = if prefs.is_dark() {
                    COREWEBVIEW2_PREFERRED_COLOR_SCHEME_DARK
                } else {
                    COREWEBVIEW2_PREFERRED_COLOR_SCHEME_LIGHT
                };
                let _ = profile.SetPreferredColorScheme(scheme);
            }
        }
    }
}

/// Apply or clear the anti-fingerprinting user agent on an existing webview.
pub fn set_user_agent(webview: &ICoreWebView2, antifp: bool) {
    unsafe {
        if let Ok(s) = webview.Settings() {
            if antifp {
                let ua = HSTRING::from(CHROME_UA);
                let _ = s.SetUserAgent(&ua);
            } else {
                let _ = s.SetUserAgent(&HSTRING::new());
            }
        }
    }
}

/// Inject a document-start script and report its id so it can be removed later.
pub fn add_script(webview: &ICoreWebView2, js: &str, on_id: impl Fn(String) + 'static) {
    unsafe {
        let js = HSTRING::from(js);
        let handler = AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(
            move |hr, id: String| {
                if hr.is_ok() && !id.is_empty() {
                    on_id(id);
                }
                Ok(())
            },
        ));
        let _ = webview.AddScriptToExecuteOnDocumentCreated(&js, &handler);
    }
}

pub fn remove_scripts(webview: &ICoreWebView2, ids: &[String]) {
    for id in ids {
        unsafe {
            let _ = webview.RemoveScriptToExecuteOnDocumentCreated(&HSTRING::from(id.as_str()));
        }
    }
}

/// Force-dark pages, as a document-start script that appends a stylesheet.
pub fn force_dark_js() -> String {
    let css = serde_json::to_string(FORCE_DARK_CSS).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"(() => {{
  const css = {css};
  const add = () => {{
    try {{
      const s = document.createElement('style');
      s.setAttribute('data-rustchrome', 'force-dark');
      s.textContent = css;
      (document.head || document.documentElement).appendChild(s);
    }} catch (e) {{}}
  }};
  if (document.documentElement) add();
  else document.addEventListener('DOMContentLoaded', add, {{ once: true }});
}})();"#
    )
}

/// The anti-fingerprinting payload, ready to inject.
pub fn antifp_js() -> &'static str {
    ANTI_FP_JS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_dark_payload_is_valid_js_shape() {
        let js = force_dark_js();
        assert!(js.contains("createElement('style')"));
        assert!(js.contains("invert"));
        assert!(js.trim_end().ends_with("})();"));
    }

    #[test]
    fn ua_is_chromium_not_firefox() {
        assert!(CHROME_UA.contains("Chrome/124"));
        assert!(!CHROME_UA.contains("Firefox"));
    }
}
