# Troubleshooting

## Windows

**"WebView2 Runtime is missing"** — install Microsoft's Evergreen bootstrapper, or
use Windows 11 / updated Windows 10 where it ships by default.

**The window opens but there is no toolbar** — a known failure mode that was fixed
in 0.2.0 by loading the browser UI from a local virtual host. If you see it, say
which version and whether it also happens after a restart.

**Antivirus flags the exe** — it is unsigned. Portable, self-built binaries get
flagged by heuristic scanners. Build it yourself if you prefer.

## Linux

**`error while loading shared libraries: libwebkit2gtk-4.1.so.0`** — install
WebKitGTK 4.1 (`libwebkit2gtk-4.1-0`, `webkit2gtk-4.1` or `webkit2gtk4.1` depending
on distro).

**Blank or black page with software rendering** — try
`WEBKIT_DISABLE_COMPOSITING_MODE=1 ./rustchrome`.

**Wayland issues** — run with `GDK_BACKEND=x11` to compare; report which one fails.

## Both

**It does not start at all** — run it from a terminal and read stderr:

```bash
./rustchrome 2> log.txt
```

```powershell
.\rustchrome.exe 2> log.txt
```

**Something broke on one specific site** — say whether it also happens in Firefox
or Chrome. That separates our bug from the site's.

Still stuck? Open a thread in
[Discussions](https://github.com/RustChrome/RustChrome/discussions) with your
platform, version and what you did.
