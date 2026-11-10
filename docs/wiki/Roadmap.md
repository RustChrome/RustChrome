# Roadmap

Canonical version: [`ROADMAP.md`](https://github.com/RustChrome/RustChrome/blob/main/ROADMAP.md).

## Shipped

* **0.1.0** — Linux browser: tabs, omnibox, bookmarks, userscripts, privacy basics.
* **0.2.0 Beta** — Windows shell (Win32 + WebView2), privacy hardening on both
  engines, privacy dashboard, everyday features (bookmark import/export, crash
  restore, tab search, session modes, site rules), QA gates and a release pipeline.

## Next

1. Fix the Linux gaps from the QA report: enforce per-site permission rules, add a
   download manager, add a settings surface.
2. **Interactive test harness** — nothing clicks the UI today; this is the biggest
   hole in the testing.
3. Windows startup time (~7.8 s cold on CI hardware is not acceptable).
4. Memory: 300–470 MB needs to come down before the word "lightweight" is used.
5. Ad and tracker blocking, only if an engine exposes a real request-level API.

## Later

`.deb`/`.rpm`/Flatpak, macOS (WKWebView), optional Tor transport via Arti (off by
default, never described as anonymity), Android.

## Not planned

Accounts, sync servers, telemetry, crash-reporting services, a Web Store-compatible
extension ecosystem, a password manager before the storage design is audited.
