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
  runners; **all four jobs (CI Linux, CI Windows, QA Linux, QA Windows) pass on
  `main` (2026-11-27)**. The interactive harness (`clicking_buttons...`) and the
  per-theme GTK stylesheet parse test both ran and passed under xvfb on the QA
  runner. `rc-core` suite passes on both platforms (45 tests).

## 0.2.3 — themes (DONE and verified on real runners)

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
* **Verified on both real runners** (fmt, clippy `-D warnings`, tests, build).

## 0.2.3 — Linux permission-rule enforcement (done and verified)

`connect_permission_request` consults `rc_core::features::site_decision`, mapping
WebKitGTK request types to the same kind names Windows uses. Deny-by-default
preserved. Passed QA Linux with clippy `-D warnings`.

## 0.2.3 — interactive test harness (done and verified)

`shell_tests::clicking_buttons_changes_state_when_a_display_exists` builds the
real App under xvfb, clicks the theme/+/⋯ buttons and asserts state changes.
Passed on the QA Linux runner.

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