//! The Privacy Dashboard: a plain page that states what is actually on.
//!
//! Everything here is filled in by the shell from real state. Nothing is
//! advertised that the engine does not do, and anything unavailable is shown as
//! unavailable rather than silently dropped.

pub struct Row {
    pub name: String,
    pub state: String,
    pub note: String,
}

pub struct Dashboard {
    pub engine: String,
    pub session_mode: String,
    pub settings: Vec<(String, String)>,
    pub protections: Vec<Row>,
    pub permissions: Vec<String>,
    pub limitations: Vec<String>,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn state_class(state: &str) -> &'static str {
    match state {
        "on" => "ok",
        "partial" => "warn",
        _ => "off",
    }
}

pub fn dashboard_html(d: &Dashboard) -> String {
    let mut settings = String::new();
    for (k, v) in &d.settings {
        settings.push_str(&format!(
            "<tr><td>{}</td><td class=v>{}</td></tr>",
            esc(k),
            esc(v)
        ));
    }

    let mut prot = String::new();
    for r in &d.protections {
        prot.push_str(&format!(
            "<tr><td>{}</td><td><span class=\"pill {}\">{}</span></td><td class=n>{}</td></tr>",
            esc(&r.name),
            state_class(&r.state),
            esc(&r.state),
            esc(&r.note)
        ));
    }

    let mut perms = String::new();
    for p in &d.permissions {
        perms.push_str(&format!("<li>{}</li>", esc(p)));
    }

    let mut lim = String::new();
    for l in &d.limitations {
        lim.push_str(&format!("<li>{}</li>", esc(l)));
    }

    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>Privacy</title><style>
:root{{--bg:#fff;--fg:#202124;--dim:#5f6368;--line:#e8eaed;--ok:#188038;--warn:#b06000;--off:#5f6368;--code:#f1f3f4}}
@media (prefers-color-scheme: dark){{:root{{--bg:#202124;--fg:#e8eaed;--dim:#9aa0a6;--line:#3c4043;--ok:#81c995;--warn:#fdd663;--off:#9aa0a6;--code:#303134}}}}
body{{margin:0;padding:32px 40px;background:var(--bg);color:var(--fg);font-family:system-ui,-apple-system,"Segoe UI",sans-serif;font-size:14px;line-height:1.55}}
h1{{font-weight:500;font-size:24px;margin:0 0 4px}}
.sub{{color:var(--dim);margin:0 0 28px}}
h2{{font-weight:600;font-size:15px;margin:30px 0 10px;text-transform:none}}
table{{border-collapse:collapse;width:100%;max-width:900px}}
td,th{{text-align:left;padding:8px 10px;border-bottom:1px solid var(--line);vertical-align:top}}
th{{color:var(--dim);font-weight:500;font-size:12px;text-transform:uppercase;letter-spacing:.04em}}
td.v{{color:var(--dim);font-family:ui-monospace,Consolas,monospace;font-size:12px}}
td.n{{color:var(--dim);font-size:13px}}
.pill{{display:inline-block;padding:1px 9px;border-radius:999px;font-size:12px;font-weight:600}}
.pill.ok{{background:color-mix(in srgb,var(--ok) 18%,transparent);color:var(--ok)}}
.pill.warn{{background:color-mix(in srgb,var(--warn) 20%,transparent);color:var(--warn)}}
.pill.off{{background:var(--code);color:var(--off)}}
.box{{max-width:900px;background:var(--code);border-radius:10px;padding:14px 18px;margin:10px 0}}
ul{{margin:6px 0 0;padding-left:20px;max-width:900px}}
li{{margin:3px 0}}
code{{background:var(--code);padding:1px 5px;border-radius:4px;font-size:12px}}
.foot{{margin-top:34px;color:var(--dim);font-size:12px;max-width:900px}}
</style></head><body>
<h1>Privacy</h1>
<p class="sub">Engine: {engine} &middot; Session mode: {mode}</p>

<h2>Settings</h2>
<table>{settings}</table>

<h2>Protections</h2>
<table>
<tr><th>Protection</th><th>State</th><th>How it is enforced</th></tr>
{prot}
</table>

<h2>Permissions</h2>
<div class="box">Every permission request is denied before the page sees a prompt:</div>
<ul>{perms}</ul>

<h2>Limitations</h2>
<ul>{lim}</ul>

<div class="foot">
RustChrome does not claim anonymity or Tor-equivalence. These are engine-level
settings plus a small scripted hardening layer; a page that is allowed to run
JavaScript can still infer things about the machine. Tracker counters are not
shown because neither WebKitGTK nor WebView2 exposes a blocked-request count.
</div>
</body></html>"#,
        engine = esc(&d.engine),
        mode = esc(&d.session_mode),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Dashboard {
        Dashboard {
            engine: "WebView2 131".into(),
            session_mode: "in-private".into(),
            settings: vec![("Anti-fingerprinting".into(), "on".into())],
            protections: vec![Row {
                name: "Canvas".into(),
                state: "partial".into(),
                note: "per-session noise".into(),
            }],
            permissions: vec!["Microphone".into()],
            limitations: vec!["No Tor".into()],
        }
    }

    #[test]
    fn renders_every_section() {
        let html = dashboard_html(&sample());
        for section in ["Settings", "Protections", "Permissions", "Limitations"] {
            assert!(html.contains(section), "missing {section}");
        }
        assert!(html.contains("WebView2 131"));
        assert!(html.contains("pill warn"));
    }

    #[test]
    fn escapes_user_visible_text() {
        let mut d = sample();
        d.limitations = vec!["<script>alert(1)</script>".into()];
        let html = dashboard_html(&d);
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
