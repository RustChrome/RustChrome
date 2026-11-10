# Contributing

Canonical version: [`CONTRIBUTING.md`](https://github.com/RustChrome/RustChrome/blob/main/CONTRIBUTING.md).

## Before you write code

* For anything larger than a bug fix, open an issue or a discussion first.
* Read [Known Issues](Known-Issues) and
  [`docs/PRIVACY.md`](https://github.com/RustChrome/RustChrome/blob/main/docs/PRIVACY.md).

## Gates, none optional

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
cargo audit
```

## Code rules

* No new dependency without a reason in the pull request.
* **No telemetry, ever.** No analytics, no crash reporting, no update pings.
* No new network traffic at startup.
* No fake functionality: no dead buttons, no settings nothing reads, no privacy
  claim the engine cannot back.
* Keep the engine honest — no lying user agent.

## Privacy changes

Anything touching fingerprinting, permissions, storage or cleanup needs a line in
`docs/PRIVACY.md`, a probe check if it is observable from a page, and proof that
ordinary sites still work.

## Reporting bugs

Use the [bug template](https://github.com/RustChrome/RustChrome/issues/new/choose).
Include platform, version, the URL that broke and exact steps — and whether it also
happens in Firefox or Chrome.
