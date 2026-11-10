# Quality assurance

What RustChrome is tested against, what the numbers are, and what is known to be
broken. Everything below comes from a GitHub Actions run on a real operating
system — ubuntu-22.04 and windows-2022 — not from a developer machine.

Two workflows:

* `.github/workflows/ci.yml` — build, package, functional smoke test, privacy
  probe on both platforms.
* `.github/workflows/qa.yml` — the gates and the measurements: `cargo fmt
  --check`, `cargo clippy -- -D warnings`, `cargo test`, release build, `cargo
  audit`, a memory/idle-CPU/startup measurement, and a real-website
  compatibility run.

Nothing is skipped to obtain a green run. When a gate fails, the failure is
reported and the code is fixed.

## What is actually exercised

| Area | How | Platform |
|---|---|---|
| Clean compilation | `cargo build --release` with no warnings (`-D warnings` via clippy) | both |
| Formatting | `cargo fmt --all -- --check` | both |
| Unit tests | `cargo test` — core logic only | both |
| Dependency advisories | `cargo audit` against the RustSec database | both |
| Startup | the browser logs `startup_ms` itself, first finished load | both |
| Memory | summed RSS/working set of the whole process tree | both |
| Idle CPU | two samples five seconds apart, no interaction | both |
| Navigation, multi-tab, shutdown | smoke test: launch, create tab, navigate, verify status 0, exit 0 | both |
| Real websites | `RUSTCHROME_COMPAT` loads example.com, Wikipedia, Hacker News and rust-lang.org, records status and title | Windows only |
| Anti-fingerprinting | privacy probe, two fresh sessions, 18 invariants | both |
| Session cleanup | probe asserts storage is empty in a fresh session | both |

## What is not covered

* **No GUI interaction.** Nothing clicks a button, opens a menu or types into the
  omnibox. The handlers are exercised through their code paths, not through the
  user interface. A broken menu item would not be caught.
* **No GTK shell tests.** `src/main.rs` has zero unit tests — the browser logic
  that lives there (tab bookmarks, zoom bookmarks, dialogs) is only covered by
  compilation.
* **Website compatibility runs on Windows only.** The Linux job has no
  equivalent mode yet.
* **No accessibility testing.** Linux CI logs an AT-SPI warning because no
  accessibility bus exists on the runner; that is the extent of what is known
  about a11y.
* **No certificate tests.** Certificate validation is the engine's; there is no
  code path that weakens it, but nothing verifies that either.
* **No long-running soak test.** Memory is sampled for ten seconds, not hours.
* **No GPU paths.** CI runs software rendering, so hardware compositing,
  hardware video decode and GPU WebGL behaviour are untested.

## Known bugs and unsupported features

Reported honestly. Nothing here is a roadmap promise.

### Bugs

1. **Linux ignores per-site permission rules.** The rules are stored and the
   Windows shell enforces them; the GTK shell still denies every permission
   request unconditionally.
2. **Linux has no settings UI for session mode, search engines or site rules.**
   The preferences exist and are read; there is no control to change them, so in
   practice Linux is always a Private Session with the built-in engine list.
3. **Recently-closed depth differs.** 20 entries in the shared core, 10 in the
   GTK shell's own list.
4. **Linux downloads have no manager.** A completion dialog only: no list, no
   cancel, no progress, no resume.
5. **Windows window title only updates on load.** It shows the active page
   title, but a tab switch does not refresh it until the next title change.
6. **No "restore session?" prompt.** Crash recovery is silent: if the last run
   ended badly and restore is enabled, the tabs come back without asking.
7. **Private Session on Windows is InPrivate tabs, not an ephemeral profile.**
   Page data is not persisted, but the user-data folder still exists on disk
   with its cache and GPU caches.
8. **Bookmarks are flat.** No folders in the UI, no drag and drop, no "save as"
   for export.

### Unsupported, by design or by absence

* No GPU/hardware acceleration verification — untested, may be used or not
  depending on the engine.
* No extensions beyond userscripts. No Web Store, no `chrome.*` APIs.
* No password manager, no autofill, no sync, no accounts, no telemetry.
* No ad or tracker blocker. Neither engine exposes a request-blocking API here;
  tracking prevention (Windows) and ITP (Linux) are engine heuristics, not a
  blocker, and no blocked-request count is shown because none is available.
* No Tor integration and no claim of anonymity. IP address is untouched.
* No macOS or Android target.
* No PDF viewer beyond what the engine renders inline.
* Linux packaging is a tarball; no `.deb`, `.rpm`, Flatpak or AppImage is
  produced by CI.
