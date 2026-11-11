# Roadmap

What is done, what is next, and what is deliberately not planned. Nothing here is
a promise with a date on it.

## Shipped

**0.1.0** — Linux. GTK3 + WebKitGTK, tabs, omnibox, bookmarks, downloads,
userscripts, light/dark themes, anti-fingerprinting, ephemeral session, New
Identity, no telemetry.

**0.2.0 (first public beta)**

* Windows shell: Win32 + WebView2, native window, tabs, downloads, find in page,
  zoom, themes, portable ZIP with a static C runtime.
* Privacy hardening on both engines: consistent user agent, per-session canvas
  noise, WebGL/WebRTC neutralised in Strict mode, DNS prefetch and pings off,
  third-party cookies and ITP, InPrivate on Windows, ephemeral context on Linux.
* Privacy dashboard on both platforms, plus a reproducible probe harness that
  runs on every CI push.
* Everyday features: Chrome and Firefox bookmark import, HTML export, session
  restore after a crash, Private vs Standard session, Ctrl+Shift+A tab search,
  recently closed tabs, custom search engines, per-site permission rules,
  cancellable downloads.
* QA: format, clippy, tests, `cargo audit`, memory/CPU/startup measurement and a
  real-website compatibility run, on real Linux and Windows runners.

## Audit-driven work (after 0.2.0)

Fixed: the Linux preference store rewrote `prefs.json` and destroyed settings on a
theme toggle; dropped UI callbacks and failed `SetBounds` calls were silent; the
COM modules had no documented unsafe contract.

Still open from the same audit: the four functions over 100 lines (split one at a
time, with a compile cycle each), the GTK shell's missing tests, and the external
protocol policy that exists on Windows but not on Linux.

## Next, in rough order

1. **Fix the Linux gaps the QA report found** — per-site permission rules are
   stored but not enforced there, and downloads have no manager.
2. **Interactive test harness.** Nothing currently clicks a menu; a broken menu
   item would ship. This is the largest hole in the current testing.
3. **Windows startup time.** ~7.8 s cold on CI hardware is not acceptable; most
   of it is WebView2 initialisation and needs measuring properly on real
   hardware before anyone optimises it.
4. **Memory.** 300–470 MB is a browser, not a lightweight browser. The next
   release should beat it with evidence, not adjectives.
5. **Ad and tracker blocking**, only if an engine actually exposes a
   request-level API. No cosmetic shields that block nothing.

## Later, if it survives

* Additional Linux packaging: `.deb`, `.rpm`, Flatpak.
* macOS (WKWebView) — a fourth shell against the same core.
* Android — a different browser, not a port.
* Optional Tor transport via Arti. Optional, off by default, and never described
  as anonymity.

## Not planned

* Accounts, sync servers, telemetry, crash reporting services.
* An extension ecosystem with Web Store compatibility.
* A built-in password manager before the storage design is audited.
* Rewriting the Linux shell into the Windows one or vice versa. Two honest
  shells over one core beats one abstraction that fits neither.
