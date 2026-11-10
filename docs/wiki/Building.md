# Building from source

## Linux (GTK3 / WebKitGTK 4.1)

```bash
sudo apt install build-essential librust-gtk-3-dev libwebkit2gtk-4.1-dev
cargo build --release
./target/release/rustchrome
```

## Windows (Win32 / WebView2)

```powershell
rustup target add x86_64-pc-windows-msvc
cargo build --release -p rustchrome-win
.\target\release\rustchrome.exe
```

The portable build uses a static C runtime (`.cargo/config.toml` sets
`+crt-static`), which is why the ZIP runs on a machine without the VC++
redistributable.

## Layout

| Path | What it is |
| --- | --- |
| `src/main.rs` | the Linux shell: GTK3 window, tabs, menus, dialogs |
| `crates/rc-core` | platform-neutral logic and tests; **no GUI dependencies** |
| `crates/rc-windows` | the Windows shell: Win32, WebView2, downloads, HTML chrome |
| `scripts/` | privacy probe checker, measurement harnesses |
| `docs/` | privacy, Windows, QA and release documents |

## Gates

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
cargo audit
```

CI runs these on ubuntu-22.04 and windows-2022. See [Contributing](Contributing).
