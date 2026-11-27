# Project Status

Last verified: 2026-11-27 (HEAD `d9f2923`, branch `main`, working tree clean).

This file records the *verified* state of the repo on each steward session. It is
updated when reality changes, not when opinions do.

## Version and layout

* Latest release: **0.2.2** (main; `release/0.2.2` merged via PR #10).
* Workspace: root package is the Linux shell; members `crates/rc-core`,
  `crates/rc-windows`.
* `src/main.rs` — Linux shell, 2,498 lines (GTK3 + WebKitGTK).
* `crates/rc-windows/` — Windows shell (Win32 + WebView2), `app.rs` is 2,366 lines.
* `crates/rc-core/` — shared core, no GUI deps, 40 `#[test]` cases.

## Privacy posture (as documented and observed)

One generic machine fingerprint; per-session canvas noise (pure function of
pixel + per-session seed); UA matches the real engine; WebGL/WebRTC neutralised
in Strict mode; all permission requests denied by default; ephemeral / InPrivate
contexts; no telemetry, accounts, or sync. The project correctly documents that
this is not anonymity and does not hide IPs.

## What is verified working

* Both shells build and release (Linux tarball, Windows ZIP with static CRT).
* Privacy dashboard rendered only in `rc-core` (cannot drift into two claim sets).
* `rc-core` carries a typed, single-writer `prefs.json` store — the prefs-erasure
  defect from the 0.2.0 audit is fixed.
* 40 core tests; CI runs fmt / clippy / test / audit / build on real Linux and
  Windows runners (`.github/workflows/qa.yml`).
* `cargo audit`: 117 crates, no advisories (reported in #4).

## Known gaps (open as of this session)

High priority, from ROADMAP + code inspection:

1. **Linux does not enforce per-site permission rules.** `sites.json` rules are
   stored but `connect_permission_request` in `src/main.rs` unconditionally
   denies every request. Windows honours the rules. Platform divergence.
2. **Linux has no external-protocol policy.** Windows routes non-web schemes
   through a policy; Linux falls back to `xdg-open` for the Downloads menu only.
   `rc_core::normalize` blocks scheme-carrying strings being typed, but clicked
   `mailto:`/`custom:` links need a decision.
3. **Linux downloads have no manager UI.** Files auto-save to `~/Downloads` with
   collision renaming, but there is no way to see, cancel, or open managed
   downloads (Windows has a downloads panel).
4. **No interactive test harness.** Nothing clicks a menu or drives the GTK
   shell; a broken menu item would ship. Largest testing hole.
5. **Windows cold startup ~7.8 s** on CI hardware; WebView2 init dominates. Needs
   measurement on real hardware before optimizing.

Medium / hygiene:

6. Three GitHub issues (#2, #3, #4) are **merged, shipped epics that were never
   closed**. They reflect delivered work, not open work; should be closed for an
   honest "open issues" count.
7. Release artifacts are committed under `rel2/…rel9/` (tarballs, Zips, probe
   JSONs, an `exe`). ~4–5 MB of generated binaries in history; `.git` is 3.8 MB.
   Consider `.gitignore`-ing `rel*/` or moving to GitHub Releases (do not delete
   without authorization).
8. `crates/rc-windows/src/app.rs::wire_events` is 156 lines — the last
   function over 100 lines after the audit's split work.

Low:

9. The README's "single ~1 MB binary" and rounding on "../crates/rc-core : 37
   tests" (doc says 37, repo now has 40) are stale micro-claims.

## Non-goals (deliberate)

Accounts/sync/telemetry; an extension ecosystem; a password manager before the
storage is audited; merging the two shells into one abstraction.