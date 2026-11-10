# RustChrome for Windows

Windows build of RustChrome: a native Win32 window driving the Microsoft Edge
WebView2 (Chromium) engine. The Linux GTK3/WebKitGTK build is unchanged and
still lives in this repository's root package.

Status: **foundation**. It compiles, launches and renders on Windows; treat it as
a working browser shell under active development, not a finished product.

## Requirements

| | |
|---|---|
| OS | Windows 10 (1809+) x64, Windows 11 x64 |
| Engine | Microsoft Edge **WebView2 Runtime** (Evergreen) |
| C runtime | none — the CRT is linked statically |

The WebView2 Runtime is preinstalled on Windows 11 and on Windows 10 1803+ with
the November 2022 update, but it is **not** guaranteed on clean Windows 10, LTSC
or Windows Server installs. RustChrome checks for it at startup and shows an
actionable message instead of crashing if it is missing.

If it is missing: download the Evergreen Bootstrapper from
<https://developer.microsoft.com/microsoft-edge/webview2/> and run it, then
start RustChrome again.

The runtime auto-updates through Microsoft Edge. RustChrome cannot pin its
version; a Chromium update can change rendering and fingerprint behaviour
without a RustChrome release.

## Install (portable)

1. Download `rustchrome-0.1.0-x86_64-windows.zip` from the CI artifacts (or a
   release once one is published).
2. Verify the SHA256 against the `.sha256` file next to it.
3. Extract anywhere and run `rustchrome.exe`. No installer, no admin rights.

## Build from source

Requires the MSVC toolchain (`x86_64-pc-windows-msvc`), Visual Studio 2022
Build Tools with the C++ workload, and the Windows SDK.

```powershell
cargo build --release -p rustchrome-win
# -> target\release\rustchrome.exe
```

The GUI subsystem is enabled in release builds, so no console window appears.
Use a debug build if you want stdout.

### Smoke test

```powershell
$env:RUSTCHROME_SMOKE_OUT = "$PWD\smoke.txt"
.\target\release\rustchrome.exe --smoke-test
Get-Content smoke.txt    # "OK url=... runtime=..."
```

It creates the window, starts WebView2, loads a page, and exits 0 only after the
engine reports a completed navigation. A watchdog fails the run after 60s.

## What it does

Tabs (new, close, reopen, Ctrl+Tab cycling), omnibox that normalises input the
same way the Linux build does, back/forward/reload/stop, per-tab zoom, find in
page with match counts, bookmarks with a bar, downloads with progress and a
"Show" action, light/dark themes, force-dark pages, settings, userscripts,
new-identity, and a clean shutdown that clears browsing data before exiting.

## Where your data lives

| Path | Contents |
|---|---|
| `%LOCALAPPDATA%\RustChrome\prefs.json` | settings |
| `%LOCALAPPDATA%\RustChrome\bookmarks.json` | bookmarks |
| `%LOCALAPPDATA%\RustChrome\userscripts\` | `.js` files injected into every page |
| `%LOCALAPPDATA%\RustChrome\UserData\` | WebView2 profile (only used when "keep session data" is on) |
| `%USERPROFILE%\Downloads` | downloads |

## Privacy model on Windows

By default every tab is created with `IsInPrivateModeEnabled`, so cookies and
site data never touch disk and a crash cannot leak a session. On top of that:

* strict tracking prevention (WebView2's highest level);
* all permission requests denied (12 kinds: microphone, camera, geolocation,
  notifications, sensors, clipboard read, autofill downloads, file read/write,
  autoplay, MIDI, window placement, unknown);
* general autofill and password saving disabled;
* anti-fingerprinting: Chromium UA override (not a Firefox string — an
  engine/UA mismatch is a fingerprint by itself), pinned screen size, DPR, core
  count, memory, language, UTC clock, canvas readback noise, WebGL context denied
  and renderer strings masked, `RTCPeerConnection` removed, `userAgentData`
  pinned;
* "New identity" clears the profile and restarts with one tab.

### Honest limitations vs the Linux build

* WebKitGTK can switch WebGL and WebRTC off **in the engine**. WebView2 has no
  public API for either, so on Windows both are blocked from JavaScript only.
  That is weaker: a page cannot get a WebGL context or a peer connection, but a
  determined script can still probe for the constructors' existence.
* Client hints (`Sec-CH-UA*`) are a Chromium-only fingerprint surface; the UA
  override pins the major version, but the surface does not exist on Linux so
  the two builds are not byte-for-byte comparable.
* Font metrics cannot be normalised.
* Fingerprint parity with Tor Browser is not claimed on either platform.
* "Lightweight" on Windows means startup and RAM, not disk: Chromium is already
  installed system-wide for Edge and is shared, but it is not small.

## Known gaps

* Downloads go to `%USERPROFILE%\Downloads`; a redirected/relocated Downloads
  folder (KnownFolder) is not followed yet.
* No drag-to-reorder for tabs, no session restore, no per-site zoom memory.
* No ad/tracker blocking beyond tracking prevention.
* The UI is HTML in its own WebView2 (one extra renderer process, ~40-70 MB).
