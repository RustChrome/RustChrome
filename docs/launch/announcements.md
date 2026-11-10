# Launch drafts — for approval, not posted

Every draft assumes **you post it from your own account, as the author**. Change
the first person if that is not how you want to sound. Nothing here claims a user
number, a benchmark or a feature the code does not have.

---

## r/rust

**Title:** RustChrome 0.2.0 beta — a browser with two engine shells and a shared Rust core

**Body:**

I've been building a small browser in Rust and it's finally worth other people
looking at. Beta, so please read the known-issues list before trying it.

The part I think is interesting is the architecture: one `rc-core` crate with no
GUI dependencies (omnibox rules, prefs, storage, privacy payloads, bookmarks,
session, tab search, site rules) and two thin shells — GTK3 + WebKitGTK on Linux,
Win32 + WebView2 on Windows. No Tauri, no bundled Chromium, no Electron. On Linux
it links the system WebKitGTK; the binary is under a megabyte.

Things I got wrong and fixed, in case it saves someone else the afternoon:

- I was setting a Firefox user agent on WebKitGTK. That is a lie the engine
  exposes in a dozen ways — `navigator.userAgent` says Firefox, everything else
  says WebKit. Fingerprinting tools see the mismatch immediately. The UA now
  matches the engine.
- I was salting canvas reads with a fixed seed, which is not noise, it's a
  constant — a stable identifier. It is per-session now: stable while the session
  lasts (so a page that reads twice does not look synthetic), different on every
  launch (so it cannot follow you).
- WebGL/WebRTC removal broke sites, so coercion is the default and removal is an
  opt-in Strict mode.

The privacy work is tested by a harness that runs on every CI push: two fresh
sessions, 18 invariants, including "canvas stable within a session, different
across sessions".

Honest state: Linux has no download manager and ignores per-site permission
rules. Bookmarks are flat. There are no GUI interaction tests — nothing clicks a
menu, so a broken menu item would ship. Measured 306–474 MB of memory depending
on platform, which is not lightweight and I'm not going to call it that.

`docs/QA.md` has the full list.

<https://github.com/RustChrome/RustChrome>

MIT. No telemetry, no accounts, no sync. Feedback on the architecture is more
welcome than stars.

**Note:** r/rust is fine with project posts from the author, but check the
current sidebar rules on self-promotion before posting, and consider the weekly
threads as an alternative if the post gets removed.

---

## r/linux (or r/linuxquestions for a softer audience)

**Title:** RustChrome: a GTK3 + WebKitGTK browser in Rust, no telemetry, looking for testers

**Body:**

Looking for a handful of Linux users to try this and tell me what breaks.

It's a GTK3 + WebKitGTK 4.1 browser written in Rust — links the system WebKitGTK
rather than shipping its own Chromium, so the package is small and it updates
with your distro's engine.

What works: tabs, omnibox, bookmarks, userscripts, find in page, zoom, light/dark
themes, bookmark import from Chrome/Firefox, crash restore, Ctrl+Shift+A tab
search, recently closed.

Privacy defaults: no history database, no telemetry, no accounts. Private Session
by default — the WebKit data store is wiped on exit. New Identity wipes and
reopens a fresh tab. Canvas reads get per-session noise; Strict mode neutralises
WebGL/WebRTC and will break some sites, which is why it's opt-in.

**Not anonymity.** No IP protection, no Tor. `docs/PRIVACY.md` says exactly what
is and isn't covered.

Known gaps on Linux specifically: no download manager (completion dialog only),
per-site permission rules are stored but not enforced in the GTK shell yet, and
there's no settings UI for session mode or search engines. No `.deb` or `.rpm`
yet — it's a tarball with a sha256.

Ten testers is the goal, not a thousand. If it crashes on your distribution, the
bug template asks for the WebKitGTK version, which is usually the whole story.

<https://github.com/RustChrome/RustChrome>

**Note:** r/linux has strict self-promotion rules and removes many project posts.
Check the sidebar, and prefer a weekly thread or r/linuxquestions if the rules
bar it.

---

## r/browsers

**Title:** RustChrome 0.2.0 beta: system-engine browser in Rust (WebKitGTK / WebView2), no telemetry

**Body:**

Short version: a browser that uses the engine already on your system instead of
shipping one. WebKitGTK 4.1 on Linux, WebView2 on Windows, browser logic shared in
Rust.

It is not trying to replace Chrome for you tomorrow. It is trying to be a
plausible small browser with honest privacy defaults.

- No telemetry, no accounts, no sync, no history database.
- Private Session by default (data store wiped on exit); Standard Session is opt-in.
- Per-session canvas noise, engine-consistent UA, third-party cookies off, ITP /
  tracking prevention on, DNS prefetch off, InPrivate tabs on Windows.
- Strict mode removes WebGL/WebRTC and will break sites — opt-in for a reason.

What it does not have: ad/tracker blocking (neither engine exposes a
request-blocking API I can use), a password manager, extensions, macOS, Android.
Memory measured at 306–474 MB on CI hardware; startup 1.9 s Linux / 7.8 s Windows
cold. Those numbers are in the repo, not just in this post.

Beta, MIT, looking for ten people who'll actually use it for a day.

<https://github.com/RustChrome/RustChrome>

---

## Hacker News — Show HN

**Title:** Show HN: RustChrome – a browser in Rust using the system engine

**Body (as the first comment, posted immediately by you):**

Author here. RustChrome is a browser with two engine shells and one shared core:
GTK3 + WebKitGTK on Linux, Win32 + WebView2 on Windows, with everything
platform-neutral in a `rc-core` crate that has no GUI dependencies.

The design bet is that a browser does not have to ship its own engine. On Linux
the release tarball is a sub-megabyte binary that links your distro's WebKitGTK,
which also means security updates come from the distro.

Two things I got wrong first and want to be explicit about, because both are easy
mistakes:

1. I set a Firefox UA on WebKitGTK to look "hardened". That is a fingerprint in
   itself — the UA contradicts `navigator.vendor`, the feature set, and the
   rendering quirks. It now reports the engine it is.
2. I used a fixed seed for canvas noise. That is not noise, it's a constant
   identifier that follows the user forever. It is per-session now: consistent
   within a session, different per launch.

Privacy claims are backed by a probe that runs on every CI push: two fresh
sessions, 18 invariants, canvas stability within a session and difference across
sessions among them. What it does not do is stated in `docs/PRIVACY.md`: no IP
protection, no Tor, no ad blocking.

Known holes are documented rather than hidden: Linux has no download manager and
doesn't enforce per-site permission rules yet, there are no GUI interaction tests,
and memory measures 306–474 MB, which is not "lightweight" and I don't market it
as such.

Beta. MIT. Feedback welcome, especially "this site broke".

**Note:** Show HN is one post, once. Do not ask anyone to upvote, do not repost
if it does not take off — post again only after a release with real changes.

---

## users.rust-lang.org → Announcements

**Title:** RustChrome 0.2.0 — two browser shells, one shared core (looking for testers)

**Body:**

Announcing a beta of RustChrome, a small browser in Rust.

Architecture, briefly: `crates/rc-core` holds everything that is not a window —
omnibox parsing, prefs and atomic stores, privacy payloads, bookmark
import/export, session records, tab search ranking, per-site rules — with tests
and no GUI dependencies. `src/main.rs` is the GTK3 shell; `crates/rc-windows` is
the Win32 + WebView2 shell. They share behaviour, not code structure.

The Windows shell talks to its engine through `webview2-com` directly rather than
Tauri, and the browser chrome is HTML in its own WebView2 instance, which keeps
window management and engine lifecycle in our hands at the cost of a bridge
protocol.

Testing: `cargo test` for the core, plus a privacy probe and a website
compatibility run on real GitHub runners for both platforms. Everything gated on
`fmt`, `clippy -D warnings` and `cargo audit`.

I would value review of the core crate's API — that's the part meant to survive
the next three platforms.

<https://github.com/RustChrome/RustChrome>

**Note:** the Announcements category has its own posting rules; check them. This
audience will care about the crates.io API, not the marketing.

---

## Lobsters

Invite only — only if you already have an account. Same text as r/rust, trimmed:
lead with the two-engine architecture and the UA/canvas mistakes, drop the
install instructions.

---

## Do not

* Post the same text to several places on the same day.
* Ask anyone to upvote, star, or "check out my friend's project".
* Claim "lightweight", "private" or "fast" without the numbers from `docs/QA.md`.
* Reply to criticism with a roadmap promise. Answer with what exists today.
