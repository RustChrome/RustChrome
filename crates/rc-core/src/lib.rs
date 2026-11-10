//! Platform-neutral core for RustChrome.
//!
//! Everything in here is GUI-free and dependency-light so that the GTK shell and
//! the Win32/WebView2 shell can share the same behaviour: omnibox rules, the
//! on-disk stores, the data directory policy and the privacy payloads.

pub mod dashboard;
pub mod features;
pub mod privacy;
pub mod store;

use std::path::PathBuf;

/// Search engines offered in Settings. `(id, label, template)`, `{q}` is the
/// already percent-encoded query.
pub const SEARCH_ENGINES: &[(&str, &str, &str)] = &[
    ("ddg", "DuckDuckGo", "https://duckduckgo.com/?q={q}"),
    ("google", "Google", "https://www.google.com/search?q={q}"),
    ("bing", "Bing", "https://www.bing.com/search?q={q}"),
    (
        "brave",
        "Brave Search",
        "https://search.brave.com/search?q={q}",
    ),
    (
        "startpage",
        "Startpage",
        "https://www.startpage.com/sp/search?query={q}",
    ),
];

pub fn search_template(id: &str) -> &'static str {
    SEARCH_ENGINES
        .iter()
        .find(|e| e.0 == id)
        .map(|e| e.2)
        .unwrap_or(SEARCH_ENGINES[0].2)
}

/// Percent-encode a query. Spaces become `%20` (not `+`) because `+` is
/// interpreted literally by several engines in the path component.
pub fn encode_query(q: &str) -> String {
    let mut out = String::with_capacity(q.len());
    for b in q.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

pub fn search_url(engine: &str, query: &str) -> String {
    search_template(engine).replace("{q}", &encode_query(query))
}

/// Omnibox rules, matching the Linux shell:
/// * an explicit allow-listed scheme is used as-is,
/// * anything else that carries a scheme is searched for instead of executed
///   (this is what stops `javascript:` and `ms-settings:` from being typed in),
/// * something that looks like a host becomes `https://<host>`,
/// * everything else becomes a search.
pub fn normalize(input: &str, engine: &str) -> String {
    let t = input.trim();
    if t.is_empty() {
        return String::new();
    }
    let lower = t.to_ascii_lowercase();
    for scheme in ["https://", "http://", "file://", "about:"] {
        if lower.starts_with(scheme) {
            return t.to_string();
        }
    }
    // Host first: "localhost:8080" and "example.com:8080" look like schemes.
    let looks_like_host = !t.contains(' ')
        && !t.starts_with('-')
        && (t.contains('.') || lower == "localhost" || lower.starts_with("localhost:"));
    if looks_like_host {
        return format!("https://{}", t);
    }
    // Anything else that still carries a scheme is searched for, not executed:
    // this is what stops javascript:, ms-settings:, data: and friends.
    if let Some(idx) = t.find(':') {
        let (head, _) = t.split_at(idx);
        let looks_like_scheme = !head.is_empty()
            && !head.contains('/')
            && head
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
        if looks_like_scheme {
            return search_url(engine, t);
        }
    }
    search_url(engine, t)
}

/// Scheme allow-list for everything a page or the shell can navigate to.
///
/// The empty string is allowed on purpose: WebView2 reports an empty URI for
/// its own `NavigateToString` navigations (the new-tab and error pages), and
/// cancelling those would cancel our own internal pages.
pub fn is_allowed_navigation(url: &str) -> bool {
    if url.is_empty() {
        return true;
    }
    let l = url.to_ascii_lowercase();
    // Internal pages (new tab, error page) are rendered with
    // `ICoreWebView2::NavigateToString`, which presents itself as a
    // `data:text/html` navigation. Chromium already refuses page-initiated
    // top-level data: navigations, so this only admits our own pages.
    if l.starts_with("data:text/html") {
        return true;
    }
    l.starts_with("https://")
        || l.starts_with("http://")
        || l.starts_with("file://")
        || l.starts_with("about:")
        || l.starts_with("blob:")
}

/// Directory for RustChrome's own files (prefs, bookmarks, userscripts).
///
/// Windows: `%LOCALAPPDATA%\RustChrome`
/// Unix:    `$XDG_DATA_HOME/rustchrome` (or `~/.local/share/rustchrome`)
pub fn data_dir() -> PathBuf {
    let dir = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("RustChrome")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
            })
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustchrome")
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Best-effort downloads directory. The Windows shell prefers the real
/// KnownFolder; this is the fallback and what the core's tests use.
pub fn downloads_dir() -> PathBuf {
    let dir = if cfg!(windows) {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Downloads")
    } else {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Downloads"))
            .unwrap_or_else(|| PathBuf::from("."))
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_becomes_https() {
        assert_eq!(normalize("example.com", "ddg"), "https://example.com");
        assert_eq!(normalize("localhost:8080", "ddg"), "https://localhost:8080");
    }

    #[test]
    fn explicit_scheme_is_kept() {
        assert_eq!(normalize("https://a.test/x", "ddg"), "https://a.test/x");
        assert_eq!(normalize("file:///tmp/a.html", "ddg"), "file:///tmp/a.html");
    }

    #[test]
    fn javascript_scheme_is_searched_not_executed() {
        let out = normalize("javascript:alert(1)", "ddg");
        assert!(out.starts_with("https://duckduckgo.com/?q="), "got {out}");
        assert!(!out.contains("javascript:alert"));
    }

    #[test]
    fn phrase_becomes_search() {
        let out = normalize("rust webview2", "google");
        assert!(out.starts_with("https://www.google.com/search?q="));
        assert!(out.ends_with("rust%20webview2"));
    }

    #[test]
    fn query_is_encoded() {
        assert_eq!(encode_query("a b&c=d"), "a%20b%26c%3Dd");
    }

    #[test]
    fn empty_input_is_empty() {
        assert!(normalize("   ", "ddg").is_empty());
    }

    #[test]
    fn localhost_with_port_stays_a_host() {
        assert_eq!(normalize("localhost:8080", "ddg"), "https://localhost:8080");
        assert_eq!(
            normalize("example.com:8443/x", "ddg"),
            "https://example.com:8443/x"
        );
    }

    #[test]
    fn internal_pages_with_no_uri_are_allowed() {
        assert!(is_allowed_navigation(""));
        assert!(is_allowed_navigation("about:blank"));
        // NavigateToString lands here; blocking it would break every
        // internal page.
        assert!(is_allowed_navigation(
            "data:text/html;charset=utf-8,<h1>x</h1>"
        ));
    }

    #[test]
    fn scheme_gate_blocks_dangerous_targets() {
        assert!(is_allowed_navigation("https://example.com"));
        assert!(is_allowed_navigation("about:blank"));
        assert!(!is_allowed_navigation("ms-settings:privacy"));
        assert!(!is_allowed_navigation("javascript:alert(1)"));
        assert!(!is_allowed_navigation("data:text/plain,hello"));
    }

    #[test]
    fn unknown_engine_falls_back() {
        assert_eq!(search_template("nope"), SEARCH_ENGINES[0].2);
    }
}
