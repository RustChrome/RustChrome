# Technical Debt

Live ledger of outstanding engineering problems. Ordered by severity/priority.
"Done" entries are removed, not commented out, unless a note is needed.

## Critical (security / privacy / data loss)

None currently tracked.

## High

1. **Linux external-protocol policy.** Clicked links to non-web schemes
   (e.g. `mailto:`, custom handlers) have no defined behaviour on Linux while
   Windows has a policy. A hostile page that can trigger an arbitrary external
   handler is a real vector; decide allow-list + UX, and keep it consistent.

2. **Interactive test harness coverage is still thin.** The first harness test
   passes on the Linux runner, but it drives only three buttons. Menu items,
   omnibox actions, find-in-page and bookmarks are not yet exercised; expand the
   harness over time rather than letting it stay a three-click smoke test.

3. Ship 0.2.3 (all work is verified on real runners).

## Medium

4. **Linux download manager.** Downloads auto-save with collision renaming but
   cannot be viewed, cancelled, or re-opened from the UI (Windows has a
   downloads panel). Parity gap.

5. **`wire_events` too long** — `crates/rc-windows/src/app.rs`, 156 lines. The
   audit split the other oversized functions; this one remains.

6. **Windows cold startup ~7.8 s.** Need real-hardware measurement of WebView2
   init before any optimization; optimizing blind is prohibited.

7. **Memory 300–470 MB.** Must be reduced with measurement, not adjectives.

8. **Stale repo hygiene:** three done epics still open on GitHub (#2, #3, #4);
   generated release artifacts committed under `rel2/…rel9/`.

## Low

9. Stale doc micro-claims (README binary size). The "37 tests" claim is now 45
   and corrected in ARCHITECTURE.md.

## Work in progress (0.2.3 themes)

Implemented in `rc-core` (verified: 45 tests) and written in both shells. The
Linux shell (GTK) and Windows shell (`chrome.html`) wiring is **not yet
compiled** in this sandbox — the sandbox lacks `webkit2gtk-4.0` (only 4.1 is
present) and there is no Windows host. It must pass the real-runner qa.yml gates
before the 0.2.3 release is cut. Items to verify there:

* Linux: per-theme GTK stylesheet parse under xvfb (test added), theme picker
  menu, toolbar-cycle button, new-tab/tab-background colours.
* Windows: injected `html[data-theme]` blocks render for every theme, theme menu
  cycle + label, new-tab/error pages.

## Interactive test harness — VERIFIED (real-runner, 2026-11-27)

`shell_tests::clicking_buttons_changes_state_when_a_display_exists` builds the
real `App` under xvfb, clicks the theme / `+` / ⋯ buttons by stable widget name,
and asserts state changes. **Passed on the Linux QA runner** (test printed
`ok`). Widgets got stable `set_widget_name` values (also an accessibility win).

## Linux permission enforcement — VERIFIED (real-runner, 2026-11-27)

`src/main.rs` consults `rc_core::features::site_decision` on every WebKitGTK
permission request. The only compile error the CI caught (missing
`UserMediaPermissionRequestExt` import for `is_for_*_device`) was fixed, and QA
Linux passed with clippy `-D warnings`. Deny-by-default preserved.

## Observability counters

Windows tracks `DROPPED_MESSAGES` and `BOUNDS_FAILURES` so silent failures become
countable. No equivalent exists on Linux or for download failures — a good place
to add parallel counters rather than swallowing `let _ =`.