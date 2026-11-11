# Security policy

RustChrome is a browser. It renders untrusted content from the internet, so
memory-safety and sandbox assumptions matter, and a report is welcome even when
it turns out to be a false alarm.

## Supported versions

| Version | Supported |
| --- | --- |
| 0.2.x (beta) | yes — fixes land on `main` |
| 0.1.0 | no, superseded |

Pre-release builds are beta software. They are not described as stable anywhere
in this repository, and they should not be used where a compromise would be
expensive.

## Reporting a vulnerability

**Do not open a public issue for a security problem.**

Use GitHub's private reporting: the **Security** tab → **Report a vulnerability**.
That opens a private advisory visible only to the maintainers.

Please include:

* The version (archive name or commit) and platform.
* What an attacker gains — code execution, file read, permission bypass, data
  leak, or "only" a fingerprint that contradicts `docs/PRIVACY.md`.
* Reproduction steps, and a proof of concept if you have one.
* Whether it also reproduces in the upstream engine (WebKitGTK, WebView2/Edge).

Expect an acknowledgement within a few days. This is a small project without a
security team; there is no bug bounty.

## What has already been reviewed

[`docs/SECURITY-REVIEW.md`](docs/SECURITY-REVIEW.md) contains the threat model, the
controls that exist, the findings with their status, and the weaknesses that are
known and accepted. Read it before reporting: if your finding is listed there as
open, an issue is still welcome, but it saves you writing it up.

## Scope

In scope, because it is our code:

* The permission handler, download destination policy, and file naming
  (`crates/rc-windows/src/{downloads,app}.rs`, `src/main.rs`).
* The navigation gates and the internal-page handling on both shells.
* The userscript installer and its file handling.
* The bookmark importer, which reads files produced by other browsers.
* The storage layer in `crates/rc-core/src/store.rs` — atomic writes, path
  handling, anything that could be tricked into writing outside its directory.
* Any privacy behaviour that contradicts what `docs/PRIVACY.md` claims.
* Anything that makes the HTML chrome's message bridge reachable from web
  content.

Out of scope, because it is not our code:

* Memory-corruption bugs in WebKitGTK or WebView2/Chromium. Report those
  upstream; we will ship the update as soon as the distro or Microsoft does.
* Site-side vulnerabilities.
* Missing features listed in `docs/QA.md`, and the documented limitations of the
  privacy model — for example, that the IP address is not hidden and that this
  is not Tor.
* Self-XSS via pasting into the developer console (devtools are disabled).

## What we already know and do not pretend otherwise

* There is no process sandbox beyond what the engine provides.
* The Windows build runs with the user's privileges; there is no separate
  low-integrity process of our own.
* Private Session avoids writing browsing data to disk, but the WebView2
  user-data folder still exists with engine caches (`docs/QA.md`, bug 7).
* The anti-fingerprinting work reduces uniqueness; it does not provide
  anonymity.

## Disclosure

We aim to publish a fixed release within 90 days of a confirmed report, sooner
for anything being exploited. Credit is given in the release notes unless you
ask to stay anonymous.
