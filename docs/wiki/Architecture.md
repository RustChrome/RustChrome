# Architecture

Canonical: [`docs/ARCHITECTURE.md`](https://github.com/RustChrome/RustChrome/blob/main/docs/ARCHITECTURE.md).

Two shells over one shared core:

* **`crates/rc-core`** — no GUI dependencies: omnibox rules, the preference store,
  bookmarks, session records, privacy payloads, tab-search ranking, site rules, the
  dashboard renderer. 37 tests, runs in CI without a display server.
* **`src/main.rs`** — the GTK3 + WebKitGTK shell.
* **`crates/rc-windows`** — the Win32 + WebView2 shell, including an HTML UI in its
  own WebView2 that talks to Rust over JSON messages.

Not one abstraction over both toolkits: that ends in the lowest common denominator
and `#[cfg]` everywhere. The behaviour that can be shared is shared and tested once;
the parts that cannot are each honest about their platform.

## Threading

Linux is a single GTK main loop with `Rc<RefCell<…>>` per tab. Windows is COM:
environments, controllers and callbacks live on the UI thread; `App` is a
thread-local `Rc<RefCell<App>>` reached through `with_app`, which counts a callback
it could not deliver instead of dropping it silently.

## Invariants

1. `prefs.json` has exactly one writer.
2. The privacy dashboard is rendered by `rc-core`, never per shell, so its claims
   cannot diverge.
3. The UI webview only ever loads the document we write ourselves; content webviews
   pass every navigation through `is_allowed_navigation`.
4. Nothing in `rc-core` knows what a window is — which is why it is testable.
