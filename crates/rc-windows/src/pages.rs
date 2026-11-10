//! Generated content pages: new tab, error, bookmarks and extensions.
//!
//! The new-tab page is a direct port of the GTK shell's `NEW_TAB_HTML`
//! (src/main.rs) including its dark-mode colour substitutions, so the two
//! builds open on the same page.

use rc_core::store::Bookmark;
use rc_core::{search_template, SEARCH_ENGINES};
use std::path::PathBuf;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The Google wordmark, letter by letter, exactly as the Linux build draws it.
fn wordmark() -> &'static str {
    r#"<div class="logo"><span style="color:#4285f4">G</span><span style="color:#ea4335">o</span><span style="color:#fbbc05">o</span><span style="color:#4285f4">g</span><span style="color:#34a853">l</span><span style="color:#ea4335">e</span></div>"#
}

pub fn new_tab_html(engine: &str, dark: bool) -> String {
    let template = search_template(engine);
    let label = SEARCH_ENGINES
        .iter()
        .find(|e| e.0 == engine)
        .map(|e| e.1)
        .unwrap_or("Search");

    // Google keeps its wordmark; any other engine gets its name in the same slot.
    let logo = if engine == "google" {
        wordmark().to_string()
    } else {
        format!(r#"<div class="logo">&#8203;{}</div>"#, esc(label))
    };
    let search_label = if engine == "google" {
        "Google Search".to_string()
    } else {
        format!("{} Search", esc(label))
    };

    let (bg, fg, field, field_border, btn_bg, btn_fg, foot) = if dark {
        ("#202124", "#e8eaed", "#202124", "#5f6368", "#303134", "#e8eaed", "#9aa0a6")
    } else {
        ("#ffffff", "#202124", "#ffffff", "#dfe1e5", "#f8f9fa", "#3c4043", "#70757a")
    };

    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>New Tab</title><style>
*{{box-sizing:border-box}}
body{{margin:0;height:100vh;display:flex;flex-direction:column;align-items:center;justify-content:center;
font-family:arial,sans-serif;background:{bg};color:{fg}}}
.logo{{font-size:72px;letter-spacing:-4px;font-weight:500;margin-bottom:26px;user-select:none}}
form{{width:min(584px,86vw)}}
input{{width:100%;height:46px;background:{field};color:{fg};border:1px solid {field_border};border-radius:24px;padding:0 20px;font-size:16px;outline:none}}
input:hover,input:focus{{box-shadow:0 1px 6px rgba(32,33,36,.28);border-color:rgba(223,225,229,0)}}
.btns{{margin-top:28px;display:flex;gap:12px;justify-content:center}}
button{{background:{btn_bg};border:1px solid {btn_bg};border-radius:4px;color:{btn_fg};font-size:14px;
padding:10px 18px;cursor:pointer}}
button:hover{{box-shadow:0 1px 1px rgba(0,0,0,.1);border-color:{field_border}}}
.foot{{position:fixed;bottom:20px;font-size:12px;color:{foot}}}
a{{color:#1a73e8;text-decoration:none}}
</style></head><body>
{logo}
<form id="f"><input id="q" autofocus placeholder="Search Google or type a URL" autocomplete="off"></form>
<div class="btns"><button id="sb">{search_label}</button><button id="lucky">I'm Feeling Lucky</button></div>
<div class="foot">RustChrome v{version} &middot; <a href="https://github.com/RustChrome/RustChrome">github</a></div>
<script>
const TEMPLATE = {template_json};
const ROOT = {root_json};
const q = document.getElementById('q');
function go(to){{ location.href = to; }}
document.getElementById('f').addEventListener('submit', (e) => {{
  e.preventDefault();
  const v = q.value.trim();
  if (!v) return;
  go(TEMPLATE.replace('{{q}}', encodeURIComponent(v)));
}});
document.getElementById('sb').addEventListener('click', () => {{
  const v = q.value.trim();
  if (!v) return;
  go(TEMPLATE.replace('{{q}}', encodeURIComponent(v)));
}});
document.getElementById('lucky').addEventListener('click', () => {{
  const v = q.value.trim();
  go(v ? ROOT + '/search?q=' + encodeURIComponent(v) : ROOT);
}});
</script>
</body></html>"#,
        version = env!("CARGO_PKG_VERSION"),
        template_json = serde_json::to_string(template).unwrap_or_else(|_| "\"\"".into()),
        root_json = serde_json::to_string(
            template.split('/').take(3).collect::<Vec<_>>().join("/").as_str()
        )
        .unwrap_or_else(|_| "\"\"".into()),
    )
}

pub fn error_html(url: &str, message: &str, dark: bool) -> String {
    let (bg, fg, sub, code_bg) = if dark {
        ("#202124", "#e8eaed", "#9aa0a6", "#303134")
    } else {
        ("#ffffff", "#202124", "#5f6368", "#f1f3f4")
    };
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>Can't be reached</title>
<style>
body{{margin:0;height:100vh;display:flex;align-items:center;justify-content:center;
background:{bg};color:{fg};font-family:arial,sans-serif}}
div{{max-width:560px;padding:0 24px}}h2{{font-weight:400;margin:0 0 16px;font-size:22px}}
p{{color:{sub};line-height:1.6;font-size:14px}}
code{{background:{code_bg};padding:3px 6px;border-radius:4px;font-size:13px;word-break:break-all}}
</style></head><body><div>
<h2>This site can&rsquo;t be reached</h2>
<p><b>{url}</b> could not be loaded.</p>
<p><code>{url}</code></p>
<p>{message}</p>
<p>Check the address, or your network connection, then try again.</p>
</div></body></html>"#,
        url = esc(url),
        message = esc(message),
    )
}

/// Mirrors the GTK shell's `bookmarks_html` page.
pub fn bookmarks_html(items: &[Bookmark]) -> String {
    let mut rows = String::new();
    for b in items {
        rows.push_str(&format!(
            "<tr><td><a href=\"{url}\">{title}</a></td><td class=u>{url}</td></tr>",
            url = esc(&b.url),
            title = esc(&b.title),
        ));
    }
    if items.is_empty() {
        rows.push_str(
            "<tr><td colspan=2 class=empty>No bookmarks yet &mdash; hit the star in the toolbar.</td></tr>",
        );
    }
    format!(
        r#"<!doctype html><html><head><meta charset='utf-8'><title>Bookmarks</title><style>
body{{font-family:system-ui,sans-serif;margin:40px;max-width:880px;background:#fff;color:#202124}}
h1{{font-weight:400;font-size:24px;margin:0 0 20px}}table{{border-collapse:collapse;width:100%;font-size:14px}}
td,th{{text-align:left;padding:10px 8px;border-bottom:1px solid #e8eaed}}
a{{color:#1a73e8;text-decoration:none}}a:hover{{text-decoration:underline}}
.u{{color:#5f6368;font-size:12px;font-family:monospace}}.empty{{color:#80868b}}
</style></head><body><h1>Bookmarks</h1>
<table><tr><th>Name</th><th>URL</th></tr>{rows}</table></body></html>"#
    )
}

/// Mirrors the GTK shell's `extensions_html` page.
pub fn extensions_html(files: &[PathBuf], dir: &str) -> String {
    let mut rows = String::new();
    for f in files {
        rows.push_str(&format!(
            "<tr><td class=n>{}</td><td class=u>{}</td></tr>",
            esc(&f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
            esc(&f.display().to_string()),
        ));
    }
    if files.is_empty() {
        rows.push_str("<tr><td colspan=2 class=empty>No userscripts installed yet.</td></tr>");
    }
    format!(
        r#"<!doctype html><html><head><meta charset='utf-8'><title>Extensions</title><style>
body{{font-family:system-ui,sans-serif;margin:40px;max-width:820px;background:#fff;color:#202124}}
h1{{font-weight:400;font-size:24px;margin:0 0 4px}}p.sub{{color:#5f6368;margin:0 0 24px;font-size:14px}}
table{{border-collapse:collapse;width:100%;font-size:14px}}
td,th{{text-align:left;padding:10px 8px;border-bottom:1px solid #e8eaed}}
.u{{color:#5f6368;font-family:monospace;font-size:12px}}
.empty{{color:#80868b}}code{{background:#f1f3f4;padding:2px 5px;border-radius:4px}}
.box{{border:1px solid #e8eaed;border-radius:10px;padding:16px 18px;margin-top:26px;font-size:14px;color:#3c4043}}
</style></head><body><h1>Extensions</h1>
<p class=sub>RustChrome runs userscripts, not Chrome Web Store packages &mdash; WebView2 has no Chrome extension API.</p>
<table><tr><th>Script</th><th>Path</th></tr>{rows}</table>
<div class=box><b>Add one:</b> drop a <code>.js</code> file into<br><code>{dir}</code>, then use Settings &rarr; Userscripts &rarr; Reload.</div></body></html>"#,
        dir = esc(dir),
    )
}
