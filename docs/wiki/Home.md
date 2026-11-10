# RustChrome wiki

RustChrome is a small, privacy-first desktop browser written in Rust. It uses the
engine your system already has instead of shipping one: **GTK3 + WebKitGTK 4.1** on
Linux, **Win32 + WebView2** on Windows. MIT licensed, no telemetry, no accounts, no
sync server, no history database.

**Current version: 0.2.0 Beta** — [download](https://github.com/RustChrome/RustChrome/releases/latest).
Beta software. Not labelled stable anywhere, and it has [known bugs](Known-Issues).

## Start here

| Page | What it covers |
| --- | --- |
| [Installation](Installation) | Download, verify and run on Linux or Windows |
| [Building](Building) | Build from source, dependencies, tests |
| [Privacy](Privacy) | What the protections actually do, and what they do not |
| [Known Issues](Known-Issues) | Every bug we know about, and everything untested |
| [Troubleshooting](Troubleshooting) | It will not start, blank window, rendering glitches |
| [Roadmap](Roadmap) | What is shipped, next, and deliberately not planned |
| [Contributing](Contributing) | Gates, code rules, how to send a patch |

The canonical versions of the privacy and QA documents live in the repository under
[`docs/`](https://github.com/RustChrome/RustChrome/tree/main/docs). This wiki is the
front door; if the two ever disagree, the repository is right.

## What it is not

Not Tor. It does not hide your IP address. There is no ad blocker, no password
manager, no extension store, no macOS or Android build. Saying that up front saves
everyone an issue report.
