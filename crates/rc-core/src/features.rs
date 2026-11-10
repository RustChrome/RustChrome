//! Everyday browser features, implemented once for both shells.
//!
//! Everything here is plain Rust with no GUI dependencies: bookmark import and
//! export, session recording (restore + crash detection), the recently-closed
//! list, custom search engines, per-site permission rules and the tab-search
//! matcher. The shells only do presentation.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::store::Bookmark;

// ---------------------------------------------------------------- bookmarks

fn html_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let (entity, len) = match rest.find(';') {
            Some(end) if end <= 10 => (&rest[1..end], end + 1),
            _ => {
                out.push('&');
                rest = &rest[1..];
                continue;
            }
        };
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ => None,
        };
        match decoded {
            Some(c) => out.push(c),
            None => {
                out.push('&');
                out.push_str(entity);
                out.push(';');
            }
        }
        rest = &rest[len..];
    }
    out.push_str(rest);
    out
}

/// Parse a Netscape bookmark file — the format both Chrome and Firefox export.
pub fn parse_netscape_html(html: &str) -> Vec<Bookmark> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(pos) = rest.find("<A ") {
        rest = &rest[pos + 3..];
        let Some(tag_end) = rest.find('>') else { break };
        let attrs = &rest[..tag_end];
        let body = &rest[tag_end + 1..];
        let Some(close) = body.find("</A>") else {
            break;
        };

        let href = attr_value(attrs, "HREF").or_else(|| attr_value(attrs, "href"));
        let tags = attr_value(attrs, "TAGS").unwrap_or_default();
        let title = html_unescape(body[..close].trim());
        if let Some(url) = href {
            if !url.is_empty() && !url.starts_with("javascript:") && !url.starts_with("place:") {
                // A single tag like "unsorted" marks the Firefox "Unsorted
                // Bookmarks" placeholder rather than a real folder.
                if !tags.eq_ignore_ascii_case("unsorted") {
                    out.push(Bookmark {
                        title: if title.is_empty() { url.clone() } else { title },
                        url,
                    });
                }
            }
        }
        rest = &body[close + 4..];
    }
    out
}

fn attr_value(attrs: &str, name: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let needle = name.to_ascii_lowercase();
    let idx = lower.find(&format!("{needle}="))?;
    let after = &attrs[idx + needle.len() + 1..];
    let (quote, rest) = match after.chars().next()? {
        c @ ('"' | '\'') => (c, &after[1..]),
        _ => return Some(after.split_whitespace().next()?.to_string()),
    };
    let end = rest.find(quote)?;
    Some(html_unescape(&rest[..end]))
}

/// Parse Chrome's own `Bookmarks` JSON file (the one inside a profile).
pub fn parse_chrome_json(json: &str) -> Vec<Bookmark> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(roots) = v.get("roots") {
        for key in ["bookmark_bar", "other", "synced"] {
            if let Some(node) = roots.get(key) {
                walk_chrome_node(node, &mut out, 0);
            }
        }
    }
    out
}

fn walk_chrome_node(node: &Value, out: &mut Vec<Bookmark>, depth: usize) {
    if depth > 32 {
        return;
    }
    if let Some(url) = node.get("url").and_then(|u| u.as_str()) {
        if !url.is_empty() && !url.starts_with("javascript:") {
            let title = node
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or(url)
                .to_string();
            out.push(Bookmark {
                title,
                url: url.to_string(),
            });
            return;
        }
    }
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for child in children {
            walk_chrome_node(child, out, depth + 1);
        }
    }
}

/// Where Chrome keeps its bookmarks, if it is installed.
pub fn chrome_bookmarks_path() -> Option<PathBuf> {
    let candidates = if cfg!(windows) {
        vec![
            std::env::var_os("LOCALAPPDATA")
                .map(|b| PathBuf::from(b).join("Google/Chrome/User Data/Default/Bookmarks")),
            std::env::var_os("LOCALAPPDATA")
                .map(|b| PathBuf::from(b).join("Google/Chrome Beta/User Data/Default/Bookmarks")),
            std::env::var_os("LOCALAPPDATA")
                .map(|b| PathBuf::from(b).join("Chromium/User Data/Default/Bookmarks")),
        ]
    } else {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        vec![
            home.as_ref()
                .map(|h| h.join(".config/google-chrome/Default/Bookmarks")),
            home.as_ref()
                .map(|h| h.join(".config/chromium/Default/Bookmarks")),
            home.as_ref()
                .map(|h| h.join(".config/google-chrome-beta/Default/Bookmarks")),
        ]
    };
    candidates.into_iter().flatten().find(|p| p.is_file())
}

/// Sniff the format instead of trusting the extension: exports are often saved
/// without one, and Chrome's file has no extension at all.
pub fn parse_bookmarks_file(path: &Path) -> Result<Vec<Bookmark>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let trimmed = text.trim_start();
    let items = if trimmed.starts_with('{') {
        parse_chrome_json(&text)
    } else if text.contains("<A ") || text.contains("<a ") {
        parse_netscape_html(&text)
    } else {
        return Err(format!(
            "{} is neither a Netscape bookmark file nor a Chrome Bookmarks JSON file",
            path.display()
        ));
    };
    if items.is_empty() {
        return Err(format!("no bookmarks found in {}", path.display()));
    }
    Ok(items)
}

/// Merge imported bookmarks into the existing list, skipping duplicates by URL.
/// Returns (added, skipped).
pub fn merge_bookmarks(existing: &mut Vec<Bookmark>, incoming: Vec<Bookmark>) -> (usize, usize) {
    let mut added = 0;
    let mut skipped = 0;
    for b in incoming {
        if existing.iter().any(|e| e.url == b.url) {
            skipped += 1;
        } else {
            existing.push(b);
            added += 1;
        }
    }
    (added, skipped)
}

pub fn export_netscape(items: &[Bookmark]) -> String {
    let mut s = String::from(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n\
         <!-- This is an automatically generated file. Do not edit. -->\n\
         <META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n\
         <TITLE>Bookmarks</TITLE>\n<H1>Bookmarks</H1>\n<DL><p>\n",
    );
    for b in items {
        s.push_str(&format!(
            "    <DT><A HREF=\"{}\">{}</A>\n",
            escape_attr(&b.url),
            escape_text(&b.title)
        ));
    }
    s.push_str("</DL><p>\n");
    s
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;")
}

fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ------------------------------------------------------------------ session

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct SessionState {
    pub version: u32,
    /// Set at startup and cleared only on a clean exit: a leftover `true` means
    /// the last run died, which is what makes crash recovery possible.
    pub dirty: bool,
    /// Open tabs, in order. Empty means "nothing to restore".
    pub tabs: Vec<String>,
    /// Recently closed URLs, newest last.
    pub closed: Vec<String>,
    /// Which tab was active.
    pub active: usize,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            version: 1,
            dirty: false,
            tabs: Vec::new(),
            closed: Vec::new(),
            active: 0,
        }
    }
}

pub const MAX_CLOSED: usize = 20;

impl SessionState {
    pub fn path() -> PathBuf {
        crate::data_dir().join("session.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let tmp = Self::path().with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, Self::path())
    }

    pub fn push_closed(&mut self, url: &str) {
        if url.is_empty() {
            return;
        }
        self.closed.push(url.to_string());
        while self.closed.len() > MAX_CLOSED {
            self.closed.remove(0);
        }
    }

    pub fn take_closed(&mut self) -> Option<String> {
        self.closed.pop()
    }
}

/// True when the previous run ended badly and there is something to restore.
pub fn crashed(prev: &SessionState) -> bool {
    prev.dirty && !prev.tabs.is_empty()
}

// --------------------------------------------------------------- tab search

/// Rank tabs for the Ctrl+Shift+A palette: title hits beat URL hits, and a
/// prefix match beats a scattered one. Empty query returns them in order.
pub fn match_tabs(query: &str, tabs: &[(String, String)]) -> Vec<usize> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return (0..tabs.len()).collect();
    }
    let mut scored: Vec<(i32, usize)> = Vec::new();
    for (i, (title, url)) in tabs.iter().enumerate() {
        let t = title.to_ascii_lowercase();
        let u = url.to_ascii_lowercase();
        let score = if t.starts_with(&q) {
            100
        } else if t.contains(&q) {
            80
        } else if u.starts_with(&q) {
            60
        } else if u.contains(&q) {
            40
        } else {
            continue;
        };
        scored.push((score, i));
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

// ------------------------------------------------------------- search engines

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CustomEngine {
    pub id: String,
    pub label: String,
    /// Template with `{q}` where the query goes.
    pub template: String,
}

/// Turn whatever the user typed into a usable template.
pub fn normalize_engine_template(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    if t.contains("{q}") {
        return Some(t.to_string());
    }
    // "https://example.com/search?q=" -> append the placeholder.
    if t.starts_with("http://") || t.starts_with("https://") {
        if t.ends_with('=') || t.ends_with('?') || t.ends_with('&') {
            return Some(format!("{t}{{q}}"));
        }
        return Some(format!("{t}{{}}{{q}}"));
    }
    None
}

pub fn resolve_template(id: &str, custom: &[CustomEngine]) -> String {
    if let Some(c) = custom.iter().find(|e| e.id == id) {
        return c.template.clone();
    }
    crate::search_template(id).to_string()
}

// ----------------------------------------------------------- per-site rules

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SiteRule {
    /// Host, matched against the site and its subdomains.
    pub host: String,
    /// "camera", "microphone", "geolocation", "notifications", "clipboard",
    /// "sensors", "autoplay", "downloads", "files", "midi", "windows",
    /// "unknown".
    pub kind: String,
    pub allow: bool,
}

pub fn sites_path() -> PathBuf {
    crate::data_dir().join("sites.json")
}

pub fn load_sites() -> Vec<SiteRule> {
    std::fs::read_to_string(sites_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<SiteRule>>(&s).ok())
        .unwrap_or_default()
}

pub fn save_sites(rules: &[SiteRule]) -> std::io::Result<()> {
    let tmp = sites_path().with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(rules)?)?;
    std::fs::rename(&tmp, sites_path())
}

/// Host of a URL, lowercased, without a port.
pub fn host_of(url: &str) -> Option<String> {
    let (_, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority
        .rsplit_once('@')
        .map(|(_, h)| h)
        .unwrap_or(authority);
    let host = host_port.split(':').next().unwrap_or("");
    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

fn host_matches(rule_host: &str, site: &str) -> bool {
    site == rule_host || site.ends_with(&format!(".{rule_host}"))
}

/// `Some(true)` allow, `Some(false)` deny, `None` no rule (caller decides).
pub fn site_decision(rules: &[SiteRule], url: &str, kind: &str) -> Option<bool> {
    let host = host_of(url)?;
    rules
        .iter()
        .filter(|r| r.kind == kind && host_matches(&r.host, &host))
        .max_by_key(|r| r.host.len())
        .map(|r| r.allow)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHROME_HTML: &str = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><A HREF="https://example.com/" ADD_DATE="1">Example &amp; Co</A>
    <DT><A HREF="https://rust-lang.org/">Rust</A>
    <DT><A HREF="javascript:alert(1)">bad</A>
</DL><p>"#;

    const FIREFOX_HTML: &str = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><H3>Menu</H3>
    <DL><p>
        <DT><A HREF="https://news.ycombinator.com/">Hacker News</A>
        <DT><A HREF="place:sort=8&amp;maxresults=10" TAGS="unsorted">Recent</A>
    </DL><p>
</DL><p>"#;

    #[test]
    fn parses_chrome_export() {
        let items = parse_netscape_html(CHROME_HTML);
        assert_eq!(items.len(), 2, "javascript: entries must be dropped");
        assert_eq!(items[0].url, "https://example.com/");
        assert_eq!(items[0].title, "Example & Co");
    }

    #[test]
    fn parses_firefox_export() {
        let items = parse_netscape_html(FIREFOX_HTML);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Hacker News");
        assert_eq!(items[0].url, "https://news.ycombinator.com/");
    }

    #[test]
    fn parses_chrome_profile_json() {
        let json = r#"{"roots":{"bookmark_bar":{"children":[
            {"type":"url","name":"A","url":"https://a.test/"},
            {"type":"folder","name":"F","children":[
                {"type":"url","name":"B","url":"https://b.test/"}]}]}}}"#;
        let items = parse_chrome_json(json);
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].url, "https://b.test/");
    }

    #[test]
    fn merge_skips_duplicates() {
        let mut existing = vec![Bookmark {
            title: "A".into(),
            url: "https://a.test/".into(),
        }];
        let incoming = vec![
            Bookmark {
                title: "A again".into(),
                url: "https://a.test/".into(),
            },
            Bookmark {
                title: "B".into(),
                url: "https://b.test/".into(),
            },
        ];
        let (added, skipped) = merge_bookmarks(&mut existing, incoming);
        assert_eq!((added, skipped), (1, 1));
        assert_eq!(existing.len(), 2);
    }

    #[test]
    fn export_roundtrips() {
        let items = vec![Bookmark {
            title: "Quote \" and &".into(),
            url: "https://x.test/?a=1&b=2".into(),
        }];
        let html = export_netscape(&items);
        let back = parse_netscape_html(&html);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].url, items[0].url);
        assert_eq!(back[0].title, items[0].title);
    }

    #[test]
    fn crash_detection() {
        let mut s = SessionState {
            dirty: true,
            tabs: vec!["https://a".into()],
            ..Default::default()
        };
        assert!(crashed(&s));
        s.dirty = false;
        assert!(!crashed(&s));
    }

    #[test]
    fn closed_list_is_capped() {
        let mut s = SessionState::default();
        for i in 0..(MAX_CLOSED + 5) {
            s.push_closed(&format!("https://x/{i}"));
        }
        assert_eq!(s.closed.len(), MAX_CLOSED);
        assert_eq!(
            s.take_closed().unwrap(),
            format!("https://x/{}", MAX_CLOSED + 4)
        );
    }

    #[test]
    fn tab_search_ranks_title_before_url() {
        let tabs = vec![
            ("Rust".to_string(), "https://rust-lang.org".to_string()),
            ("Other".to_string(), "https://example.com/rust".to_string()),
            ("Unrelated".to_string(), "https://nope.test".to_string()),
        ];
        let hits = match_tabs("rust", &tabs);
        assert_eq!(hits, vec![0, 1]);
        assert_eq!(match_tabs("", &tabs).len(), 3);
        assert!(match_tabs("zzz", &tabs).is_empty());
    }

    #[test]
    fn engine_templates() {
        assert_eq!(
            normalize_engine_template("https://s.test/?q={q}").unwrap(),
            "https://s.test/?q={q}"
        );
        assert_eq!(
            normalize_engine_template("https://s.test/search?q=").unwrap(),
            "https://s.test/search?q={q}"
        );
        assert!(normalize_engine_template("not a url").is_none());
    }

    #[test]
    fn site_rules_match_subdomains_and_most_specific_wins() {
        let rules = vec![
            SiteRule {
                host: "example.com".into(),
                kind: "camera".into(),
                allow: true,
            },
            SiteRule {
                host: "meet.example.com".into(),
                kind: "camera".into(),
                allow: false,
            },
        ];
        assert_eq!(
            site_decision(&rules, "https://example.com/x", "camera"),
            Some(true)
        );
        assert_eq!(
            site_decision(&rules, "https://a.meet.example.com/x", "camera"),
            Some(false)
        );
        assert_eq!(site_decision(&rules, "https://other.test", "camera"), None);
        assert_eq!(
            site_decision(&rules, "https://example.com", "microphone"),
            None
        );
    }

    #[test]
    fn host_parsing() {
        assert_eq!(
            host_of("https://user:pw@Example.COM:8443/a"),
            Some("example.com".into())
        );
        assert_eq!(host_of("about:blank"), None);
    }
}
