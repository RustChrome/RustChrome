# Technical Debt

Live ledger of outstanding engineering problems. Ordered by severity/priority.
"Done" entries are removed, not commented out, unless a note is needed.

## Critical (security / privacy / data loss)

None currently tracked.

## High

1. **Linux does not enforce per-site permission rules.** `src/main.rs`
   `connect_permission_request` always `deny()`, ignoring `sites.json`. Windows
   honours the rules. Users who add an allow-rule on Linux get a silent no-op —
   a correctness divergence with a privacy-policy flavour (over-denies, so it is
   safe, but it is broken UX and platform-divergent). Fix: consult the shared
   site-rules lookup from the permission-request handler.

2. **Linux has no external-protocol policy.** Clicked links to non-web schemes
   (e.g. `mailto:`, custom handlers) have no defined behaviour on Linux while
   Windows has a policy. A hostile page that can trigger an arbitrary external
   handler is a real vector; decide allow-list + UX, and keep it consistent.

3. **No interactive/GUI test harness.** The GTK shell has unit tests but nothing
   drives the actual widget tree or clicks a menu. A broken menu item ships
   unnoticed. This is the single largest reliability gap.

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

## Observability counters

Windows tracks `DROPPED_MESSAGES` and `BOUNDS_FAILURES` so silent failures become
countable. No equivalent exists on Linux or for download failures — a good place
to add parallel counters rather than swallowing `let _ =`.