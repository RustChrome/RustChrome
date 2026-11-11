# Security review

A browser is a program that runs hostile code by design. This is the state of
RustChrome's attack surface, what has been changed, and what is still weak. It is
written to be read by someone deciding whether to trust the project, not to look
impressive.

Reviewed against `main` after the 0.2.0 beta. Nothing here is a guarantee.

## Threat model

In scope: a malicious or compromised web page, a malicious download, a crafted
userscript, and a malicious file the user imports (bookmarks).

Out of scope: a local attacker with the user's privileges, a compromised WebView2
runtime or WebKitGTK package, and the sites' own security.

## What the browser controls

**Navigation.** Content webviews gate every navigation through
`rc_core::is_allowed_navigation`: http, https, `about:`, `data:text/html` for our
own pages, and nothing else. `javascript:` and unknown schemes are refused.

**UI webview.** It may only load the document we write to disk ourselves. It has
no host objects, no devtools, and permission requests are denied. The message
bridge is one-way JSON: the UI can ask for an action, it cannot reach a page.

**Downloads.** `downloads.rs` sanitises the suggested name (traversal, control
characters, Windows reserved device names), picks the Downloads directory, and
resolves collisions without overwriting. Three unit tests cover it.

**Userscripts.** Installed files are read from the userscripts directory and
injected at document start. They run with page privileges — a userscript is
executed code, not a sandbox — and that is stated in the UI.

**Permissions.** Windows consults per-site rules, default deny. Linux denies
every request unconditionally, which is stricter but less useful.

**Session data.** Private Session is the default: an ephemeral WebKit context on
Linux, InPrivate tabs on Windows with `ClearBrowsingDataAll` on exit.

## Findings from this review

| Finding | Status |
| --- | --- |
| External protocol handling is asymmetric: Windows has an allow-list, Linux relies on WebKit defaults and only intercepts `NewWindowAction`. Nothing dangerous found — unknown schemes simply fail — but the policy is not written in one place. | Open |
| Discarded HRESULTs on layout calls made an invisible UI look healthy. Failures are now counted (`BOUNDS_FAILURES`). Silent failure is a security property too: it hid the fact that a control never rendered. | Fixed |
| Bridge callbacks dropped during re-entrancy were silent. Now counted (`DROPPED_MESSAGES`). | Fixed |
| Two preference writers with different schemas could destroy settings, including anti-fingerprinting. Now one typed writer with tests. | Fixed |
| The COM-heavy modules had no `SAFETY:` rationale. Each now states its threading contract. | Fixed |

## Known weaknesses, stated plainly

* **No process sandbox of our own.** We rely entirely on the engine's sandbox.
  There is no separate low-integrity process for the UI.
* **Userscripts are arbitrary code** with the privileges of the page they run in.
* **The Windows private session leaves engine caches on disk** (`docs/QA.md`, 7).
* **No certificate bypass exists** — and nothing verifies that either.
* **No fuzzing, no MIRI, no sanitizers.** `cargo audit` runs on every push; that
  covers known advisories in dependencies, not our own logic.
* **The GTK shell has no automated tests**, so a regression there is caught only
  by CI launching the browser and the probe running.

## Reporting

See `SECURITY.md`. Private advisories only; no public issue for a vulnerability.
