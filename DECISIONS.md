# Decisions

Architectural decisions and their reasoning. New decisions get appended; they
are never silently changed. "Why" matters more than the choice itself.

## D1. Two shells over one core, not one `Browser` trait

GTK and Win32 have almost no shared surface — widget signals vs COM callbacks,
`Rc<RefCell<…>>` vs apartment-threaded COM, 2,400-line signal wiring vs
controller lifecycle. A trait forces the lowest common denominator and a pile of
`#[cfg]`. What is genuinely shared (prefs, storage, parsing, ranking, privacy
payloads, dashboard, navigation allow-list) lives in `rc-core`, tested there
without a display server.

Origin: README "two honest shells over one core beats one abstraction that fits
neither"; docs/ARCHITECTURE.md.

## D2. `prefs.json` has exactly one writer, and writes are atomic

The 0.2.0 erasure bug happened because two writers with different schemas
saved a theme toggle. Every store write goes through `write_atomic` (temp file +
rename); a corrupt file falls back to defaults instead of stopping the browser.

## D3. The privacy dashboard is rendered only by `rc-core`

A single renderer means one set of claims, impossible to drift into two variants
per shell. Never re-implement the dashboard in a shell.

## D4. The UI webview only ever shows our own document

The UI webview never navigates to anything but its own HTML; content webviews
pass every navigation through `rc_core::is_allowed_navigation`. Untrusted
content must not reach native functionality.

## D5. User agents are honest per engine

WebKitGTK identifies as WebKit, WebView2 as Chromium. Lying about the engine is
itself a fingerprint. The JS shim does not repeat the lie.

## D6. Anti-fingerprinting is per-session, not per-version

Canvas noise is a pure function of (pixel, per-session seed): stable within a
session, different every launch, rotated by "New identity". A fixed pattern (the
original defect) was a stable cross-session identifier.

## D7. Privacy is the boot state, not a settings page

Permissions default to denied, Strict mode neutralises WebGL/WebRTC, browser
presents one generic machine. The product declines to monetise the surfaces it
controls. This is a product principle, not a feature toggle.

## D8. Release artifacts should not live in git history

`rel2/…rel9/` committed tarballs, Zips, probe JSONs and an `exe`. Generated
binaries bloat history and add nothing to version control. Decision: move
release delivery to GitHub Releases / CI artifacts and `.gitignore` new `rel*/`.
Requires authorization before deleting committed history. (Open — see
TECHNICAL_DEBT #8.)

## D9. Two honest shells, never merged

No plan to rewrite Linux as Windows or vice versa. A single abstraction would
lose more than it gains.

## D10. Theme colours live once, in `rc-core`, and each shell renders them in its own substrate

Themes are a shared semantic `Palette` (window, chrome, surfaces, text, accent,
hairlines, new-tab page) defined and tested in `rc-core/src/themes.rs`. Linux
renders it as GTK `@define-color` blocks over one theme-agnostic rule sheet;
Windows injects `html[data-theme]` CSS blocks into `chrome.html` at startup. No
shell carries a second copy of the colours, so a palette change can't drift.

Rationale: the two substrates genuinely differ (GTK widgets vs WebView2 HTML),
so forcing one CSS representation would be D1's mistake again. What is shared is
the *data* — the palette and its validity — which is exactly what `rc-core` owns.
The two default themes (light/dark) reproduce the 0.2.2 colours slot-for-slot so
upgrading never restyles an existing user, and a test enforces it.

## D11. Theme count is deliberately small and curated

Six themes: two defaults plus four tasteful accents (Graphite, Sepia, Midnight,
Nord; two of them light). Fifty skins would be noise, not value. Adding a theme
is a one-block change in `themes.rs` plus a test — the point is that the *data*
is cheap to extend even though the shipped set is small.