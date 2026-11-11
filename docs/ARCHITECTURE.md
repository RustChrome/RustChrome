# Architecture

Two browser shells over one shared core. The point of the split is that browser
behaviour is written and tested once, and each platform only has to render it.

```
                 crates/rc-core          (no GUI dependencies, 37 tests)
        omnibox rules · prefs store · bookmarks io · session records
        privacy payloads · tab-search ranking · site rules · dashboard
                    │                              │
        ┌───────────┴───────────┐      ┌───────────┴────────────┐
        │   src/main.rs         │      │  crates/rc-windows     │
        │   GTK3 + WebKitGTK    │      │  Win32 + WebView2      │
        └───────────────────────┘      └────────────────────────┘
```

## Why two shells instead of one abstraction

A `Browser` trait covering both GTK and Win32 ends up with the lowest common
denominator of both widget toolkits and a pile of `#[cfg]` inside it. The GTK
shell is 2,400 lines of signal wiring that has no meaning on Windows, and the
Windows shell talks to COM callbacks that have no meaning on Linux. What is
genuinely shared — preferences, storage, parsing, ranking, privacy payloads,
the dashboard — already lives in `rc-core` and is tested there.

## Modules

| `lib.rs` | navigation allow-list, omnibox parsing, search engines, data directories |
| `store.rs` | atomic JSON stores: prefs, bookmarks, userscripts |
| `features.rs` | bookmark import (Chrome JSON, Netscape HTML), export, session records, tab search, site rules, custom engines |
| `privacy.rs` | the injected payloads and the probe script, shared by both engines |
| `dashboard.rs` | the one renderer for the privacy dashboard |
| `crates/rc-windows/src/main.rs` | process entry, DPI awareness, single-instance mutex |
| `window.rs` | Win32 window, work-area clamping, DWM frame theming |
| `webview.rs` | WebView2 environment/controller lifecycle and failure reporting |
| `app.rs` | tabs, navigation, downloads, permissions, layout |
| `chrome.rs` | the HTML UI bridge: message types, load, post |
| `privacy.rs` | WebView2 settings that WebKitGTK exposes differently |
| `downloads.rs` | destination policy, name sanitising, collision handling |
| `pages.rs` | new-tab and error pages |
| `assets/chrome.html` | the tab strip, toolbar, menus and panels as HTML/CSS |

## Threading and ownership

**Linux.** Single-threaded GTK main loop. Every WebView lives in a `GtkStack`;
per-tab state is `Rc<RefCell<…>>`, so a signal handler that runs while another
handler holds a borrow would panic — which is why handlers take short borrows and
never call back into the widget tree while holding one.

**Windows.** WebView2 is COM and apartment-threaded. Environments and controllers
are created on the UI thread and used only from it. `App` is a thread-local
`Rc<RefCell<App>>`; the message bridge calls into it through `with_app`, which
counts a dropped callback instead of silently discarding it (`DROPPED_MESSAGES`).
Controllers live beside the webview they belong to, and `SetBounds` failures are
counted (`BOUNDS_FAILURES`) because for two days that failure looked exactly like
success.

## Data on disk

`%LOCALAPPDATA%\RustChrome` or `~/.local/share/rustchrome`:

| `prefs.json` | typed `Prefs` struct, round-trips unknown keys |
| `bookmarks.json` | flat list |
| `session.json` | tabs, active index, `dirty` flag for crash detection |
| `sites.json` | per-site permission rules |
| `userscripts/` | `.js` files injected at document start |
| `ui/chrome.html` | the browser UI, written from the binary at startup |

Every write goes through `write_atomic` (temp file + rename). A corrupt file
falls back to defaults rather than stopping the browser.

## Invariants worth knowing

1. `prefs.json` has one writer. Two writers with different schemas is how the
   Linux shell used to erase a user's settings on a theme toggle.
2. The privacy dashboard is rendered by `rc-core`, never re-implemented per shell,
   so it cannot drift into two different sets of claims.
3. The UI webview never navigates to anything but our own document; the content
   webviews pass every navigation through `rc_core::is_allowed_navigation`.
4. Nothing in `rc-core` knows what a window is, which is why it is testable in CI
   without a display server.
