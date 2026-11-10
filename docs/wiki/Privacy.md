# Privacy

Summary of [`docs/PRIVACY.md`](https://github.com/RustChrome/RustChrome/blob/main/docs/PRIVACY.md),
which is the canonical document.

## What the browser does

* **Private Session by default** — the web data store is cleared on exit; on Linux
  the WebKit context is ephemeral, so there is nothing to clear.
* **No history database**, no telemetry, no crash pings, no update checks, no
  accounts, no sync.
* **Per-session canvas noise** — stable while the session lasts, different on every
  launch. It cannot follow you across launches, and it does not make a page's own
  reads look random.
* **Engine-consistent user agent.** WebKitGTK reports WebKit, WebView2 reports
  Chromium. A browser that lies about its engine is a fingerprint of its own.
* **Normalised surface**: fixed screen size, 1× pixel ratio, 2 cores, 4 GB, English,
  UTC, zero plugins.
* **Third-party cookies off**, ITP (Linux) / tracking prevention (Windows),
  DNS prefetch and hyperlink auditing off, WebRTC relay-only.
* **Strict mode (opt-in)** removes WebGL and WebRTC entirely and will break sites.
* **New Identity** wipes the data store, rotates the canvas salt and reopens one
  clean tab.

## What it does not do

* It does not hide your IP address. There is no proxy and no Tor.
* There is no ad or tracker blocker. Tracking prevention is an engine heuristic, not
  a blocker, and no blocked-request count is shown because none is available.
* It is not anonymity. It reduces how unique you look; it does not make you one of
  a crowd on its own.
* The Windows private session is InPrivate tabs over a persistent user-data folder,
  so engine caches still exist on disk.

## How it is tested

A probe runs twice in fresh sessions on every CI push and checks 18 invariants —
including that canvas noise is identical within a session and different across
sessions. Anything the probe cannot observe is reported as `skip`, not as a pass.
