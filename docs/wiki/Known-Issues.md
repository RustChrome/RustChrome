# Known issues

The canonical list is [`docs/QA.md`](https://github.com/RustChrome/RustChrome/blob/main/docs/QA.md).
This page is the short version for 0.2.0 Beta.

## Bugs

1. **Linux ignores per-site permission rules.** They are stored and enforced on
   Windows only; the GTK shell denies every permission request.
2. **Linux has no settings UI** for session mode, search engines or site rules. The
   preferences exist and are read; there is no control to change them.
3. **Linux has no download manager** — a completion dialog only: no list, no cancel,
   no progress, no resume.
4. **Recently-closed depth differs**: 20 in the shared core, 10 in the GTK shell.
5. **The Windows window title only updates on load**, not when you switch tabs.
6. **No "restore session?" prompt** — crash recovery restores silently.
7. **Windows private mode is InPrivate tabs**, not an ephemeral profile: the
   user-data folder and its caches still exist on disk.
8. **Bookmarks are flat** — no folders, no drag and drop, no "save as" for export.

## Untested

* **No GUI interaction tests.** Nothing clicks a button; a broken menu item would
  ship. This is the largest hole.
* **No GTK unit tests** — `src/main.rs` has none.
* Website compatibility runs on Windows only.
* No accessibility, certificate, soak or GPU-path testing.
* Memory is sampled for ten seconds, not hours.

## Measured, and not flattering

On four-core CI runners with software rendering: **306–313 MB** Windows (8
processes), **474 MB** Linux (3 processes), ~1.9% / ~0.4% of one core idle, startup
1.9 s Linux and 7.8 s Windows cold. Summed resident memory overcounts shared pages,
but these are not "lightweight" numbers and the project does not market them as such.
