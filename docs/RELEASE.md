# Release process

## What a release is

A git tag on `main` and a GitHub release built by CI from that commit. Source
lives in the repository; binaries live only as release assets. Nothing generated
is ever committed — `dist/`, `rel*/`, `dist_qa*/`, `fmt_out/` and `.wiki-work/`
are ignored.

## Steps

1. **Version.** Bump `version` in `Cargo.toml`, `crates/rc-core/Cargo.toml` and
   `crates/rc-windows/Cargo.toml`, and `VERSION` in `.github/workflows/ci.yml`.
   All four must agree or the artifact names lie.
2. **Notes.** Write `docs/releases/<version>.md`. Say what changed, what is
   broken, and that a beta is not stable.
3. **Green.** Wait for both workflows on the commit being released: `CI` (build,
   smoke, privacy probe, packaging) and `QA` (fmt, clippy `-D warnings`, tests,
   `cargo audit`, measurement, compatibility).
4. **Artifacts.** Download the run's artifacts. They are built from the release
   commit, not from a developer machine.
5. **Verify.** `sha256sum -c` both `.sha256` files, on Linux, in the directory the
   archives live in.
6. **Draft.** `gh release create <tag> --draft --prerelease --notes-file …` with
   the four assets.
7. **Confirm the tag target.** A draft targets a branch name, not a SHA. Make sure
   `main` already points at the release commit before publishing, or the tag will
   land on older code.
8. **Publish** only on the maintainer's explicit approval.

## Verification that a download matches the source

```bash
# after downloading the release assets
sha256sum -c rustchrome-<version>-x86_64-linux.tar.gz.sha256
sha256sum -c rustchrome-<version>-x86_64-windows.zip.sha256
```

The checksum file is written with LF endings by CI, because a CRLF file fails
`sha256sum -c` on Linux — that mistake shipped once and is now checked by hand on
every release.

To tie a binary to a commit: the release tag points at a commit on `main`, and the
assets are that commit's CI artifacts. Anyone can rebuild from the tag and compare
behaviour, though not bit-for-bit: neither the MSVC nor the GNU toolchain here is
pinned to a reproducible build, and this project does **not** claim reproducible
builds.

## Checks before publishing

- [ ] `main` HEAD is the commit the artifacts were built from
- [ ] Both workflows green on that commit
- [ ] `docs/QA.md` updated with anything newly broken or fixed
- [ ] Release notes state known bugs and that it is not stable
- [ ] Checksums verified after download
- [ ] `v0.1.0` and any earlier tags untouched
