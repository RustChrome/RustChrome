//! On-disk stores: preferences, bookmarks and userscripts.
//!
//! Every write is atomic (temp file + rename) and every load falls back to
//! defaults instead of failing, so a corrupt JSON file can never stop the
//! browser from starting. This also fixes the Linux shell's old habit of
//! rewriting `prefs.json` with only the keys it happened to know about.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{data_dir, SEARCH_ENGINES};

fn write_atomic(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

// ---------------------------------------------------------------- preferences

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Prefs {
    /// "light" or "dark"
    pub theme: String,
    /// Invert page colours with a filter (opt-in, off by default).
    pub force_dark: bool,
    /// Pin the fingerprint: UA override, WebGL/WebRTC shims, injected script.
    pub antifp: bool,
    /// Strict mode: WebGL contexts denied and WebRTC removed entirely, at the
    /// cost of breaking some sites. Off = coerce instead of remove.
    pub strict: bool,
    /// Keep cookies/cache on disk between launches. Off = InPrivate tabs
    /// (Private Session), on = Standard Session.
    pub keep_session: bool,
    /// Reopen the tabs from the previous run when it ended in a crash.
    pub restore_session: bool,
    /// User-defined search engines, in addition to the built-in list.
    pub engines: Vec<crate::features::CustomEngine>,
    pub show_bookmarks: bool,
    /// Search engine id from `SEARCH_ENGINES`.
    pub engine: String,
    pub find_case: bool,
    pub zoom: f64,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: "light".to_string(),
            force_dark: false,
            antifp: true,
            strict: false,
            keep_session: false,
            restore_session: true,
            engines: Vec::new(),
            show_bookmarks: true,
            // Same default as the GTK build (HOME + search are Google).
            engine: "google".to_string(),
            find_case: false,
            zoom: 1.0,
        }
    }
}

impl Prefs {
    pub fn path() -> PathBuf {
        data_dir().join("prefs.json")
    }

    pub fn load() -> Self {
        let mut p: Prefs = std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if !SEARCH_ENGINES.iter().any(|e| e.0 == p.engine) {
            p.engine = SEARCH_ENGINES[0].0.to_string();
        }
        if p.theme != "dark" {
            p.theme = "light".to_string();
        }
        p.zoom = p.zoom.clamp(0.25, 5.0);
        p
    }

    pub fn save(&self) -> std::io::Result<()> {
        let s = serde_json::to_string_pretty(self)?;
        write_atomic(&Self::path(), s.as_bytes())
    }

    /// Resolve a search template, honouring custom engines.
    pub fn template_for(&self, id: &str) -> String {
        crate::features::resolve_template(id, &self.engines)
    }

    pub fn is_dark(&self) -> bool {
        self.theme == "dark"
    }

    /// Apply one setting coming from the UI. Unknown keys are ignored.
    pub fn set_json(&mut self, key: &str, value: &serde_json::Value) -> bool {
        let mut changed = true;
        match key {
            "theme" => {
                self.theme = if value.as_str() == Some("dark") { "dark" } else { "light" }.to_string()
            }
            "force_dark" => self.force_dark = value.as_bool().unwrap_or(self.force_dark),
            "antifp" => self.antifp = value.as_bool().unwrap_or(self.antifp),
            "strict" => self.strict = value.as_bool().unwrap_or(self.strict),
            "keep_session" => self.keep_session = value.as_bool().unwrap_or(self.keep_session),
            "restore_session" => {
                self.restore_session = value.as_bool().unwrap_or(self.restore_session)
            }
            "show_bookmarks" => {
                self.show_bookmarks = value.as_bool().unwrap_or(self.show_bookmarks)
            }
            "find_case" => self.find_case = value.as_bool().unwrap_or(self.find_case),
            "engine" => {
                if let Some(id) = value.as_str() {
                    if SEARCH_ENGINES.iter().any(|e| e.0 == id) {
                        self.engine = id.to_string();
                    }
                }
            }
            "zoom" => self.zoom = value.as_f64().unwrap_or(self.zoom).clamp(0.25, 5.0),
            _ => changed = false,
        }
        changed
    }
}

// ------------------------------------------------------------------ bookmarks

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
}

pub fn bookmarks_path() -> PathBuf {
    data_dir().join("bookmarks.json")
}

pub fn load_bookmarks() -> Vec<Bookmark> {
    std::fs::read_to_string(bookmarks_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<Bookmark>>(&s).ok())
        .unwrap_or_default()
}

pub fn save_bookmarks(items: &[Bookmark]) -> std::io::Result<()> {
    let s = serde_json::to_string_pretty(items)?;
    write_atomic(&bookmarks_path(), s.as_bytes())
}

// ---------------------------------------------------------------- userscripts

#[derive(Clone, Debug)]
pub struct Userscript {
    pub name: String,
    pub code: String,
}

pub fn userscript_dir() -> PathBuf {
    let d = data_dir().join("userscripts");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Paths of every installed userscript, sorted.
pub fn userscript_files() -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(userscript_dir()) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "js").unwrap_or(false))
        .collect();
    paths.sort();
    paths
}

/// Read every `.js` file in the userscript folder, sorted by name for a stable
/// injection order.
pub fn load_userscripts() -> Vec<Userscript> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(userscript_dir()) else {
        return out;
    };
    let mut paths: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "js").unwrap_or(false))
        .collect();
    paths.sort();
    for p in paths {
        if let (Ok(code), Some(name)) = (
            std::fs::read_to_string(&p),
            p.file_stem().map(|s| s.to_string_lossy().to_string()),
        ) {
            out.push(Userscript { name, code });
        }
    }
    out
}

/// Make an arbitrary string safe as a file name on Windows and Unix.
pub fn sanitize_filename(raw: &str) -> String {
    let mut s: String = raw
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    s = s.trim_matches(|c: char| c == '.' || c == ' ').to_string();
    if s.is_empty() {
        s = "download".to_string();
    }
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        s = format!("_{s}");
    }
    if s.chars().count() > 120 {
        s = s.chars().take(120).collect();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefs_roundtrip_keeps_unrelated_keys() {
        // The old shell reset anti-fingerprinting whenever the theme changed.
        let mut p = Prefs::default();
        p.antifp = false;
        p.engine = "google".into();
        p.set_json("theme", &serde_json::json!("dark"));
        assert_eq!(p.theme, "dark");
        assert!(!p.antifp, "toggling the theme must not reset anti-fingerprinting");
        assert_eq!(p.engine, "google");
    }

    #[test]
    fn prefs_reject_unknown_engine_and_theme() {
        let mut p = Prefs::default();
        p.set_json("engine", &serde_json::json!("not-a-thing"));
        assert_ne!(p.engine, "not-a-thing");
        p.set_json("theme", &serde_json::json!("chartreuse"));
        assert_eq!(p.theme, "light");
    }

    #[test]
    fn sanitize_blocks_traversal_and_reserved_names() {
        assert_eq!(sanitize_filename("../../evil.exe"), "_.._evil.exe");
        assert_eq!(sanitize_filename("a/b\\c.zip"), "a_b_c.zip");
        assert_eq!(sanitize_filename("CON.exe"), "_CON.exe");
        assert_eq!(sanitize_filename("   "), "download");
        assert_eq!(sanitize_filename("report.pdf"), "report.pdf");
    }

    #[test]
    fn bookmarks_serialize() {
        let b = vec![Bookmark { title: "A".into(), url: "https://a.test".into() }];
        let s = serde_json::to_string(&b).unwrap();
        let back: Vec<Bookmark> = serde_json::from_str(&s).unwrap();
        assert_eq!(back, b);
    }
}
