## What this changes

<!-- One or two sentences. What behaviour is different after this patch? -->

## Why

<!-- The problem. If it fixes an issue, link it. -->

## How it was tested

<!-- "cargo test" alone is not enough if behaviour changed. Say what you ran,
     what you clicked, and what you saw. If a platform was not tested, say so. -->

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] Linux build and launch
- [ ] Windows build and launch

## Checks

- [ ] No new dependency (or the reason is explained above)
- [ ] No telemetry, no new network traffic at startup
- [ ] No new privacy claim that the engine cannot back
- [ ] `docs/PRIVACY.md` updated if fingerprinting, permissions, storage or cleanup changed
- [ ] No feature that is decorative, placeholder or permanently disabled
