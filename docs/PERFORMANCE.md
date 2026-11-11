# Performance

Measured, not claimed. Numbers come from GitHub Actions runners with software
rendering and no GPU, so they compare one build to another — they are not what a
user with hardware acceleration sees.

## Method

`scripts/measure.sh` and `scripts/measure.ps1` launch the browser, wait five
seconds, then sample over a fixed window:

* resident memory summed over the **whole process tree** (WebView2 and WebKitGTK
  both run renderer and network processes as children; sampling only the parent
  once reported 17 MB for a 300 MB browser);
* idle CPU from two CPU-time samples five seconds apart, with no interaction;
* startup as logged by the browser itself, from process start to the first
  completed load (`startup_ms`).

## 0.2.0 beta

| Linux (ubuntu-22.04) | Windows (windows-2022) |
| --- | --- |
| Processes | 3 | 8 |
| Memory, whole tree | 474 MB | 306–313 MB |
| Memory drift over 5 s | none | +7 MB |
| Idle CPU | ~0.4 % of one core | ~1.9 % of one core |
| Startup | 1.9 s | 7.8 s |
| Binary size | see release asset | 1.13 MB |

## How to read that

* **Summed RSS overcounts.** Shared libraries and shared heap are counted once per
  process. Proportional set size would be the honest metric and is not easy to get
  on either platform, so these are upper bounds.
* **The Linux number includes a real page** (the start page), not a blank window.
* **Windows startup is dominated by cold WebView2 initialisation** on a CI VM with
  no profile. It is the number we have, and it is not good.
* Both runs are CPU-rendered; hardware compositing is untested.

## What is not measured yet

* Multiple tabs under load — the number that actually matters for a browser.
* Long sessions: memory is sampled for ten seconds, not hours.
* Page-load performance versus other browsers (no harness, and a fair comparison
  would need the same engine anyway).

## Rules for future work

Measure before optimising, keep the harness honest (whole tree, fixed window,
same runner), and never trade a privacy or security setting for a benchmark. The
summed-tree correction above is the reason this document exists: the earlier
17 MB figure would have been a lie told with a straight face.
