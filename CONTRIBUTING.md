# Contributing to RustChrome

RustChrome is a small browser with a small surface area, on purpose. The fastest
way to get a change merged is to keep it that way.

## Before you start

* For anything larger than a bug fix, open an issue or a discussion first. A
  rejected 800-line patch is a bad day for both of us.
* Read `docs/QA.md` (what is tested, what is known broken) and
  `docs/PRIVACY.md` (what the protections do and do not claim).

## Build

**Linux** (GTK3 + WebKitGTK 4.1):

```bash
sudo apt install build-essential librust-gtk-3-dev libwebkit2gtk-4.1-dev   # Debian/Ubuntu
cargo build --release
./target/release/rustchrome
```

**Windows** (Win32 + WebView2): needs the MSVC target and the WebView2 Runtime.

```powershell
rustup target add x86_64-pc-windows-msvc
cargo build --release -p rustchrome-win
.\target\release\rustchrome.exe
```

## Layout

| Path | What it is |
| --- | --- |
| `src/main.rs` | the Linux shell: GTK3 window, tabs, menus, dialogs |
| `crates/rc-core` | everything platform-neutral: omnibox rules, prefs, stores, privacy payloads, bookmarks, session, tab search, site rules. **No GUI dependencies.** |
| `crates/rc-windows` | the Windows shell: Win32 window, WebView2 controllers, downloads, HTML chrome |
| `scripts/` | the privacy checker and the measurement harnesses |
| `docs/` | architecture, security review, privacy, performance, QA, release process |

New browser behaviour belongs in `rc-core` with tests. Platform shells only
render it and wire it to their engine.

## Where things are written down

| Question | Document |
| --- | --- |
| How is this put together? | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) |
| What does it claim about privacy, and is that verified? | [`docs/PRIVACY.md`](docs/PRIVACY.md) |
| What is broken or untested? | [`docs/QA.md`](docs/QA.md) |
| How is it attacked, and what is still weak? | [`docs/SECURITY-REVIEW.md`](docs/SECURITY-REVIEW.md) |
| How fast is it, and how was that measured? | [`docs/PERFORMANCE.md`](docs/PERFORMANCE.md) |
| How is a release cut? | [`docs/RELEASE.md`](docs/RELEASE.md) |
| Windows specifics | [`docs/WINDOWS.md`](docs/WINDOWS.md) |

## Gates

Every pull request must pass, and none of these are optional:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
cargo audit
```

CI runs the same things on ubuntu-22.04 and windows-2022. A green run is
required; do not disable a check or mark a test `#[ignore]` to get one.

## Testing

`cargo test` covers `rc-core` and the Windows helpers. Beyond that, CI runs things
a unit test cannot:

* **Smoke test** — launches the browser, opens a tab, navigates, asserts the
  navigation completed and exits 0. Both platforms.
* **Privacy probe** — two fresh sessions, 18 invariants, compared by
  `scripts/privacy_check.py`.
* **Compatibility run** — loads four real sites and records status and title.
* **Measurement** — memory, idle CPU and startup, over the whole process tree.

The GTK shell has **no unit tests**: its logic needs a display and a
`GtkApplication`. If you touch `src/main.rs`, say in the pull request what you ran
by hand, because nothing else will catch a regression there.

## Code rules

* **No new dependency without a reason in the pull request.** This browser is
  measured in megabytes and milliseconds.
* **No telemetry, ever.** No analytics, no crash reporting service, no update
  pings, no "anonymous usage statistics". A pull request that adds one will be
  closed.
* **No new network traffic at startup.** The only connections are the pages the
  user asked for.
* **No fake functionality.** A button that does nothing, a setting that is not
  read, a privacy claim the engine cannot back — all rejected.
* **No placeholder features behind flags that are on by default.**
* Keep the engine honest: do not make WebKitGTK claim to be Chrome or WebView2
  claim to be Firefox. A lying user agent is a fingerprint.

## Privacy changes

Anything touching fingerprinting, permissions, storage or session cleanup needs:

1. A line in `docs/PRIVACY.md` explaining what changes and for which engine.
2. A probe check in `scripts/privacy_check.py` if it is observable from a page.
3. Proof it does not break ordinary sites (see the compatibility run in
   `.github/workflows/qa.yml`).

Do not claim anonymity. RustChrome does not provide it.

## Commits and pull requests

* One logical change per commit; write the message in the imperative
  ("fix", "add", not "fixed", "added") and explain *why* in the body.
* Say what you tested. "Compiles" is not testing.
* If you changed behaviour, say what the old behaviour was.

## Reporting bugs

Use the bug template. Include the platform, the version, the site that broke and
the exact steps. If you can, say whether it also happens in Firefox or Chrome —
that separates our bug from the site's.
