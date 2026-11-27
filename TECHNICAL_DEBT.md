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

2. **Interactive test harness unverified.** The harness
   (`clicking_buttons_changes_state_when_a_display_exists`) is written and
   grounded in the real gtk 0.18.2 API, but has not yet been compiled/run — it
   must pass under CI's `xvfb-run` before it can be trusted to gate future
   clicks.

3. **WebKitGTK permission mapping unverified.** The Linux permission-rule
   wiring (`permission_kind_for` + handler) is written but not compiled; the
   `is::<T>()` downcasts and `WebView::uri()` usage must be validated on the
   real runner.

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

## Linux permission enforcement (implemented)

`src/main.rs` now consults `rc_core::features::site_decision` on every WebKitGTK
permission request, mapping each concrete request type (notification,
geolocation, device-info, user-media audio/video) to the same kind names Windows
uses. **Deny-by-default is preserved** — only an explicit user allow-rule changes
the outcome — so there is no security regression. Status: written, **not
compiled** in this sandbox (needs webkit2gtk-4.0); verified by the real-runner
`qa.yml`.

## Interactive test harness (written, needs real-runner verification)

`shell_tests::clicking_buttons_changes_state_when_a_display_exists` builds the
real `App` under a display, pumps the main loop, and clicks the theme, new-tab
and ⋯ menu buttons by widget name, asserting state changes (a click that does
nothing is a broken feature). Widgets got stable `set_widget_name` values for
findability (also an accessibility win). The test skips without a display and
runs under CI's `xvfb-run`. Status: API is grounded in the actual gtk 0.18.2
sources but this is **unverified** — it has not been compiled or executed here.

## Observability counters

Windows tracks `DROPPED_MESSAGES` and `BOUNDS_FAILURES` so silent failures become
countable. No equivalent exists on Linux or for download failures — a good place
to add parallel counters rather than swallowing `let _ =`.