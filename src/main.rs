// RustChrome v0.2 — Chrome-look browser shell in Rust (GTK3 + WebKitGTK).
// Tabs, Google omnibox, bookmarks bar, find-in-page, zoom, downloads.

use gtk::gdk::prelude::*;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Entry, EventBox, Image,
    Label, Orientation, ProgressBar, SearchEntry, Separator, Stack, StackTransitionType,
};
use rc_core::privacy::{antifp_js, SessionSeed};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use webkit2gtk::{
    CookieAcceptPolicy, CookieManagerExt, DownloadExt, FindControllerExt, LoadEvent,
    NavigationPolicyDecision, NavigationPolicyDecisionExt, PolicyDecisionExt, PolicyDecisionType,
    SettingsExt, URIRequestExt, UserContentInjectedFrames, UserContentManager,
    UserContentManagerExt, UserStyleLevel, UserStyleSheet, WebContext, WebContextExt, WebView,
    WebViewExt, WebsiteDataManagerExt,
};

const HOME: &str = "https://www.google.com/";
const NEW_TAB_PAGE: &str = "about:rustchrome-newtab";
const EXT_PAGE: &str = "about:rustchrome-extensions";

const BOOKMARKS_PAGE: &str = "about:rustchrome-bookmarks";
const PRIVACY_PAGE: &str = "about:rustchrome-privacy";

/// RUSTCHROME_PROBE=<file>: load the probe page, dump its report, exit.
/// Reported once, on the first finished load, so the QA workflow can measure
/// startup without guessing.
static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
static STARTUP_LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn log_startup_once() {
    use std::io::Write;
    if STARTUP_LOGGED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let (Some(start), Ok(path)) = (START.get(), std::env::var("RUSTCHROME_STARTUP_LOG")) else {
        return;
    };
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "startup_ms={}", start.elapsed().as_millis());
    }
}

fn probe_path() -> Option<PathBuf> {
    std::env::var_os("RUSTCHROME_PROBE").map(PathBuf::from)
}

fn finish_probe(view: &WebView, path: &PathBuf) {
    let text = view.title().map(|s| s.to_string()).unwrap_or_default();
    // The first Finished event is the about:blank we started from; the report
    // only arrives once the probe page has run, and it starts with "{".
    if !text.starts_with('{') && !text.starts_with("FAIL") {
        return;
    }
    let _ = std::fs::write(path, text);
    std::process::exit(0);
}

const NEW_TAB_HTML: &str = r#"<!doctype html><html><head><meta charset="utf-8"><title>New Tab</title><style>
*{box-sizing:border-box}
body{margin:0;height:100vh;display:flex;flex-direction:column;align-items:center;justify-content:center;
font-family:arial,sans-serif;background:#fff;color:#202124}
.logo{font-size:72px;letter-spacing:-4px;font-weight:500;margin-bottom:26px;user-select:none}
form{width:min(584px,86vw)}
input{width:100%;height:46px;background:#fff;color:#202124;border:1px solid #dfe1e5;border-radius:24px;padding:0 20px;font-size:16px;outline:none}
input:hover,input:focus{box-shadow:0 1px 6px rgba(32,33,36,.28);border-color:rgba(223,225,229,0)}
.btns{margin-top:28px;display:flex;gap:12px}
button{background:#f8f9fa;border:1px solid #f8f9fa;border-radius:4px;color:#3c4043;font-size:14px;
padding:10px 18px;cursor:pointer}
button:hover{box-shadow:0 1px 1px rgba(0,0,0,.1);border-color:#dadce0;color:#202124}
.foot{position:fixed;bottom:20px;font-size:12px;color:#70757a}
a{color:#1a73e8;text-decoration:none}
</style></head><body>
<div class="logo"><span style="color:#4285f4">G</span><span style="color:#ea4335">o</span><span style="color:#fbbc05">o</span><span style="color:#4285f4">g</span><span style="color:#34a853">l</span><span style="color:#ea4335">e</span></div>
<form action="https://www.google.com/search" method="get">
<input name="q" autofocus placeholder="Search Google or type a URL" autocomplete="off">
</form>
<div class="btns"><button onclick="document.querySelector('form').requestSubmit()">Google Search</button>
<button onclick="location.href='https://www.google.com/doodles'">I'm Feeling Lucky</button></div>
<div class="foot">RustChrome v0.2 &middot; <a href="https://github.com">github</a></div>
</body></html>"#;

const FORCE_DARK_CSS: &str = r#"
html { background-color: #ffffff !important; }
html { filter: invert(0.92) hue-rotate(180deg) !important; }
img, picture, video, canvas, svg, iframe { filter: invert(1) hue-rotate(180deg) !important; }
"#;

const CSS: &str = r#"
window { background: #ffffff; }
window.dark { background: #202124; }

headerbar { background: #dee1e6; border: none; box-shadow: none; min-height: 40px;
            padding: 0 2px 0 4px; }
headerbar button.titlebutton { background: transparent; border: none; box-shadow: none;
            min-width: 24px; min-height: 24px; padding: 2px; margin: 0 2px; color: #5f6368; }
headerbar button.titlebutton:hover { background: #cfd3d9; border-radius: 999px; color: #202124; }
.dark headerbar { background: #202124; }
.dark headerbar button.titlebutton { color: #9aa0a6; }
.dark headerbar button.titlebutton:hover { background: #35363a; color: #e8eaed; }
#tabstrip { background: transparent; padding: 4px 0 0 0; }
.dark #tabstrip { background: transparent; }
.tab { background: transparent; border-radius: 8px 8px 0 0; padding: 0 6px 0 12px;
       margin: 0 2px 0 0; color: #5f6368; min-height: 34px; }
.dark .tab { color: #9aa0a6; }
.tab:hover { background: #cfd3d9; }
.dark .tab:hover { background: #2d2f31; }
.tab.active { background: #ffffff; color: #202124; }
.dark .tab.active { background: #35363a; color: #e8eaed; }
.tab label { font-size: 12px; }
.tab button { background: transparent; border: none; padding: 0; color: inherit; min-height: 0; }
.closetab { padding: 0 2px; min-width: 18px; min-height: 18px; border-radius: 999px; color: #5f6368; }
.dark .closetab { color: #9aa0a6; }
.closetab:hover { background: #dadce0; color: #202124; }
.dark .closetab:hover { background: #4a4d51; color: #ffffff; }
#newtab { background: transparent; border: none; color: #5f6368; font-size: 19px;
          padding: 0 10px; border-radius: 999px; }
.dark #newtab { color: #9aa0a6; }
#newtab:hover { background: #cfd3d9; color: #202124; }
.dark #newtab:hover { background: #2d2f31; color: #e8eaed; }
#toolbar { background: #ffffff; padding: 4px 8px; }
.dark #toolbar { background: #35363a; }
#toolbar button { background: transparent; border: none; padding: 6px; border-radius: 999px;
                  color: #5f6368; }
.dark #toolbar button { color: #9aa0a6; }
#toolbar button:hover { background: #f1f3f4; }
.dark #toolbar button:hover { background: #4a4d51; color: #e8eaed; }
#toolbar button:disabled { color: #bdc1c6; }
.dark #toolbar button:disabled { color: #5f6368; }
#urlem { font-size: 14px; background: #f1f3f4; border: 1px solid transparent; border-radius: 999px;
         padding: 8px 16px; min-height: 30px; color: #202124; caret-color: #1a73e8; }
.dark #urlem { background: #202124; color: #e8eaed; caret-color: #8ab4f8; }
#urlem:focus { background: #ffffff; border-color: #1a73e8; box-shadow: 0 1px 6px rgba(32,33,36,.28); }
.dark #urlem:focus { background: #202124; border-color: #8ab4f8; box-shadow: 0 1px 6px rgba(0,0,0,.6); }
#bookmarksbar { background: #ffffff; padding: 0 6px 4px 6px; }
.dark #bookmarksbar { background: #35363a; }
#bookmarksbar button { background: transparent; border: none; color: #3c4043; font-size: 12px;
                       padding: 4px 8px; border-radius: 4px; }
#bookmarksbar button label { font-size: 12px; }
.dark #bookmarksbar button { color: #bdc1c6; }
#bookmarksbar button:hover { background: #f1f3f4; }
.dark #bookmarksbar button:hover { background: #4a4d51; }
#findentry { font-size: 13px; }
#findbar { background: #ffffff; padding: 4px 10px; border-top: 1px solid #e8eaed; }
.dark #findbar { background: #35363a; border-top: 1px solid #202124; }
menu { background: #ffffff; padding: 6px 0; }
.dark menu { background: #35363a; }
menuitem { padding: 7px 16px; font-size: 13px; color: #202124; }
.dark menuitem { color: #e8eaed; }
menuitem:hover { background: #f1f3f4; }
.dark menuitem:hover { background: #4a4d51; }
menu separator { background: #e8eaed; }
.dark menu separator { background: #4a4d51; }

#avatar-spacer { min-width: 2px; }

#progress { min-height: 2px; background: transparent; }
#progress trough { min-height: 2px; border: none; background: transparent; }
#progress progress { min-height: 2px; border: none; background: #1a73e8; }
.dark #progress progress { background: #8ab4f8; }
"#;

#[derive(Serialize, Deserialize, Clone)]
struct Bookmark {
    title: String,
    url: String,
}

struct Tab {
    id: u64,
    view: WebView,
    tab_box: EventBox,
    label: Label,
    title: RefCell<String>,
    uri: RefCell<String>,
    zoom: Cell<f64>,
}

impl Tab {
    fn refresh_label(&self) {
        let raw = self.title.borrow().clone();
        let text: String = if raw.is_empty() {
            "New Tab".into()
        } else if raw.chars().count() > 22 {
            raw.chars().take(22).collect::<String>() + "…"
        } else {
            raw
        };
        self.label.set_text(&text);
    }
}

struct App {
    me: RefCell<Weak<App>>,
    window: ApplicationWindow,
    stack: Stack,
    tabs_box: GtkBox,
    entry: Entry,
    back: Button,
    forward: Button,
    reload: Button,
    star: Button,
    progress: ProgressBar,
    bookmarksbar: GtkBox,
    findbar: GtkBox,
    findentry: SearchEntry,
    css: RefCell<CssProvider>,
    theme_btn: Button,
    icon_btns: Vec<Button>,
    ucm: UserContentManager,
    ctx: WebContext,
    seed: Cell<SessionSeed>,
    ui_zoom: Cell<f64>,
    force_dark: Cell<bool>,
    antifp: Cell<bool>,
    strict: Cell<bool>,
    fp_script: RefCell<Option<webkit2gtk::UserScript>>,
    dark_sheet: RefCell<Option<UserStyleSheet>>,
    dark: Cell<bool>,
    tabs: RefCell<Vec<Rc<Tab>>>,
    bookmarks: RefCell<Vec<Bookmark>>,
    closed: RefCell<Vec<String>>,
    active: Cell<usize>,
    next_id: Cell<u64>,
    zoom: Cell<f64>,
}

fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let dir = PathBuf::from(home).join(".local/share/rustchrome");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn normalize(input: &str) -> String {
    let t = input.trim();
    if t.is_empty() {
        return NEW_TAB_PAGE.into();
    }
    let lower = t.to_lowercase();
    if lower.starts_with("about:")
        || lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("file://")
        || lower.starts_with("data:")
    {
        return t.to_string();
    }
    let looks_like_host = !t.contains(' ')
        && !t.starts_with('-')
        && (t.contains('.') || lower == "localhost" || lower.starts_with("localhost:"));
    if looks_like_host {
        format!("https://{}", t)
    } else {
        let q = t
            .replace('%', "%25")
            .replace(' ', "+")
            .replace('&', "%26")
            .replace('?', "%3F")
            .replace('#', "%23")
            .replace('=', "%3D");
        format!("https://www.google.com/search?q={}", q)
    }
}

/// New-tab page in the right palette without needing a page-wide CSS filter.
fn ntp_html(dark: bool) -> String {
    let base = NEW_TAB_HTML.to_string();
    if !dark {
        return base;
    }
    base.replace("background:#fff", "background:#202124")
        .replace("color:#202124", "color:#e8eaed")
        .replace("border:1px solid #dfe1e5", "border:1px solid #5f6368")
        .replace("background:#f8f9fa", "background:#303134")
        .replace("border:1px solid #f8f9fa", "border:1px solid #303134")
        .replace("color:#3c4043", "color:#e8eaed")
        .replace("color:#70757a", "color:#9aa0a6")
}

fn install_css(win: &ApplicationWindow) -> CssProvider {
    let provider = CssProvider::new();
    if let Err(e) = provider.load_from_data(CSS.as_bytes()) {
        eprintln!("css error: {e}");
    }
    let screen = gtk::prelude::WidgetExt::screen(win).or_else(gtk::gdk::Screen::default);
    if let Some(screen) = screen {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    provider
}

// ---------------------------------------------------------------- preferences
//
// `prefs.json` has exactly one writer: the typed store in `rc-core`.
//
// The shell used to rewrite the file from raw JSON containing only the keys it
// happened to know about, so toggling the theme silently erased the zoom level,
// the anti-fingerprinting setting and the cleanup preference. Every read and
// write now goes through `Prefs`, which round-trips everything it does not
// recognise rather than dropping it.

fn get_pref(key: &str) -> Option<bool> {
    let p = rc_core::store::Prefs::load();
    match key {
        "dark" => Some(p.is_dark()),
        other => p.get_bool(other),
    }
}

fn set_pref(key: &str, val: bool) {
    let mut p = rc_core::store::Prefs::load();
    let (name, value) = match key {
        "dark" => (
            "theme",
            serde_json::json!(if val { "dark" } else { "light" }),
        ),
        other => (other, serde_json::json!(val)),
    };
    if p.set_json(name, &value) {
        let _ = p.save();
    }
}

fn get_pref_f64(key: &str) -> Option<f64> {
    rc_core::store::Prefs::load().get_f64(key)
}

fn set_pref_f64(key: &str, val: f64) {
    let mut p = rc_core::store::Prefs::load();
    if p.set_json(key, &serde_json::json!(val)) {
        let _ = p.save();
    }
}

fn load_strict() -> bool {
    get_pref("strict").unwrap_or(false)
}

fn load_dark() -> bool {
    get_pref("dark").unwrap_or(false)
}

fn save_dark(dark: bool) {
    set_pref("dark", dark);
}

fn load_force_dark() -> bool {
    get_pref("force_dark").unwrap_or(false)
}

fn save_force_dark(force: bool) {
    set_pref("force_dark", force);
}

/// First-run zoom: follow the screen instead of assuming 1080p.
fn resolve_zoom(window: &ApplicationWindow) -> f64 {
    if let Some(z) = get_pref_f64("zoom") {
        return z.clamp(0.25, 5.0);
    }
    let scale = window.scale_factor() as f64;
    if scale > 1.0 {
        return 1.0; // GTK already scales HiDPI
    }
    let width = gtk::prelude::WidgetExt::screen(window)
        .and_then(|s| s.display().primary_monitor())
        .map(|m| m.geometry().width() as f64)
        .unwrap_or(1920.0);
    ((width / 1920.0).clamp(1.0, 1.4) * 20.0).round() / 20.0
}

fn load_antifp() -> bool {
    get_pref("antifp").unwrap_or(true)
}

/// Strict mode denies WebGL and removes WebRTC instead of coercing them.
/// A clean exit must not look like a crash to the next run.
fn mark_session_clean() {
    let mut s = rc_core::features::SessionState::load();
    s.dirty = false;
    s.tabs.clear();
    let _ = s.save();
}

/// The app logo, compiled into the binary (no external asset needed at runtime).
fn app_icon() -> Option<gtk::gdk_pixbuf::Pixbuf> {
    let loader = gtk::gdk_pixbuf::PixbufLoader::new();
    loader
        .write(include_bytes!("../assets/icon-256.png"))
        .ok()?;
    loader.close().ok()?;
    loader.pixbuf()
}

fn icon_button(name: &str, tip: &str) -> Button {
    let b = Button::new();
    let img = Image::from_icon_name(Some(name), gtk::IconSize::LargeToolbar);
    img.set_pixel_size(16);
    b.set_image(Some(&img));
    b.set_tooltip_text(Some(tip));
    b
}

fn theme_btn_conn<F: Fn() + 'static>(b: &Button, f: F) {
    b.connect_clicked(move |_| f());
}

/// Scales every `Npx` length in the stylesheet so the whole chrome grows with
/// the page zoom factor.
fn scale_css(css: &str, scale: f64) -> String {
    let chars: Vec<char> = css.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let num: String = chars[start..i].iter().collect();
            if i + 1 < chars.len() && chars[i] == 'p' && chars[i + 1] == 'x' {
                let v: f64 = num.parse().unwrap_or(1.0);
                let scaled = if v == 0.0 {
                    0.0
                } else {
                    (v * scale).round().max(1.0)
                };
                out.push_str(&format!("{}px", scaled as i64));
                i += 2;
                continue;
            }
            out.push_str(&num);
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn scale_images(w: &gtk::Widget, px: i32) {
    if let Ok(img) = w.clone().downcast::<gtk::Image>() {
        img.set_pixel_size(px);
    }
    if let Ok(container) = w.clone().downcast::<gtk::Container>() {
        for child in container.children() {
            scale_images(&child, px);
        }
    }
}

fn set_icon(b: &Button, name: &str) {
    b.set_image(Some(&Image::from_icon_name(
        Some(name),
        gtk::IconSize::LargeToolbar,
    )));
}

impl App {
    fn build(app: &Application) -> Rc<Self> {
        let window = ApplicationWindow::new(app);
        window.set_title("New Tab - RustChrome");
        window.set_default_size(1280, if load_antifp() { 800 } else { 820 });
        window.set_icon_name(Some("web-browser"));
        if let Some(pb) = app_icon() {
            window.set_icon(Some(&pb));
        }

        let root = GtkBox::new(Orientation::Vertical, 0);

        // tab strip
        let tabstrip = GtkBox::new(Orientation::Horizontal, 0);
        tabstrip.set_widget_name("tabstrip");
        let tabs_box = GtkBox::new(Orientation::Horizontal, 0);
        tabstrip.pack_start(&tabs_box, false, false, 0);

        let new_tab_btn = Button::with_label("+");
        new_tab_btn.set_widget_name("newtab");
        new_tab_btn.set_relief(gtk::ReliefStyle::None);
        new_tab_btn.set_tooltip_text(Some("New tab (Ctrl+T)"));
        tabstrip.pack_start(&new_tab_btn, false, false, 0);
        // Absorb all slack at the far end so tabs stay packed next to "+".
        let strip_spacer = GtkBox::new(Orientation::Horizontal, 0);
        strip_spacer.set_hexpand(true);
        tabstrip.pack_end(&strip_spacer, true, true, 0);

        // toolbar
        let toolbar = GtkBox::new(Orientation::Horizontal, 2);
        toolbar.set_widget_name("toolbar");
        let back = icon_button("go-previous-symbolic", "Back");
        let forward = icon_button("go-next-symbolic", "Forward");
        let reload = icon_button("view-refresh-symbolic", "Reload");
        let home = icon_button("go-home-symbolic", "Home");
        let star = icon_button("non-starred-symbolic", "Bookmark this tab");
        back.set_sensitive(false);
        forward.set_sensitive(false);

        let entry = Entry::new();
        entry.set_widget_name("urlem");
        entry.set_placeholder_text(Some("Search Google or type a URL"));
        entry.set_hexpand(true);
        entry.set_primary_icon_name(Some("system-search-symbolic"));

        let theme_btn = icon_button("weather-clear-night-symbolic", "Dark mode");
        let menu_btn = icon_button("view-more-symbolic", "Customize and control RustChrome");

        toolbar.pack_start(&back, false, false, 0);
        toolbar.pack_start(&forward, false, false, 0);
        toolbar.pack_start(&reload, false, false, 0);
        toolbar.pack_start(&entry, true, true, 8);
        // Chrome's profile bubble: a drawn circle, so nothing can stretch it.
        let avatar_area = gtk::DrawingArea::new();
        avatar_area.set_size_request(30, 30);
        avatar_area.connect_draw(|_, cr| {
            let size = 30.0_f64;
            cr.set_source_rgb(0.102, 0.451, 0.910); // #1a73e8
            cr.arc(
                size / 2.0,
                size / 2.0,
                size / 2.0,
                0.0,
                std::f64::consts::PI * 2.0,
            );
            let _ = cr.fill();
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.select_font_face(
                "Sans",
                gtk::cairo::FontSlant::Normal,
                gtk::cairo::FontWeight::Bold,
            );
            cr.set_font_size(13.0);
            let ext = cr.text_extents("T").map(|e| e.width()).unwrap_or(0.0);
            cr.move_to((size - ext) / 2.0, size / 2.0 + 4.5);
            let _ = cr.show_text("T");
            gtk::glib::Propagation::Proceed
        });
        avatar_area.set_valign(gtk::Align::Center);
        let avatar = EventBox::new();
        avatar.set_visible_window(false);
        avatar.set_valign(gtk::Align::Center);
        avatar.set_tooltip_text(Some("Profile"));
        avatar.add(&avatar_area);
        toolbar.pack_start(&star, false, false, 0);
        toolbar.pack_start(&avatar, false, false, 0);
        toolbar.pack_start(&menu_btn, false, false, 0);

        // bookmarks bar
        let bookmarksbar = GtkBox::new(Orientation::Horizontal, 0);
        bookmarksbar.set_widget_name("bookmarksbar");
        bookmarksbar.set_no_show_all(true);

        // find bar
        let findbar = GtkBox::new(Orientation::Horizontal, 8);
        findbar.set_widget_name("findbar");
        findbar.set_no_show_all(true);
        let findentry = SearchEntry::new();
        findentry.set_widget_name("findentry");
        findentry.set_placeholder_text(Some("Find in page"));
        findentry.set_width_chars(28);
        let find_close = icon_button("window-close-symbolic", "Close");
        findbar.pack_start(&findentry, false, false, 0);
        findbar.pack_end(&find_close, false, false, 0);

        let progress = ProgressBar::new();
        progress.set_widget_name("progress");
        progress.set_show_text(false);
        progress.set_no_show_all(true);

        let stack = Stack::new();
        stack.set_transition_type(StackTransitionType::None);
        stack.set_hexpand(true);
        stack.set_vexpand(true);

        let header = gtk::HeaderBar::new();
        header.set_show_close_button(true);
        header.set_custom_title(Some(&tabstrip));
        header.set_has_subtitle(false);
        root.pack_start(&header, false, false, 0);
        root.pack_start(&toolbar, false, false, 0);
        root.pack_start(&bookmarksbar, false, false, 0);
        root.pack_start(&findbar, false, false, 0);
        root.pack_start(&progress, false, false, 0);
        root.pack_start(&Separator::new(Orientation::Horizontal), false, false, 0);
        root.pack_start(&stack, true, true, 0);
        window.add(&root);

        let ucm = UserContentManager::new();
        // Ephemeral context: cookies, cache, localStorage, IndexedDB and
        // service workers live in memory and never reach the disk, so a crash
        // or a kill cannot leave a session behind.
        // Private Session (default): nothing is written to disk. Standard
        // Session reuses WebKit's persistent context so logins survive.
        let keep_session = rc_core::store::Prefs::load().keep_session;
        let ctx = if keep_session {
            WebContext::default().unwrap_or_else(WebContext::new_ephemeral)
        } else {
            WebContext::new_ephemeral()
        };
        ctx.set_preferred_languages(&["en-US", "en"]);
        if let Some(cm) = ctx.cookie_manager() {
            cm.set_accept_policy(CookieAcceptPolicy::NoThirdParty);
        }
        if let Some(dm) = ctx.website_data_manager() {
            dm.set_itp_enabled(true);
        }
        let dark_now = load_dark();
        let zoom_now = resolve_zoom(&window);
        let css_provider = install_css(&window);
        let icon_btns = vec![
            back.clone(),
            forward.clone(),
            reload.clone(),
            home.clone(),
            star.clone(),
            theme_btn.clone(),
            menu_btn.clone(),
        ];
        let me = Rc::new(App {
            me: RefCell::new(Weak::new()),
            css: RefCell::new(css_provider),
            theme_btn: theme_btn.clone(),
            icon_btns,
            ucm,
            ctx,
            seed: Cell::new(SessionSeed::new()),
            strict: Cell::new(load_strict()),
            ui_zoom: Cell::new(0.0),
            force_dark: Cell::new(load_force_dark()),
            antifp: Cell::new(load_antifp()),
            fp_script: RefCell::new(None),
            dark_sheet: RefCell::new(None),
            dark: Cell::new(dark_now),
            window,
            stack,
            tabs_box: tabs_box.clone(),
            entry,
            back,
            forward,
            reload,
            star,
            progress,
            bookmarksbar,
            findbar,
            findentry: findentry.clone(),
            tabs: RefCell::new(Vec::new()),
            bookmarks: RefCell::new(load_bookmarks()),
            closed: RefCell::new(Vec::new()),
            active: Cell::new(0),
            next_id: Cell::new(1),
            zoom: Cell::new(zoom_now),
        });
        *me.me.borrow_mut() = Rc::downgrade(&me);
        me.wire(&new_tab_btn, &home, &menu_btn, &avatar, &find_close);
        me.apply_theme(dark_now, false);
        me.apply_ui_zoom(zoom_now);
        me.refresh_bookmarks_bar();
        me.setup_downloads();
        me.install_extensions();
        me.sync_fp_script();
        me.startup_tabs();
        me
    }

    fn rc(&self) -> Rc<App> {
        self.me.borrow().upgrade().expect("app is alive")
    }

    fn wire(
        self: &Rc<Self>,
        new_tab_btn: &Button,
        home: &Button,
        menu_btn: &Button,
        avatar: &EventBox,
        find_close: &Button,
    ) {
        {
            let a = self.rc();
            new_tab_btn.connect_clicked(move |_| {
                a.new_tab(NEW_TAB_PAGE);
            });
        }
        {
            let a = self.rc();
            home.connect_clicked(move |_| {
                if let Some(t) = a.current_tab() {
                    a.navigate(&t, HOME);
                }
            });
        }
        {
            let a = self.rc();
            self.entry.connect_activate(move |e| {
                let text = e.text().to_string();
                if let Some(tab) = a.current_tab() {
                    a.navigate(&tab, &text);
                }
            });
        }
        {
            self.entry.connect_button_press_event(|e, ev| {
                if ev.button() == 1 {
                    e.select_region(0, -1);
                }
                gtk::glib::Propagation::Proceed
            });
        }
        {
            let a = self.rc();
            self.back
                .connect_clicked(move |_| a.with_current(|v| v.go_back()));
        }
        {
            let a = self.rc();
            self.forward
                .connect_clicked(move |_| a.with_current(|v| v.go_forward()));
        }
        {
            let a = self.rc();
            self.reload.connect_clicked(move |_| {
                a.with_current(|v| {
                    if v.is_loading() {
                        v.stop_loading();
                    } else {
                        v.reload();
                    }
                })
            });
        }
        {
            let a = self.rc();
            self.star.connect_clicked(move |_| a.toggle_bookmark());
        }
        {
            let a = self.rc();
            menu_btn.connect_clicked(move |b| a.show_menu(b));
        }
        {
            let a = self.rc();
            let anchor = menu_btn.clone();
            avatar.connect_button_press_event(move |_, _| {
                a.show_menu(&anchor);
                gtk::glib::Propagation::Stop
            });
        }
        {
            let a = self.rc();
            theme_btn_conn(&self.theme_btn, move || {
                let d = !a.dark.get();
                a.apply_theme(d, true);
            });
        }

        // find bar
        {
            let a = self.rc();
            self.findentry.connect_search_changed(move |e| {
                let text = e.text().to_string();
                if let Some(t) = a.current_tab() {
                    if let Some(fc) = t.view.find_controller() {
                        if text.is_empty() {
                            fc.search_finish();
                        } else {
                            fc.search(&text, 0, 0);
                        }
                    }
                }
            });
        }
        {
            let a = self.rc();
            self.findentry.connect_activate(move |e| {
                let text = e.text().to_string();
                if let Some(t) = a.current_tab() {
                    if let Some(fc) = t.view.find_controller() {
                        fc.search(&text, 0, 0);
                    }
                }
            });
        }
        {
            let a = self.rc();
            find_close.connect_clicked(move |_| a.hide_find());
        }

        {
            self.window.connect_delete_event(move |w, _| {
                let keep = rc_core::store::Prefs::load().keep_session;
                if !keep && get_pref("autoclear").unwrap_or(true) {
                    w.hide();
                    clear_session_data(|| {
                        mark_session_clean();
                        std::process::exit(0);
                    });
                    return gtk::glib::Propagation::Stop;
                }
                gtk::glib::Propagation::Proceed
            });
        }

        // keyboard
        {
            let a = self.rc();
            self.window.connect_key_press_event(move |_, ev| {
                use gtk::gdk::ModifierType;
                let ctrl = ev.state().contains(ModifierType::CONTROL_MASK);
                let alt = ev.state().contains(ModifierType::MOD1_MASK);
                let shift = ev.state().contains(ModifierType::SHIFT_MASK);
                let key = ev
                    .keyval()
                    .name()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                match (ctrl, alt, key.as_str()) {
                    (true, _, "t") | (true, _, "T") if shift => a.reopen_closed(),
                    (true, _, "t") | (true, _, "T") => {
                        a.new_tab(NEW_TAB_PAGE);
                    }
                    (true, _, "w") | (true, _, "W") => a.close_active_tab(),
                    (true, _, "l") | (true, _, "L") => {
                        a.entry.grab_focus();
                        a.entry.select_region(0, -1);
                    }
                    (true, _, "r") | (true, _, "R") | (false, _, "F5") => {
                        a.with_current(|v| v.reload())
                    }
                    (true, _, "q") => {
                        mark_session_clean();
                        std::process::exit(0);
                    }
                    (true, _, "A") if shift => a.tab_search_dialog(),
                    (true, _, "f") | (true, _, "F") => a.show_find(),
                    (false, _, "Escape") => a.hide_find(),
                    (true, _, "d") | (true, _, "D") => a.toggle_bookmark(),
                    (true, _, "plus") | (true, _, "equal") => a.zoom_by(0.1),
                    (true, _, "minus") => a.zoom_by(-0.1),
                    (true, _, "0") => a.zoom_reset(),
                    (false, _, "F11") => {
                        let fs = a
                            .window
                            .window()
                            .map(|w| w.state().contains(gtk::gdk::WindowState::FULLSCREEN))
                            .unwrap_or(false);
                        if fs {
                            a.window.unfullscreen();
                        } else {
                            a.window.fullscreen();
                        }
                    }
                    (true, _, "ISO_Left_Tab") if shift => a.cycle_tab(-1),
                    (true, _, "Tab") if shift => a.cycle_tab(-1),
                    (true, _, "Tab") => a.cycle_tab(1),
                    (_, true, "Left") => a.with_current(|v| v.go_back()),
                    (_, true, "Right") => a.with_current(|v| v.go_forward()),
                    _ => return gtk::glib::Propagation::Proceed,
                }
                gtk::glib::Propagation::Stop
            });
        }
    }

    fn startup_tabs(&self) {
        if probe_path().is_some() {
            let t = self.new_tab("about:blank");
            t.view.load_html(&rc_core::privacy::probe_html(), None);
            self.window.show_all();
            self.progress.hide();
            self.findbar.hide();
            return;
        }
        let prefs = rc_core::store::Prefs::load();
        let prev = rc_core::features::SessionState::load();
        if prefs.restore_session && rc_core::features::crashed(&prev) {
            for u in prev.tabs.iter().take(20) {
                self.new_tab(u);
            }
        } else {
            let start = std::env::var("RUSTCHROME_START_URL").unwrap_or_else(|_| HOME.to_string());
            for u in start.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                self.new_tab(u);
            }
        }
        self.persist_session();
        self.window.show_all();
        if let Ok(z) = std::env::var("RUSTCHROME_UI_ZOOM") {
            if let Ok(z) = z.parse::<f64>() {
                self.apply_ui_zoom(z);
            }
        }
        self.progress.hide();
        self.findbar.hide();
        self.bookmarksbar
            .set_visible(!self.bookmarks.borrow().is_empty());
    }

    fn current_tab(&self) -> Option<Rc<Tab>> {
        self.tabs.borrow().get(self.active.get()).cloned()
    }

    fn with_current<F: FnOnce(&WebView)>(&self, f: F) {
        if let Some(t) = self.current_tab() {
            f(&t.view);
            self.sync_nav();
        }
    }

    /// Build and configure one tab's webview. Settings come from the current
    /// privacy mode, so this must be called with `antifp`/`strict` already read.
    fn create_view(&self) -> WebView {
        let view = WebView::builder()
            .web_context(&self.ctx)
            .user_content_manager(&self.ucm)
            .build();
        if let Some(s) = WebViewExt::settings(&view) {
            s.set_enable_developer_extras(false);
            s.set_javascript_can_open_windows_automatically(false);
            s.set_default_font_size(15);
            s.set_enable_smooth_scrolling(false);
            s.set_enable_dns_prefetching(false);
            s.set_enable_hyperlink_auditing(false);
            s.set_enable_media_stream(false);
            if self.antifp.get() && self.strict.get() {
                // Strict mode is the only mode that takes WebGL and WebRTC away
                // from the engine; standard mode coerces them from script so
                // ordinary sites keep working.
                s.set_enable_webgl(false);
                s.set_enable_webrtc(false);
            }
            if let Ok(v) = std::env::var("RUSTCHROME_HWACCEL") {
                let policy = match v.as_str() {
                    "never" => webkit2gtk::HardwareAccelerationPolicy::Never,
                    "always" => webkit2gtk::HardwareAccelerationPolicy::Always,
                    _ => webkit2gtk::HardwareAccelerationPolicy::OnDemand,
                };
                s.set_hardware_acceleration_policy(policy);
            }
        }
        view.connect_permission_request(|_v, req| {
            use webkit2gtk::PermissionRequestExt;
            req.deny();
            true
        });
        view.set_hexpand(true);
        view.set_vexpand(true);
        let bg = if self.dark.get() {
            gtk::gdk::RGBA::new(0.125, 0.129, 0.141, 1.0)
        } else {
            gtk::gdk::RGBA::new(1.0, 1.0, 1.0, 1.0)
        };
        view.set_background_color(&bg);
        view.set_zoom_level(self.zoom.get());
        view
    }

    fn new_tab(&self, url: &str) -> Rc<Tab> {
        let id = self.next_id.get();
        self.next_id.set(id + 1);

        let label = Label::new(Some("New Tab"));
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_width_chars(14);
        label.set_max_width_chars(18);

        // Two sibling GtkButtons (never nested, never inside another button):
        // the label area activates the tab, the "x" closes it. Both are plain
        // buttons, so mouse clicks AND accessibility actions reach them.
        let label_btn = Button::new();
        label_btn.set_relief(gtk::ReliefStyle::None);
        label_btn.style_context().add_class("tablabel");
        label_btn.add(&label);

        let close_btn = Button::new();
        close_btn.set_relief(gtk::ReliefStyle::None);
        close_btn.style_context().add_class("closetab");
        close_btn.set_image(Some(&Image::from_icon_name(
            Some("window-close-symbolic"),
            gtk::IconSize::Menu,
        )));
        close_btn.set_tooltip_text(Some("Close tab"));

        let inner = GtkBox::new(Orientation::Horizontal, 4);
        inner.pack_start(&label_btn, true, true, 0);
        inner.pack_start(&close_btn, false, false, 0);

        let tab_widget = EventBox::new();
        tab_widget.set_visible_window(true);
        tab_widget.style_context().add_class("tab");
        tab_widget.add(&inner);
        self.tabs_box.pack_start(&tab_widget, false, false, 0);
        tab_widget.show_all();

        let view = self.create_view();
        let page = format!("tab-{}", id);
        self.stack.add_named(&view, page.as_str());
        view.show();

        let tab = Rc::new(Tab {
            id,
            view: view.clone(),
            tab_box: tab_widget.clone(),
            label,
            title: RefCell::new(String::new()),
            uri: RefCell::new(String::new()),
            zoom: Cell::new(self.zoom.get()),
        });
        self.tabs.borrow_mut().push(tab.clone());

        {
            let a = Rc::downgrade(&self.rc());
            let tw = Rc::downgrade(&tab);
            view.connect_title_notify(move |v| {
                if let Some(path) = probe_path() {
                    finish_probe(v, &path);
                }
                let (Some(a), Some(t)) = (a.upgrade(), tw.upgrade()) else {
                    return;
                };
                *t.title.borrow_mut() = v.title().map(|s| s.to_string()).unwrap_or_default();
                t.refresh_label();
                if a.index_of(&t) == a.active.get() {
                    a.update_window_title(&t);
                }
            });
        }
        {
            let a = Rc::downgrade(&self.rc());
            let tw = Rc::downgrade(&tab);
            view.connect_uri_notify(move |v| {
                let (Some(a), Some(t)) = (a.upgrade(), tw.upgrade()) else {
                    return;
                };
                let raw = v.uri().map(|s| s.to_string()).unwrap_or_default();
                let shown = if raw == "about:blank" {
                    String::new()
                } else {
                    raw
                };
                *t.uri.borrow_mut() = shown;
                if a.index_of(&t) == a.active.get() {
                    if !a.entry.has_focus() && a.entry.text() != t.uri.borrow().as_str() {
                        a.entry.set_text(&t.uri.borrow());
                    }
                    a.sync_nav();
                }
            });
        }
        {}
        {
            let a = Rc::downgrade(&self.rc());
            let tw = Rc::downgrade(&tab);
            view.connect_load_changed(move |_, ev| {
                if ev == LoadEvent::Finished {
                    log_startup_once();
                }
                let (Some(a), Some(t)) = (a.upgrade(), tw.upgrade()) else {
                    return;
                };
                if a.index_of(&t) != a.active.get() {
                    return;
                }
                match ev {
                    LoadEvent::Started | LoadEvent::Redirected => {
                        a.progress.show();
                        a.progress.pulse();
                    }
                    _ => {
                        a.progress.hide();
                        a.sync_nav();
                    }
                }
            });
        }
        {
            let a = Rc::downgrade(&self.rc());
            view.connect_estimated_load_progress_notify(move |v| {
                let Some(a) = a.upgrade() else { return };
                let p = v.estimated_load_progress();
                if p > 0.0 && p < 1.0 {
                    // only repaint when the value moves enough to be visible
                    if (p - a.progress.fraction()).abs() > 0.02 {
                        a.progress.show();
                        a.progress.set_fraction(p);
                    }
                } else if a.progress.is_visible() {
                    a.progress.hide();
                }
            });
        }
        {
            // Opening target=_blank / window.open ourselves. Letting WebKit build the
            // window hits a libwebkit2gtk assertion (WindowFeatures) and aborts the app.
            let a = Rc::downgrade(&self.rc());
            view.connect_decide_policy(move |_v, decision, kind| {
                if kind != PolicyDecisionType::NewWindowAction {
                    return false;
                }
                let Some(a) = a.upgrade() else { return false };
                let uri = decision
                    .downcast_ref::<NavigationPolicyDecision>()
                    .and_then(|nav| nav.navigation_action())
                    .and_then(|action| action.request())
                    .and_then(|req| req.uri())
                    .map(|s| s.to_string());
                if let Some(u) = uri {
                    a.new_tab(&u);
                }
                decision.ignore();
                true
            });
        }
        {
            let a = Rc::downgrade(&self.rc());
            view.connect_load_failed(move |v, _ev, uri, err| {
                if a.upgrade().is_none() {
                    return false;
                }
                let msg = err.message().to_string();
                if msg.contains("cancelled") || uri.starts_with("about:") {
                    return false;
                }
                let html = format!(
                    "<!doctype html><html><head><meta charset='utf-8'><style>\
                     body{{margin:0;height:100vh;display:flex;align-items:center;justify-content:center;\
                     font-family:arial,sans-serif;background:#fff;color:#202124}}\
                     div{{max-width:520px}}h2{{font-weight:400;margin:0 0 18px;font-size:22px}}\
                     code{{background:#f1f3f4;padding:3px 6px;border-radius:4px;font-size:13px}}\
                     p{{color:#5f6368;line-height:1.6;font-size:14px}}\
                     a{{color:#1a73e8}}</style></head><body><div>\
                     <h2>This site can&rsquo;t be reached</h2>\
                     <p><b>{}</b> could not be loaded.</p><p><code>{}</code></p>\
                     <p>Check the address, or your network connection.</p>\
                     <p><a href='{}'>Try again</a></p></div></body></html>",
                    uri, msg, uri
                );
                v.load_html(&html, None);
                false
            });
        }
        {
            let a = Rc::downgrade(&self.rc());
            let tw = Rc::downgrade(&tab);
            label_btn.connect_clicked(move |_| {
                let (Some(a), Some(t)) = (a.upgrade(), tw.upgrade()) else {
                    return;
                };
                a.activate(&t);
            });
        }
        {
            let a = Rc::downgrade(&self.rc());
            let tw = Rc::downgrade(&tab);
            close_btn.connect_clicked(move |_| {
                let (Some(a), Some(t)) = (a.upgrade(), tw.upgrade()) else {
                    return;
                };
                a.close_tab(&t);
            });
        }

        self.activate(&tab);
        // single entry point so internal pages work from every path (CLI, "+", links)
        self.navigate(&tab, url);
        tab
    }

    fn index_of(&self, tab: &Rc<Tab>) -> usize {
        self.tabs
            .borrow()
            .iter()
            .position(|t| t.id == tab.id)
            .unwrap_or(usize::MAX)
    }

    fn update_window_title(&self, tab: &Rc<Tab>) {
        let title = tab.title.borrow().clone();
        let title = if title.is_empty() {
            "New Tab".to_string()
        } else {
            title
        };
        self.window.set_title(&format!("{} - RustChrome", title));
    }

    fn activate(&self, tab: &Rc<Tab>) {
        let idx = self.index_of(tab);
        if idx == usize::MAX {
            return;
        }
        self.active.set(idx);
        for (i, t) in self.tabs.borrow().iter().enumerate() {
            if i == idx {
                t.tab_box.style_context().add_class("active");
            } else {
                t.tab_box.style_context().remove_class("active");
            }
        }
        self.stack
            .set_visible_child_name(&format!("tab-{}", tab.id));
        let z = tab.zoom.get();
        tab.view.set_zoom_level(z);
        self.zoom.set(z);
        self.apply_ui_zoom(z);
        self.entry.set_text(&tab.uri.borrow());
        self.update_window_title(tab);
        self.sync_nav();
        self.sync_star();
    }

    fn navigate(&self, tab: &Rc<Tab>, input: &str) {
        let url = normalize(input);
        if url == EXT_PAGE {
            tab.view.load_html(&self.extensions_html(), None);
            self.entry.set_text(EXT_PAGE);
            tab.view.grab_focus();
            return;
        }
        if url == PRIVACY_PAGE {
            tab.view.load_html(&self.privacy_html(), None);
            self.entry.set_text(PRIVACY_PAGE);
            tab.view.grab_focus();
            return;
        }
        if url == BOOKMARKS_PAGE {
            tab.view.load_html(&self.bookmarks_html(), None);
            self.entry.set_text(BOOKMARKS_PAGE);
            tab.view.grab_focus();
            return;
        }
        if url == NEW_TAB_PAGE {
            tab.view.load_html(&ntp_html(self.dark.get()), None);
            tab.uri.borrow_mut().clear();
            self.entry.set_text("");
        } else {
            self.entry.set_text(&url);
            tab.view.load_uri(&url);
        }
        tab.view.grab_focus();
    }

    fn extensions_html(&self) -> String {
        let files = ext_files();
        let mut rows = String::new();
        for f in &files {
            let name = f
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            rows.push_str(&format!(
                "<tr><td class=n>{}</td><td class=u>{}</td></tr>",
                name,
                f.display()
            ));
        }
        if files.is_empty() {
            rows.push_str("<tr><td colspan=2 class=empty>No userscripts installed yet.</td></tr>");
        }
        format!(
            "<!doctype html><html><head><meta charset='utf-8'><title>Extensions</title><style>\
             body{{font-family:system-ui,sans-serif;margin:40px;max-width:820px;background:#fff;color:#202124}}\
             h1{{font-weight:400;font-size:24px;margin:0 0 4px}}p.sub{{color:#5f6368;margin:0 0 24px;font-size:14px}}\
             table{{border-collapse:collapse;width:100%;font-size:14px}}\
             td,th{{text-align:left;padding:10px 8px;border-bottom:1px solid #e8eaed}}\
             .u{{color:#5f6368;font-family:monospace;font-size:12px}}\
             .empty{{color:#80868b}}code{{background:#f1f3f4;padding:2px 5px;border-radius:4px}}\
             .box{{border:1px solid #e8eaed;border-radius:10px;padding:16px 18px;margin-top:26px;font-size:14px;color:#3c4043}}\
             </style></head><body><h1>Extensions</h1>\
             <p class=sub>RustChrome runs userscripts, not Chrome Web Store packages — WebKitGTK has no Chrome extension API.</p>\
             <table><tr><th>Script</th><th>Path</th></tr>{}</table>\
             <div class=box><b>Add one:</b> &#8942; &rarr; <i>Install userscript from URL…</i>, or drop a <code>.js</code> file into<br>\
             <code>{}</code></div></body></html>",
            rows,
            ext_dir().display()
        )
    }

    fn privacy_html(&self) -> String {
        rc_core::dashboard::dashboard_html(&self.build_dashboard())
    }

    fn build_dashboard(&self) -> rc_core::dashboard::Dashboard {
        use rc_core::dashboard::{Dashboard, Row};
        let on = |b: bool| if b { "on" } else { "off" }.to_string();
        let antifp = self.antifp.get();
        let strict = self.strict.get();
        Dashboard {
            engine: "WebKitGTK 4.1".to_string(),
            session_mode: "in-memory (ephemeral WebKit context, nothing written to disk)".to_string(),
            settings: vec![
                ("Anti-fingerprinting".to_string(), on(antifp)),
                ("Strict mode".to_string(), on(strict)),
                ("Theme".to_string(), if self.dark.get() { "dark" } else { "light" }.to_string()),
            ],
            protections: vec![
                Row {
                    name: "User agent".to_string(),
                    state: "off".to_string(),
                    note: "left as the engine reports it: overriding it to another browser would contradict the engine".to_string(),
                },
                Row {
                    name: "Canvas".to_string(),
                    state: if antifp { "partial" } else { "off" }.to_string(),
                    note: "per-session noise: same pixels within a session, different every launch".to_string(),
                },
                Row {
                    name: "WebGL".to_string(),
                    state: if !antifp { "off" } else if strict { "on" } else { "partial" }.to_string(),
                    note: if strict { "disabled in the engine".to_string() }
                        else { "available, GPU vendor/renderer coerced from script".to_string() },
                },
                Row {
                    name: "WebRTC".to_string(),
                    state: if !antifp { "off" } else if strict { "on" } else { "partial" }.to_string(),
                    note: if strict { "disabled in the engine".to_string() }
                        else { "available; WebKitGTK has no per-candidate policy, script requests relay-only".to_string() },
                },
                Row {
                    name: "Time zone".to_string(),
                    state: on(antifp),
                    note: "UTC in getTimezoneOffset, Intl and Date#toString".to_string(),
                },
                Row {
                    name: "Languages".to_string(),
                    state: on(antifp),
                    note: "en-US,en in script and in Accept-Language".to_string(),
                },
                Row {
                    name: "Screen size".to_string(),
                    state: "off".to_string(),
                    note: "reported as-is: a pinned size would contradict the real window".to_string(),
                },
                Row {
                    name: "Cookies / third-party storage".to_string(),
                    state: "on".to_string(),
                    note: "accept policy: no third party; ITP enabled".to_string(),
                },
                Row {
                    name: "Permissions".to_string(),
                    state: "on".to_string(),
                    note: "every request denied".to_string(),
                },
                Row {
                    name: "DNS prefetch".to_string(),
                    state: "on".to_string(),
                    note: "disabled in engine settings".to_string(),
                },
                Row {
                    name: "Hyperlink auditing (a ping)".to_string(),
                    state: "on".to_string(),
                    note: "disabled in engine settings".to_string(),
                },
                Row {
                    name: "Tracker blocking".to_string(),
                    state: "partial".to_string(),
                    note: "ITP only; WebKitGTK exposes no blocked-request count".to_string(),
                },
                Row {
                    name: "Ad blocking".to_string(),
                    state: "off".to_string(),
                    note: "not implemented".to_string(),
                },
                Row {
                    name: "Session storage".to_string(),
                    state: "on".to_string(),
                    note: "ephemeral context: cookies, cache, localStorage, IndexedDB and service workers live in memory".to_string(),
                },
            ],
            permissions: vec![
                "Microphone, Camera, Geolocation, Notifications".to_string(),
                "Clipboard, MIDI, pointer lock, install of media plugins".to_string(),
                "Everything else WebKitGTK asks about".to_string(),
            ],
            limitations: vec![
                "Not anonymity and not Tor. A page that runs JavaScript can still infer things about the machine.".to_string(),
                "In-memory storage means logins do not survive a restart; that is deliberate.".to_string(),
                "Font metrics and the GPU model cannot be normalised.".to_string(),
                "No blocker statistics: the engine does not report how many requests ITP stopped.".to_string(),
            ],
        }
    }

    fn bookmarks_html(&self) -> String {
        let bm = self.bookmarks.borrow().clone();
        let mut rows = String::new();
        for b in &bm {
            rows.push_str(&format!(
                "<tr><td><a href=\"{}\">{}</a></td><td class=u>{}</td></tr>",
                b.url, b.title, b.url
            ));
        }
        if bm.is_empty() {
            rows.push_str("<tr><td colspan=2 class=empty>No bookmarks yet — hit the star in the toolbar.</td></tr>");
        }
        format!(
            "<!doctype html><html><head><meta charset='utf-8'><title>Bookmarks</title><style>\
             body{{font-family:system-ui,sans-serif;margin:40px;max-width:880px;background:#fff;color:#202124}}\
             h1{{font-weight:400;font-size:24px;margin:0 0 20px}}table{{border-collapse:collapse;width:100%;font-size:14px}}\
             td,th{{text-align:left;padding:10px 8px;border-bottom:1px solid #e8eaed}}\
             a{{color:#1a73e8;text-decoration:none}}a:hover{{text-decoration:underline}}\
             .u{{color:#5f6368;font-size:12px;font-family:monospace}}.empty{{color:#80868b}}\
             </style></head><body><h1>Bookmarks</h1>\
             <table><tr><th>Name</th><th>URL</th></tr>{}</table></body></html>",
            rows
        )
    }

    /// Mirror the open tabs into the shared session file so a crash can be
    /// recovered and a clean exit can clear it.
    fn persist_session(&self) {
        let urls: Vec<String> = self
            .tabs
            .borrow()
            .iter()
            .map(|t| t.uri.borrow().clone())
            .filter(|u| !u.is_empty())
            .collect();
        if urls.is_empty() {
            return;
        }
        let mut s = rc_core::features::SessionState::load();
        s.tabs = urls;
        s.active = self.active.get();
        s.dirty = true;
        let _ = s.save();
    }

    fn import_bookmarks_dialog(&self) {
        let dlg = gtk::FileChooserDialog::with_buttons(
            Some("Import bookmarks"),
            Some(&self.window),
            gtk::FileChooserAction::Open,
            &[
                ("Cancel", gtk::ResponseType::Cancel),
                ("Import", gtk::ResponseType::Accept),
            ],
        );
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Bookmarks (HTML or Chrome JSON)"));
        filter.add_pattern("*.html");
        filter.add_pattern("*.htm");
        filter.add_pattern("*");
        dlg.add_filter(filter);
        let a = self.rc();
        dlg.connect_response(move |d, resp| {
            if resp == gtk::ResponseType::Accept {
                if let Some(path) = d.filename() {
                    match rc_core::features::parse_bookmarks_file(&path) {
                        Ok(items) => {
                            // The GTK shell has its own Bookmark type; convert,
                            // merge with the shared code, convert back.
                            let mut merged: Vec<rc_core::store::Bookmark> = a
                                .bookmarks
                                .borrow()
                                .iter()
                                .map(|b| rc_core::store::Bookmark {
                                    title: b.title.clone(),
                                    url: b.url.clone(),
                                })
                                .collect();
                            let (added, skipped) =
                                rc_core::features::merge_bookmarks(&mut merged, items);
                            {
                                let mut bm = a.bookmarks.borrow_mut();
                                bm.clear();
                                for b in merged {
                                    bm.push(Bookmark {
                                        title: b.title,
                                        url: b.url,
                                    });
                                }
                                let _ = rc_core::store::save_bookmarks(
                                    &bm.iter()
                                        .map(|b| rc_core::store::Bookmark {
                                            title: b.title.clone(),
                                            url: b.url.clone(),
                                        })
                                        .collect::<Vec<_>>(),
                                );
                            }
                            a.refresh_bookmarks_bar();
                            a.info(&format!(
                                "Imported {added} bookmark(s), skipped {skipped} duplicate(s)."
                            ));
                        }
                        Err(e) => a.info(&format!("Import failed: {e}")),
                    }
                }
            }
            d.close();
        });
        dlg.show_all();
    }

    fn export_bookmarks(&self) {
        let dest = rc_core::downloads_dir().join("rustchrome-bookmarks.html");
        let bookmarks: Vec<rc_core::store::Bookmark> = self
            .bookmarks
            .borrow()
            .iter()
            .map(|b| rc_core::store::Bookmark {
                title: b.title.clone(),
                url: b.url.clone(),
            })
            .collect();
        let html = rc_core::features::export_netscape(&bookmarks);
        match std::fs::write(&dest, html) {
            Ok(()) => self.info(&format!("Exported bookmarks to {}", dest.display())),
            Err(e) => self.info(&format!("Export failed: {e}")),
        }
    }

    fn info(&self, text: &str) {
        let dlg = gtk::MessageDialog::new(
            Some(&self.window),
            gtk::DialogFlags::MODAL,
            gtk::MessageType::Info,
            gtk::ButtonsType::Ok,
            text,
        );
        dlg.connect_response(|d, _| d.close());
        dlg.show();
    }

    /// Ctrl+Shift+A: filter open tabs by title or address and switch to one.
    fn tab_search_dialog(&self) {
        let dlg = gtk::Dialog::with_buttons(
            Some("Search tabs"),
            Some(&self.window),
            gtk::DialogFlags::MODAL,
            &[("Close", gtk::ResponseType::Close)],
        );
        dlg.set_default_size(460, 340);
        let area = dlg.content_area();
        let entry = Entry::new();
        entry.set_placeholder_text(Some("Search tabs by title or address"));
        area.pack_start(&entry, false, false, 6);
        let list = GtkBox::new(Orientation::Vertical, 2);
        area.pack_start(&list, true, true, 0);

        let rebuild = {
            let a = self.rc();
            let list = list.clone();
            let dlg = dlg.clone();
            move |query: &str| {
                for child in list.children() {
                    list.remove(&child);
                }
                let pairs: Vec<(String, String)> = a
                    .tabs
                    .borrow()
                    .iter()
                    .map(|t| (t.title.borrow().clone(), t.uri.borrow().clone()))
                    .collect();
                for idx in rc_core::features::match_tabs(query, &pairs)
                    .into_iter()
                    .take(12)
                {
                    let (title, uri) = pairs[idx].clone();
                    let label = if title.is_empty() { uri.clone() } else { title };
                    let btn = Button::with_label(&format!("{label}  —  {uri}"));
                    btn.set_relief(gtk::ReliefStyle::None);
                    btn.style_context().add_class("tablabel");
                    let a2 = a.clone();
                    let dlg2 = dlg.clone();
                    btn.connect_clicked(move |_| {
                        if let Some(t) = a2.tabs.borrow().get(idx).cloned() {
                            a2.activate(&t);
                        }
                        dlg2.close();
                    });
                    list.pack_start(&btn, false, false, 0);
                    btn.show();
                }
            }
        };
        rebuild("");
        {
            let rebuild = rebuild.clone();
            entry.connect_changed(move |e| rebuild(&e.text()));
        }
        area.show_all();
        let rebuild = rebuild.clone();
        dlg.connect_response(move |d, _| {
            d.close();
            let _ = &rebuild;
        });
        dlg.show();
        entry.grab_focus();
    }

    fn recently_closed_dialog(&self) {
        let urls: Vec<String> = self.closed.borrow().iter().rev().cloned().collect();
        if urls.is_empty() {
            self.info("Nothing has been closed in this session.");
            return;
        }
        let dlg = gtk::Dialog::with_buttons(
            Some("Recently closed"),
            Some(&self.window),
            gtk::DialogFlags::MODAL,
            &[("Close", gtk::ResponseType::Close)],
        );
        dlg.set_default_size(520, 320);
        let area = dlg.content_area();
        let list = GtkBox::new(Orientation::Vertical, 2);
        area.pack_start(&list, true, true, 0);
        for url in urls {
            let btn = Button::with_label(&url);
            btn.set_relief(gtk::ReliefStyle::None);
            let a = self.rc();
            let dlg2 = dlg.clone();
            let u = url.clone();
            btn.connect_clicked(move |_| {
                a.new_tab(&u);
                dlg2.close();
            });
            list.pack_start(&btn, false, false, 0);
        }
        area.show_all();
        dlg.connect_response(|d, _| d.close());
        dlg.show();
    }

    fn install_userscript_dialog(&self) {
        let dlg = gtk::Dialog::with_buttons(
            Some("Install userscript"),
            Some(&self.window),
            gtk::DialogFlags::MODAL,
            &[
                ("Cancel", gtk::ResponseType::Cancel),
                ("Install", gtk::ResponseType::Ok),
            ],
        );
        let area = dlg.content_area();
        let entry = Entry::new();
        entry.set_placeholder_text(Some("https://example.com/script.user.js"));
        entry.set_width_chars(52);
        area.pack_start(&entry, false, false, 0);
        dlg.show_all();
        let a = self.rc();
        dlg.connect_response(move |dlg, resp| {
            if resp == gtk::ResponseType::Ok {
                let url = entry.text().to_string();
                if url.starts_with("http") {
                    let name = url
                        .rsplit('/')
                        .next()
                        .unwrap_or("script.js")
                        .split('?')
                        .next()
                        .unwrap_or("script.js")
                        .to_string();
                    let name = if name.ends_with(".js") {
                        name
                    } else {
                        format!("{}.js", name)
                    };
                    let dest = ext_dir().join(&name);
                    let ok = std::process::Command::new("curl")
                        .args(["-sSL", "--max-time", "20", "-o"])
                        .arg(&dest)
                        .arg(&url)
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false);
                    if ok {
                        a.install_extensions();
                        if let Some(t) = a.current_tab() {
                            t.view.reload();
                        }
                    }
                }
            }
            dlg.close();
        });
    }

    fn apply_theme(&self, dark: bool, persist: bool) {
        self.dark.set(dark);
        if dark {
            self.window.style_context().add_class("dark");
        } else {
            self.window.style_context().remove_class("dark");
        }
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_application_prefer_dark_theme(dark);
        }
        if let Some(old) = self.dark_sheet.borrow_mut().take() {
            self.ucm.remove_style_sheet(&old);
        }
        if dark && self.force_dark.get() {
            let sheet = UserStyleSheet::new(
                FORCE_DARK_CSS,
                UserContentInjectedFrames::AllFrames,
                UserStyleLevel::User,
                &[],
                &[],
            );
            self.ucm.add_style_sheet(&sheet);
            *self.dark_sheet.borrow_mut() = Some(sheet);
        }
        set_icon(
            &self.theme_btn,
            if dark {
                "weather-clear-symbolic"
            } else {
                "weather-clear-night-symbolic"
            },
        );
        self.theme_btn
            .set_tooltip_text(Some(if dark { "Light mode" } else { "Dark mode" }));

        let bg = if dark {
            gtk::gdk::RGBA::new(0.125, 0.129, 0.141, 1.0)
        } else {
            gtk::gdk::RGBA::new(1.0, 1.0, 1.0, 1.0)
        };
        for t in self.tabs.borrow().iter() {
            t.view.set_background_color(&bg);
        }
        if persist {
            save_dark(dark);
        }
    }

    /// Every .js file in the extensions folder is injected into every page,
    /// the same idea as a userscript manager (Tampermonkey), which is the only
    /// extension model WebKitGTK supports.
    fn install_extensions(&self) -> Vec<String> {
        let mut names = Vec::new();
        for path in ext_files() {
            let Ok(code) = std::fs::read_to_string(&path) else {
                continue;
            };
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "extension".into());
            let script = webkit2gtk::UserScript::new(
                &code,
                UserContentInjectedFrames::AllFrames,
                webkit2gtk::UserScriptInjectionTime::Start,
                &[],
                &[],
            );
            self.ucm.add_script(&script);
            names.push(name);
        }
        names
    }

    fn new_identity(&self) {
        // New identity = new canvas key as well as fresh storage.
        self.seed.set(SessionSeed::new());
        self.sync_fp_script();
        let a = self.rc();
        clear_session_data(move || {
            let tabs: Vec<Rc<Tab>> = a.tabs.borrow().clone();
            for t in tabs {
                a.close_tab(&t);
            }
            a.closed.borrow_mut().clear();
            if let Some(t) = a.current_tab() {
                t.view.reload();
            }
        });
    }

    fn sync_fp_script(&self) {
        if let Some(old) = self.fp_script.borrow_mut().take() {
            self.ucm.remove_script(&old);
        }
        if self.antifp.get() {
            let payload = antifp_js(self.seed.get(), self.strict.get());
            let script = webkit2gtk::UserScript::new(
                &payload,
                UserContentInjectedFrames::AllFrames,
                webkit2gtk::UserScriptInjectionTime::Start,
                &[],
                &[],
            );
            self.ucm.add_script(&script);
            *self.fp_script.borrow_mut() = Some(script);
        }
    }

    fn apply_fingerprint_mode(&self) {
        let on = self.antifp.get();
        let strict = self.strict.get();
        for t in self.tabs.borrow().iter() {
            if let Some(s) = WebViewExt::settings(&t.view) {
                // Engine-level switches only exist as an on/off pair, so they
                // are used only in strict mode; standard mode coerces from JS.
                s.set_enable_webgl(!(on && strict));
                s.set_enable_webrtc(!(on && strict));
                s.set_enable_media_stream(!on);
            }
            t.view.reload();
        }
        self.sync_fp_script();
    }

    fn sync_nav(&self) {
        if let Some(t) = self.current_tab() {
            self.back.set_sensitive(t.view.can_go_back());
            self.forward.set_sensitive(t.view.can_go_forward());
            let loading = t.view.is_loading();
            set_icon(
                &self.reload,
                if loading {
                    "process-stop-symbolic"
                } else {
                    "view-refresh-symbolic"
                },
            );
            self.reload
                .set_tooltip_text(Some(if loading { "Stop loading" } else { "Reload" }));
            if let Some(img) = self.reload.image() {
                if let Ok(img) = img.downcast::<Image>() {
                    img.set_pixel_size(((16.0 * self.ui_zoom.get()).round() as i32).clamp(10, 96));
                }
            }
            self.entry
                .set_primary_icon_name(Some(if t.uri.borrow().starts_with("https://") {
                    "channel-secure-symbolic"
                } else {
                    "system-search-symbolic"
                }));
        }
    }

    fn sync_star(&self) {
        if let Some(t) = self.current_tab() {
            let url = t.uri.borrow().clone();
            let marked = !url.is_empty() && self.bookmarks.borrow().iter().any(|b| b.url == url);
            set_icon(
                &self.star,
                if marked {
                    "starred-symbolic"
                } else {
                    "non-starred-symbolic"
                },
            );
            self.star.set_tooltip_text(Some(if marked {
                "Remove bookmark"
            } else {
                "Bookmark this tab"
            }));
        }
    }

    fn toggle_bookmark(&self) {
        let Some(t) = self.current_tab() else { return };
        let url = t.uri.borrow().clone();
        if url.is_empty() {
            return;
        }
        let title = {
            let ti = t.title.borrow().clone();
            if ti.is_empty() {
                url.clone()
            } else {
                ti
            }
        };
        {
            let mut bm = self.bookmarks.borrow_mut();
            if let Some(pos) = bm.iter().position(|b| b.url == url) {
                bm.remove(pos);
            } else {
                bm.push(Bookmark { title, url });
            }
        }
        save_bookmarks(&self.bookmarks.borrow());
        self.refresh_bookmarks_bar();
        if let Some(t) = self.current_tab() {
            if t.uri.borrow().as_str() == BOOKMARKS_PAGE {
                t.view.load_html(&self.bookmarks_html(), None);
            }
        }
        self.sync_star();
    }

    fn refresh_bookmarks_bar(&self) {
        for child in self.bookmarksbar.children() {
            self.bookmarksbar.remove(&child);
        }
        let items: Vec<Bookmark> = self.bookmarks.borrow().clone();
        for b in items.iter().take(14) {
            let btn = Button::new();
            btn.set_relief(gtk::ReliefStyle::None);
            btn.set_tooltip_text(Some(&b.url));
            let row = GtkBox::new(Orientation::Horizontal, 6);
            let icon = Image::from_icon_name(Some("folder-symbolic"), gtk::IconSize::Menu);
            icon.set_pixel_size(
                ((14.0 * self.ui_zoom.get().max(1.0)).round() as i32).clamp(10, 40),
            );
            row.pack_start(&icon, false, false, 0);
            row.pack_start(&Label::new(Some(&b.title)), false, false, 0);
            btn.add(&row);
            let a = self.rc();
            let url = b.url.clone();
            btn.connect_clicked(move |_| {
                if let Some(t) = a.current_tab() {
                    a.navigate(&t, &url);
                }
            });
            self.bookmarksbar.pack_start(&btn, false, false, 0);
            btn.show_all();
        }
        self.bookmarksbar.set_visible(!items.is_empty());
    }

    fn close_tab(&self, tab: &Rc<Tab>) {
        let idx = self.index_of(tab);
        if idx == usize::MAX {
            return;
        }
        let uri = tab.uri.borrow().clone();
        if !uri.is_empty() {
            let mut c = self.closed.borrow_mut();
            c.push(uri);
            if c.len() > 10 {
                c.remove(0);
            }
        }
        self.stack.remove(&tab.view);
        self.tabs_box.remove(&tab.tab_box);
        self.tabs.borrow_mut().remove(idx);
        let empty = self.tabs.borrow().is_empty();
        if empty {
            self.new_tab(NEW_TAB_PAGE);
            return;
        }
        let next = idx.min(self.tabs.borrow().len() - 1);
        let t = self.tabs.borrow()[next].clone();
        self.activate(&t);
    }

    fn close_active_tab(&self) {
        if let Some(t) = self.current_tab() {
            self.close_tab(&t);
        }
    }

    fn reopen_closed(&self) {
        let url = self.closed.borrow_mut().pop();
        if let Some(u) = url {
            self.new_tab(&u);
        }
    }

    fn cycle_tab(&self, delta: i32) {
        let n = self.tabs.borrow().len() as i32;
        if n < 2 {
            return;
        }
        let mut idx = self.active.get() as i32 + delta;
        if idx < 0 {
            idx = n - 1;
        }
        if idx >= n {
            idx = 0;
        }
        let t = self.tabs.borrow()[idx as usize].clone();
        self.activate(&t);
    }

    fn show_find(&self) {
        self.findbar.show_all();
        self.findentry.grab_focus();
        self.findentry.select_region(0, -1);
    }

    fn hide_find(&self) {
        if let Some(t) = self.current_tab() {
            if let Some(fc) = t.view.find_controller() {
                fc.search_finish();
            }
        }
        self.findbar.hide();
        if let Some(t) = self.current_tab() {
            t.view.grab_focus();
        }
    }

    fn zoom_by(&self, delta: f64) {
        let base = self
            .current_tab()
            .map(|t| t.zoom.get())
            .unwrap_or(self.zoom.get());
        self.set_zoom(base + delta);
    }

    fn zoom_reset(&self) {
        self.set_zoom(1.0);
    }

    fn set_zoom(&self, z: f64) {
        let z = z.clamp(0.25, 5.0);
        self.zoom.set(z);
        if let Some(t) = self.current_tab() {
            t.zoom.set(z);
            t.view.set_zoom_level(z);
        }
        self.apply_ui_zoom(z);
        set_pref_f64("zoom", z);
    }

    /// Chrome itself (tabs, toolbar, address bar) follows the content zoom.
    fn apply_ui_zoom(&self, scale: f64) {
        // switching tabs must not rebuild the stylesheet if the scale is unchanged
        if (self.ui_zoom.get() - scale).abs() < 0.001 && self.ui_zoom.get() > 0.0 {
            return;
        }
        self.ui_zoom.set(scale);
        let provider = CssProvider::new();
        let css = scale_css(CSS, scale);
        if let Err(e) = provider.load_from_data(css.as_bytes()) {
            eprintln!("css error: {e}");
        }
        if let Some(screen) =
            gtk::prelude::WidgetExt::screen(&self.window).or_else(gtk::gdk::Screen::default)
        {
            // drop the previous zoomed provider so they cannot pile up
            {
                let old = self.css.borrow();
                gtk::StyleContext::remove_provider_for_screen(&screen, &*old);
            }
            gtk::StyleContext::add_provider_for_screen(
                &screen,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        *self.css.borrow_mut() = provider;
        self.apply_icon_zoom();
    }

    fn apply_icon_zoom(&self) {
        let px = ((16.0 * self.ui_zoom.get()).round() as i32).clamp(10, 96);
        scale_images(self.window.upcast_ref::<gtk::Widget>(), px);
        for b in &self.icon_btns {
            if let Some(img) = b.image() {
                if let Ok(img) = img.downcast::<Image>() {
                    img.set_pixel_size(px);
                }
            }
        }
    }

    fn setup_downloads(&self) {
        let Some(ctx) = WebContext::default() else {
            return;
        };
        ctx.set_cache_model(if std::env::var("RUSTCHROME_LEAN").is_ok() {
            webkit2gtk::CacheModel::DocumentViewer
        } else {
            webkit2gtk::CacheModel::WebBrowser
        });
        let win = self.window.clone();
        ctx.connect_download_started(move |_ctx, download| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            let downloads = PathBuf::from(&home).join("Downloads");
            let _ = std::fs::create_dir_all(&downloads);
            let name = download
                .request()
                .and_then(|r| r.uri())
                .map(|u| u.to_string())
                .and_then(|u| u.rsplit('/').next().map(|s| s.to_string()))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "download".into());
            let mut dest = downloads.join(&name);
            let mut n = 1;
            while dest.exists() {
                let p = PathBuf::from(&name);
                let stem = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "file".into());
                let ext = p
                    .extension()
                    .map(|s| format!(".{}", s.to_string_lossy()))
                    .unwrap_or_default();
                dest = downloads.join(format!("{} ({}){}", stem, n, ext));
                n += 1;
            }
            download.set_allow_overwrite(true);
            download.set_destination(&dest.to_string_lossy());

            let win2 = win.clone();
            download.connect_finished(move |d| {
                let dest = d.destination().map(|s| s.to_string()).unwrap_or_default();
                let dialog = gtk::MessageDialog::new(
                    Some(&win2),
                    gtk::DialogFlags::MODAL,
                    gtk::MessageType::Info,
                    gtk::ButtonsType::Ok,
                    &format!("Downloaded to {}", dest),
                );
                dialog.connect_response(|d, _| d.close());
                dialog.show();
            });
        });
    }

    fn show_menu(&self, btn: &Button) {
        let menu = gtk::Menu::new();
        let zoom_pct = (self.zoom.get() * 100.0).round() as i64;

        let add = |label: &str| -> gtk::MenuItem { gtk::MenuItem::with_label(label) };

        let mi_new = add("New tab");
        {
            let a = self.rc();
            mi_new.connect_activate(move |_| {
                a.new_tab(NEW_TAB_PAGE);
            });
        }
        let mi_reopen = add("Reopen closed tab");
        {
            let a = self.rc();
            mi_reopen.connect_activate(move |_| a.reopen_closed());
        }
        let mi_find = add("Find in page…");
        {
            let a = self.rc();
            mi_find.connect_activate(move |_| a.show_find());
        }
        let mi_zin = add("Zoom in");
        {
            let a = self.rc();
            mi_zin.connect_activate(move |_| a.zoom_by(0.1));
        }
        let mi_zout = add("Zoom out");
        {
            let a = self.rc();
            mi_zout.connect_activate(move |_| a.zoom_by(-0.1));
        }
        let mi_zres = add(&format!("Reset zoom  ({}%)", zoom_pct));
        {
            let a = self.rc();
            mi_zres.connect_activate(move |_| a.zoom_reset());
        }
        let mi_dl = add("Downloads folder");
        mi_dl.connect_activate(move |_| {
            let home = std::env::var("HOME").unwrap_or_default();
            let _ = std::process::Command::new("xdg-open")
                .arg(format!("{}/Downloads", home))
                .spawn();
        });
        let mi_bm = add("Bookmarks");
        {
            let a = self.rc();
            mi_bm.connect_activate(move |_| {
                if let Some(t) = a.current_tab() {
                    a.navigate(&t, BOOKMARKS_PAGE);
                }
            });
        }
        let mi_ext = add("Extensions");
        {
            let a = self.rc();
            mi_ext.connect_activate(move |_| {
                if let Some(t) = a.current_tab() {
                    a.navigate(&t, EXT_PAGE);
                }
            });
        }
        let mi_ext_add = add("Install userscript from URL…");
        {
            let a = self.rc();
            mi_ext_add.connect_activate(move |_| a.install_userscript_dialog());
        }
        let mi_privacy = add("Privacy");
        {
            let a = self.rc();
            mi_privacy.connect_activate(move |_| {
                if let Some(t) = a.current_tab() {
                    a.navigate(&t, PRIVACY_PAGE);
                }
            });
        }
        let mi_import = add("Import bookmarks…");
        {
            let a = self.rc();
            mi_import.connect_activate(move |_| a.import_bookmarks_dialog());
        }
        let mi_export = add("Export bookmarks");
        {
            let a = self.rc();
            mi_export.connect_activate(move |_| a.export_bookmarks());
        }
        let mi_tabs = add("Search tabs…  (Ctrl+Shift+A)");
        {
            let a = self.rc();
            mi_tabs.connect_activate(move |_| a.tab_search_dialog());
        }
        let mi_closed = add("Recently closed…");
        {
            let a = self.rc();
            mi_closed.connect_activate(move |_| a.recently_closed_dialog());
        }
        let mi_strict = add(if self.strict.get() {
            "Strict mode: on"
        } else {
            "Strict mode: off"
        });
        {
            let a = self.rc();
            mi_strict.connect_activate(move |_| {
                let now = !a.strict.get();
                a.strict.set(now);
                set_pref("strict", now);
                a.apply_fingerprint_mode();
            });
        }
        let mi_newid = add("New identity  (clear cookies + history)");
        {
            let a = self.rc();
            mi_newid.connect_activate(move |_| a.new_identity());
        }
        let mi_autoclear = add(if get_pref("autoclear").unwrap_or(true) {
            "Clear data on exit: on"
        } else {
            "Clear data on exit: off"
        });
        mi_autoclear.connect_activate(|_| {
            let now = !get_pref("autoclear").unwrap_or(true);
            set_pref("autoclear", now);
        });
        let mi_fp = add(if self.antifp.get() {
            "Anti-fingerprinting: on"
        } else {
            "Anti-fingerprinting: off"
        });
        {
            let a = self.rc();
            mi_fp.connect_activate(move |_| {
                let now = !a.antifp.get();
                a.antifp.set(now);
                set_pref("antifp", now);
                a.apply_fingerprint_mode();
            });
        }
        let mi_force = add(if self.force_dark.get() {
            "Force dark pages: on"
        } else {
            "Force dark pages: off"
        });
        {
            let a = self.rc();
            mi_force.connect_activate(move |_| {
                let now = !a.force_dark.get();
                a.force_dark.set(now);
                save_force_dark(now);
                let d = a.dark.get();
                a.apply_theme(d, false);
            });
        }
        let mi_theme = add(if self.dark.get() {
            "Light mode"
        } else {
            "Dark mode"
        });
        {
            let a = self.rc();
            mi_theme.connect_activate(move |_| {
                let d = !a.dark.get();
                a.apply_theme(d, true);
            });
        }
        let mi_quit = add("Quit");
        mi_quit.connect_activate(move |_| std::process::exit(0));

        menu.append(&mi_new);
        menu.append(&mi_reopen);
        menu.append(&gtk::SeparatorMenuItem::new());
        menu.append(&mi_find);
        menu.append(&gtk::SeparatorMenuItem::new());
        menu.append(&mi_zin);
        menu.append(&mi_zout);
        menu.append(&mi_zres);
        menu.append(&gtk::SeparatorMenuItem::new());
        menu.append(&mi_dl);
        menu.append(&mi_bm);
        menu.append(&mi_ext);
        menu.append(&mi_ext_add);
        menu.append(&mi_privacy);
        menu.append(&mi_tabs);
        menu.append(&mi_closed);
        menu.append(&gtk::SeparatorMenuItem::new());
        menu.append(&mi_import);
        menu.append(&mi_export);
        menu.append(&gtk::SeparatorMenuItem::new());
        menu.append(&mi_newid);
        menu.append(&mi_autoclear);
        menu.append(&gtk::SeparatorMenuItem::new());
        menu.append(&mi_fp);
        menu.append(&mi_strict);
        menu.append(&mi_force);
        menu.append(&mi_theme);
        menu.append(&gtk::SeparatorMenuItem::new());
        menu.append(&mi_quit);
        menu.show_all();
        menu.popup_easy(gtk::gdk::BUTTON_PRIMARY, gtk::current_event_time());
        let _ = btn;
    }
}

/// Wipes cookies, caches, localStorage, IndexedDB, sessions — everything a
/// session leaves behind — then runs `then` on the GTK main loop.
fn clear_session_data(then: impl FnOnce() + 'static) {
    use gtk::glib::object::ObjectType;
    let Some(ctx) = WebContext::default() else {
        then();
        return;
    };
    ctx.clear_cache();
    let Some(dm) = ctx.website_data_manager() else {
        then();
        return;
    };

    let boxed: Box<Box<dyn FnOnce()>> = Box::new(Box::new(then));
    unsafe {
        webkit2gtk_sys::webkit_website_data_manager_clear(
            dm.as_ptr() as *mut _,
            webkit2gtk::WebsiteDataTypes::ALL.bits(),
            0,
            std::ptr::null_mut(),
            Some(on_data_cleared),
            Box::into_raw(boxed) as *mut _,
        );
    }
}

unsafe extern "C" fn on_data_cleared(
    src: *mut gtk::glib::gobject_ffi::GObject,
    res: *mut gtk::gio::ffi::GAsyncResult,
    ud: *mut std::ffi::c_void,
) {
    let mut err: *mut gtk::glib::ffi::GError = std::ptr::null_mut();
    webkit2gtk_sys::webkit_website_data_manager_clear_finish(src as *mut _, res, &mut err);
    let cb = *Box::from_raw(ud as *mut Box<dyn FnOnce()>);
    cb();
}

fn ext_dir() -> PathBuf {
    let d = data_dir().join("extensions");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn ext_files() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(ext_dir())
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map(|x| x == "js").unwrap_or(false))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn load_bookmarks() -> Vec<Bookmark> {
    std::fs::read_to_string(data_dir().join("bookmarks.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<Bookmark>>(&s).ok())
        .unwrap_or_default()
}

fn save_bookmarks(bm: &[Bookmark]) {
    if let Ok(s) = serde_json::to_string(bm) {
        let _ = std::fs::write(data_dir().join("bookmarks.json"), s);
    }
}

fn main() {
    let _ = START.set(std::time::Instant::now());
    {
        let path = data_dir().join("crash.log");
        std::panic::set_hook(Box::new(move |info| {
            let msg = format!("[panic] {}\n", info);
            eprintln!("{msg}");
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                use std::io::Write;
                let _ = f.write_all(msg.as_bytes());
            }
        }));
    }
    let application = Application::builder()
        .application_id("com.nous.RustChrome")
        .build();
    application.connect_startup(|_| {
        // WebKitGTK's overlay scrollbars keep repainting scrollable pages forever
        // (measured ~19% CPU idle). Permanent scrollbars are free and drop idle to ~0%.
        if let Some(settings) = gtk::Settings::default() {
            settings.set_property("gtk-overlay-scrolling", false);
        }
    });
    application.connect_activate(|app| {
        let _me = App::build(app);
    });
    application.run();
}

#[cfg(test)]
mod shell_tests {
    use super::*;

    /// The stylesheet is one embedded string. If a selector disappears, a control
    /// silently loses its styling; this catches that without a display.
    #[test]
    fn stylesheet_still_styles_the_controls_we_ship() {
        for selector in [".tab", "omnibox", "#progress", ".closetab", "headerbar"] {
            assert!(CSS.contains(selector), "stylesheet lost `{selector}`");
        }
    }

    /// The stylesheet must be parseable GTK CSS. Skipped when there is no display
    /// server, which is why CI runs the tests under `xvfb-run`.
    #[test]
    fn stylesheet_parses_when_a_display_exists() {
        if gtk::init().is_err() {
            eprintln!("no display server; skipping the GTK CSS parse check");
            return;
        }
        let provider = gtk::CssProvider::new();
        if let Err(e) = provider.load_from_data(CSS.as_bytes()) {
            panic!("embedded stylesheet does not parse: {e}");
        }
    }

    /// The shell addresses preferences by UI name ("dark"), the store by field
    /// name ("theme"). That mapping is exactly what used to be done by hand, and
    /// what destroyed settings when it was done wrong.
    #[test]
    fn shell_pref_names_map_to_typed_fields() {
        let p = rc_core::store::Prefs::load();
        assert_eq!(get_pref("dark"), Some(p.is_dark()));
        assert_eq!(get_pref("antifp"), Some(p.antifp));
        assert_eq!(get_pref("no-such-setting"), None);
    }

    /// Writing one setting through the shell must leave the rest untouched.
    #[test]
    fn shell_writes_preserve_unrelated_settings() {
        let before = rc_core::store::Prefs::load();
        set_pref("autoclear", !before.autoclear);

        let after = rc_core::store::Prefs::load();
        assert_eq!(after.autoclear, !before.autoclear, "the setting did not change");
        assert_eq!(after.zoom, before.zoom, "zoom was disturbed");
        assert_eq!(after.antifp, before.antifp, "anti-fingerprinting was disturbed");
        assert_eq!(after.engine, before.engine, "the search engine was disturbed");

        set_pref("autoclear", before.autoclear);
    }
}
