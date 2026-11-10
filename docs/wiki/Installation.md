# Installation

## Windows 10 / 11 (portable)

1. Download `rustchrome-0.2.0-x86_64-windows.zip` from the
   [release](https://github.com/RustChrome/RustChrome/releases/latest).
2. Verify it (below).
3. Unzip anywhere and run `rustchrome.exe`.

Nothing is installed, nothing is written to the registry, there is no uninstaller —
delete the folder when you are done. The build links a static C runtime, so no
Visual C++ redistributable is needed.

**Requires the Microsoft Edge WebView2 Runtime.** It ships with Windows 11 and
current Windows 10. If it is missing, RustChrome tells you and links to Microsoft's
Evergreen bootstrapper.

## Linux x86_64

```bash
sha256sum -c rustchrome-0.2.0-x86_64-linux.tar.gz.sha256
tar xzf rustchrome-0.2.0-x86_64-linux.tar.gz
cd rustchrome-0.2.0-x86_64-linux
./rustchrome
```

Requires GTK3 and WebKitGTK 4.1:

```bash
# Debian / Ubuntu
sudo apt install libgtk-3-0 libwebkit2gtk-4.1-0
# Arch
sudo pacman -S gtk3 webkit2gtk-4.1
# Fedora
sudo dnf install gtk3 webkit2gtk4.1
```

There is no `.deb`, `.rpm`, Flatpak or AppImage yet. It is a tarball.

## Verify the download

```bash
sha256sum -c rustchrome-0.2.0-x86_64-linux.tar.gz.sha256
```

```powershell
(Get-FileHash .\rustchrome-0.2.0-x86_64-windows.zip -Algorithm SHA256).Hash.ToLower()
# compare with the first field of the .sha256 file next to it
```

Each release lists its checksums in the release notes and on the assets themselves.

## Environment variables

| Variable | Effect |
| --- | --- |
| `RUSTCHROME_START_URL` | comma-separated URLs to open instead of the start page |
