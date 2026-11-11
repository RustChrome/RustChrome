# Performance

Canonical: [`docs/PERFORMANCE.md`](https://github.com/RustChrome/RustChrome/blob/main/docs/PERFORMANCE.md).

Measured on GitHub runners, software rendering, whole process tree.

| Linux | Windows |
| --- | --- |
| 3 processes, 474 MB, ~0.4 % of one core idle, 1.9 s startup | 8 processes, 306–313 MB, ~1.9 % idle, 7.8 s startup cold |

Read them carefully:

* Summed RSS counts shared pages once per process, so the real unique cost is
  lower — these are upper bounds.
* Windows startup is dominated by cold WebView2 initialisation on a CI VM.
* Nothing here is measured on hardware acceleration, with many tabs, or over hours.

The whole-tree number matters because an earlier measurement sampled only the
browser process and reported 17 MB for a 300 MB browser. That was a lie told with a
straight face, and the harness now sums the tree.
