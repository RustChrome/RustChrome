# Project Status

Last verified: 2026-11-27 (HEAD `d9f2923`, branch `main`; working tree now has
the 0.2.3 changes staged locally on top).

This file records the *verified* state of the repo on each steward session. It is
updated when reality changes, not when opinions do.

## Version and layout

* In progress: **0.2.3** (themes). Version bumped in the workspace and both
  crates; the theme feature is implemented on both shells.
* Workspace: root package is the Linux shell; members `crates/rc-core`,
  `crates/rc-windows`.
* `src/main.rs` — Linux shell (GTK3 + WebKitGTK).
* `crates/rc-windows/` — Windows shell (Win32 + WebView2).
* `crates/rc-core/` — shared core, no GUI deps, 45 tests.

## Privacy posture (as documented and observed)

One generic machine fingerprint; per-session canvas noise; UA matches the real
engine; WebGL/WebRTC neutralised in Strict mode; all permission requests denied
by default; ephemeral / InPrivate contexts; no telemetry, accounts, or sync.
The project correctly documents this is not anonymity and does not hide IPs.

## What is verified working

* Both shells build and release (Linux tarball, Windows ZIP with static CRT).
* Privacy dashboard rendered only in `rc-core`.
* `rc-core` typed, single-writer `prefs.json` store.
* **45 rc-core tests pass** — including the new theme suite (palette integrity,
  accent contrast, historic-colour preservation, Windows chrome variable
  mapping).
* CI runs fmt / clippy / test / audit / build on real Linux and Windows
  runners; the new GTK per-theme CSS parse test runs under `xvfb` there.

## 0.2.3 — themes (implemented; shell wiring needs real-runner verify)

* Shared theme registry in `rc-core/src/themes.rs`: Light, Dark (both
  byte-identical to 0.2.2), Graphite, Sepia, Midnight, Nord. One semantic
  `Palette` (window/chrome/surfaces/text/accent/hairlines/new-tab page).
* Linux shell: theme-agnostic GTK stylesheet over `@define-color rc_*`; the ⋯
  menu has a *Themes* submenu, the toolbar button cycles; new-tab page and tab
  backgrounds use the palette. `save_dark`/`.dark`-class plumbing removed.
* Windows shell: `chrome.rs` injects every theme into `chrome.html` at startup
  as `html[data-theme]` blocks; the page sets `data-theme` from the persisted
  id; the hardcoded `html.dark` block and `.dark` toast rule removed; new-tab
  and error pages take the palette. Theme menu item cycles with a label.
* **Verified here:** `rc-core` (45 tests, clippy clean). **Not compiled here:**
  the Linux shell (this sandbox lacks `webkit2gtk-4.0`; only 4.1 is present) and
  the Windows shell (needs a Windows host). Both are exercised by the existing
  qa.yml real-runner gates before the release is cut.

## Known gaps (open as of this session)

High priority:

1. **Linux does not enforce per-site permission rules** — still unaddressed.
2. **Linux has no external-protocol policy** — still unaddressed.
3. **Linux downloads have no manager UI** — still unaddressed.
4. **No interactive test harness** — the largest testing hole.
5. **Windows cold startup ~7.8 s / memory 300–470 MB** — still open measurement.

Medium / hygiene:

6. Three GitHub issues (#2/#3/#4) are merged epics that were never closed.
7. Committed release artifacts under `rel2/…rel9/` bloat history.
8. `wire_events` in `rc-windows/src/app.rs` remains ~150 lines.

## Non-goals (deliberate)

Accounts/sync/telemetry; an extension ecosystem; an unaudited password manager;
merging the two shells into one abstraction.