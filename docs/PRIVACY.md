# Privacy and security in RustChrome

This document describes what the two shells actually do, and how it was measured.
It is deliberately conservative: it lists the gaps next to the wins.

RustChrome is **not** an anonymity tool and not a Tor Browser. It is a small
browser with a privacy-first configuration, a scripted hardening layer, and a
test harness that measures what a page can see.

## What was reviewed

| Area | Linux (WebKitGTK 4.1) | Windows (WebView2) |
|---|---|---|
| User agent | engine default, never overridden | Chrome-like UA built from the installed runtime, `Edg/` removed, **UA client hints disabled** |
| Canvas | per-session seeded noise | same shared script |
| WebGL | live, vendor/renderer coerced; denied in Strict | same |
| Screen size | reported as-is (see "contradictions") | same |
| hardwareConcurrency / deviceMemory | pinned to 8 / 8 | same |
| Language | script `en-US,en` + `webkit_web_context_set_preferred_languages` | script + `--accept-lang=en-US,en` |
| Time zone | UTC in `getTimezoneOffset`, `Intl`, `Date#toString` | same |
| WebRTC | live, relay-only requested from script; removed in Strict | live, relay-only (`--force-webrtc-ip-handling-policy=disable_non_proxied_udp`) + script; removed in Strict |
| Media devices | `enumerateDevices()` emptied, all permissions denied | same |
| Permissions | every request denied | every request denied (12 kinds) |
| Cookies / third-party storage | `NoThirdParty` accept policy + ITP enabled | tracking prevention: strict |
| DNS prefetch | `enable-dns-prefetching(false)` | `--dns-prefetch-disable` |
| Hyperlink auditing (`ping`) | `enable-hyperlink-auditing(false)` | `--no-pings` |
| Session cleanup | **ephemeral WebKit context**: nothing is written to disk at all | InPrivate profile per tab + `ClearBrowsingDataAll` on exit |
| Local storage / IndexedDB / service workers | in memory only (ephemeral) | in the in-private profile, cleared on exit |
| Cache | memory only | disk cache in the profile, cleared on exit |
| Tracking behaviour | ITP; no per-request counts available | Microsoft's tracking-prevention list; no counts available |
| New identity | clears data, closes every tab, **rotates the canvas seed** | same |

### Deliberate design decisions

* **No user-agent lie.** The GTK build used to present a Firefox UA while running
  WebKit. That is a stronger fingerprint than a common one, so both shells now
  report an engine-consistent UA. On Windows the UA is rebuilt from the installed
  runtime version and client hints are disabled, so the header, the hints and the
  script cannot disagree.
* **No pinned screen size.** Pinning `screen` to 1280×800 while the real window
  can be resized or maximised produces a detectable contradiction
  (`outerWidth > screen.width`). Real values are reported and the test asserts
  that the client area still fits on screen.
* **Canvas noise is seeded per session, not fixed.** A fixed noise pattern is a
  stable cross-session identifier — worse than no noise. The seed is drawn at
  startup and on New Identity, so repeated reads inside a session are identical
  (sites keep working) and a new session hashes differently.
* **WebGL and WebRTC are coerced, not removed, in standard mode.** Removing them
  breaks maps, video calls and games for no gain in most cases. Strict mode is
  the switch that takes them away, and the dashboard says which mode is active.
* **No certificate-validation bypass anywhere.** There is no
  `server-certificate-error-detected` handler that accepts, and no `--ignore-*`
  flag.

## How it was tested

`crates/rc-core/src/privacy.rs` carries one probe script used by both shells. It
reports what a page can read: UA, languages, time zone (three ways), core count,
memory, screen/window dimensions, plugin count, DNT, WebGL vendor/renderer, canvas
hash twice, WebRTC availability.

* Windows: `rustchrome.exe --privacy-probe <file>`
* Linux: `RUSTCHROME_PROBE=<file> xvfb-run … dbus-run-session -- ./rustchrome`

CI launches the browser **twice per platform** (two separate processes, so two
seeds) and runs `scripts/privacy_check.py` over the two reports. The checker
asserts the properties RustChrome claims and marks anything it cannot observe as
`skip`, never as `pass`.

Command:

```
python3 scripts/privacy_check.py probe1.json probe2.json
```

### Results — run 42895531862, both platforms green

| Check | Linux (WebKitGTK 4.1) | Windows (WebView2 131.0.2903.86) |
|---|---|---|
| canvas stable within a session | pass `f3e75182` = `f3e75182` | pass `caa575b6` = `caa575b6` |
| canvas changes between sessions | pass `f3e75182` → `3e039ea9` | pass `caa575b6` → `3cc1139e` |
| time zone is UTC, offset 0 | pass | pass |
| `Date#toString` agrees with UTC | pass `… GMT+0000 (Coordinated Universal Time)` | pass (same) |
| languages pinned | pass `en-US,en` | pass `en-US,en` |
| memory / cores | pass `8 / 8` | pass `8 / 8` |
| DNT header value | pass `1` | pass `1` |
| client area fits the screen | pass `1280×1024 / 1280×703` | pass `1024×768 / 1020×728` |
| WebGL renderer coerced | pass `Generic / Generic Renderer` | pass `Generic / Generic Renderer` |
| storage APIs | skip (probe runs on an opaque origin) | skip (same) |
| WebRTC candidate policy | skip (not observable from script) | skip (same) |

Both platforms: **18/18 checks passed, 0 failed** (3 of them explicit skips).

Observed platform differences, stated rather than smoothed over:

* WebKitGTK does not expose `RTCPeerConnection` on an opaque origin, so WebRTC
  availability could not be measured there. WebView2 does expose it.
* Plugin count is 5 on both, but they are not the same plugins.
* The Linux UA is WebKit's own (`X11; Ubuntu … Version/60.5 Safari/605.1.15`),
  the Windows UA is a Chromium UA without `Edg/`.

## What this does not prove

* It does not prove the browser is unlinkable. The probe measures what *this page
  in this run* sees; real fingerprinting uses far more surface (fonts, GPU
  timing, TLS/JA3, audio, client hints on other platforms).
* Font enumeration and GPU model are not normalised on either platform.
* Tracker blocking cannot be counted: neither engine exposes how many requests
  ITP or tracking prevention stopped, so the dashboard shows none.
* WebRTC relay-only is an engine policy on Windows and a JS request on Linux; the
  test cannot observe which candidates were actually gathered.
* Downloads, TLS and the network stack were not part of this task's measurements.

## Reproducing locally

```
# Windows
.\target\release\rustchrome.exe --privacy-probe probe1.json   # twice
python scripts\privacy_check.py probe1.json probe2.json

# Linux
RUSTCHROME_PROBE=$PWD/probe1.json ./target/release/rustchrome  # twice, under X
python3 scripts/privacy_check.py probe1.json probe2.json
```
