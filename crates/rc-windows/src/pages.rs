//! Generated content pages: the new-tab page and the navigation error page.

use rc_core::{search_template, SEARCH_ENGINES};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// New tab page. The search box posts straight to the configured engine, so no
/// bridge is needed for the most common action on the page.
pub fn new_tab_html(engine: &str, dark: bool) -> String {
    let template = search_template(engine);
    let label = SEARCH_ENGINES
        .iter()
        .find(|e| e.0 == engine)
        .map(|e| e.1)
        .unwrap_or("Search");
    let (bg, fg, field, field_border, sub) = if dark {
        ("#202124", "#e8eaed", "#303134", "#5f6368", "#9aa0a6")
    } else {
        ("#ffffff", "#202124", "#ffffff", "#dfe1e5", "#70757a")
    };
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>New Tab</title>
<style>
*{{box-sizing:border-box}}
body{{margin:0;height:100vh;display:flex;flex-direction:column;align-items:center;justify-content:center;
background:{bg};color:{fg};font-family:'Segoe UI',Arial,sans-serif}}
.logo{{font-size:60px;font-weight:600;letter-spacing:-2px;margin-bottom:24px;user-select:none}}
form{{width:min(584px,86vw)}}
input{{width:100%;height:46px;background:{field};color:{fg};border:1px solid {field_border};border-radius:23px;
padding:0 20px;font-size:16px;outline:none}}
input:focus{{border-color:#1a73e8;box-shadow:0 1px 6px rgba(32,33,36,.28)}}
.sub{{margin-top:18px;font-size:12px;color:{sub}}}
</style></head><body>
<div class="logo">RustChrome</div>
<form id="f"><input id="q" autofocus autocomplete="off" placeholder="Search {label} or type a URL"></form>
<div class="sub">Private by default &middot; Ctrl+L for the address bar, Ctrl+T for a new tab</div>
<script>
const TEMPLATE = {template_json};
const q = document.getElementById('q');
document.getElementById('f').addEventListener('submit', (e) => {{
  e.preventDefault();
  const v = q.value.trim();
  if (!v) return;
  location.href = TEMPLATE.replace('{{q}}', encodeURIComponent(v));
}});
</script>
</body></html>"#,
        template_json = serde_json::to_string(template).unwrap_or_else(|_| "\"\"".into()),
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
background:{bg};color:{fg};font-family:'Segoe UI',Arial,sans-serif}}
div{{max-width:560px;padding:0 24px}}h2{{font-weight:400;margin:0 0 16px;font-size:22px}}
p{{color:{sub};line-height:1.6;font-size:14px}}
code{{background:{code_bg};padding:3px 6px;border-radius:4px;font-size:13px;word-break:break-all}}
</style></head><body><div>
<h2>This site can&rsquo;t be reached</h2>
<p><b>{host}</b> could not be loaded.</p>
<p><code>{url}</code></p>
<p>{message}</p>
<p>Check the address, or your network connection, then try again.</p>
</div></body></html>"#,
        host = esc(url),
        url = esc(url),
        message = esc(message),
    )
}
