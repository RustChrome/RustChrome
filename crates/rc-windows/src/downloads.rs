//! Download handling.
//!
//! Safety rules: the destination is always inside the Downloads folder, the
//! file name is sanitised against traversal and Windows reserved names, and an
//! existing file is never overwritten — a free `name (n).ext` is chosen instead.

use std::path::{Path, PathBuf};

use rc_core::store::sanitize_filename;
use webview2_com::Microsoft::Web::WebView2::Win32::*;


#[derive(Clone, Debug)]
pub struct Item {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub state: String,
    pub received: i64,
    pub total: i64,
}

pub fn downloads_dir() -> PathBuf {
    let d = rc_core::downloads_dir();
    let _ = std::fs::create_dir_all(&d);
    d
}

/// A path inside `dir` that does not collide with an existing file.
pub fn unique_path(dir: &Path, suggested: &str) -> PathBuf {
    let safe = sanitize_filename(suggested);
    let candidate = dir.join(&safe);
    if !candidate.exists() {
        return candidate;
    }
    let p = Path::new(&safe);
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());
    let ext = p
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    for n in 1..10_000 {
        let c = dir.join(format!("{stem} ({n}){ext}"));
        if !c.exists() {
            return c;
        }
    }
    dir.join(format!("{stem}-{}{ext}", std::process::id()))
}

/// Best-effort file name from a download's URL.
pub fn suggested_name(args: &ICoreWebView2DownloadStartingEventArgs, uri: &str) -> String {
    // Content-Disposition carries the real name when there is one.
    if let Some(op) = unsafe { args.DownloadOperation().ok() } {
        if let Ok(cd) = unsafe { op.ContentDisposition() } {
            let v = unsafe { crate::webview::take(cd) };
            if let Some(name) = v.split(';').find_map(|part| {
                let part = part.trim();
                let lower = part.to_ascii_lowercase();
                lower
                    .strip_prefix("filename=")
                    .map(|_| part["filename=".len()..].trim_matches('"').to_string())
            }) {
                if !name.is_empty() {
                    return sanitize_filename(&name);
                }
            }
        }
    }
    let path = uri.split(['?', '#']).next().unwrap_or(uri);
    let last = path.rsplit('/').next().unwrap_or("");
    let decoded = percent_decode(last);
    if decoded.is_empty() || decoded == "/" {
        "download".to_string()
    } else {
        sanitize_filename(&decoded)
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("");
            if let Ok(v) = u8::from_str_radix(hex, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

pub fn map_state(state: COREWEBVIEW2_DOWNLOAD_STATE) -> &'static str {
    match state {
        COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS => "progressing",
        COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED => "interrupted",
        COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED => "completed",
        _ => "unknown",
    }
}

/// Reveal a downloaded file: opens its containing folder in Explorer.
pub fn reveal(path: &str) {
    if let Some(dir) = Path::new(path).parent() {
        open_dir(dir);
    }
}

/// Open a folder in Explorer. Uses `explorer.exe` rather than ShellExecute so
/// there is no COM/OLE dependency and the failure mode is a silent no-op.
pub fn open_dir(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let _ = std::process::Command::new("explorer.exe").arg(dir).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_path_avoids_collisions() {
        let dir = std::env::temp_dir().join(format!("rc-dl-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let first = unique_path(&dir, "file.zip");
        assert_eq!(first.file_name().unwrap(), "file.zip");
        std::fs::write(&first, b"x").unwrap();
        let second = unique_path(&dir, "file.zip");
        assert_eq!(second.file_name().unwrap(), "file (1).zip");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn traversal_is_neutralised() {
        let dir = std::env::temp_dir();
        let p = unique_path(&dir, "../../evil.exe");
        assert_eq!(p.parent().unwrap(), dir);
        assert!(!p.to_string_lossy().contains(".."));
    }

    #[test]
    fn percent_decoding() {
        assert_eq!(percent_decode("a%20b.zip"), "a b.zip");
        assert_eq!(percent_decode("plain.txt"), "plain.txt");
    }
}
