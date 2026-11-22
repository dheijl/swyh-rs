# swyh-rs — Claude Guidelines

## Working Rules

- **Rust care**: validate suggestions against ownership/borrowing before recommending removal of `.clone()`, `.to_string()`, etc.; verify numeric types and bit widths before flagging values; suggest `cargo clippy` when unsure.
- **No GUI testing**: never launch or drive the FLTK GUI. Make the change, verify it compiles, then stop and let me test.
- **No git writes**: never run `git add`, `git commit`, or `git push` (I use GitHub Desktop). End with a suggested commit message.
- **Permissions**: each session starts in manual (ask-before-acting) mode; approvals don't carry over between sessions.
- **Code review**: read the file directly — no `gh`, PR numbers, or PR workflows. Deliver the complete review in one response, critical issues first.
- **Regression safety**: before optimizing/refactoring behavior-carrying logic (CLI arg parsing, device selection, stream config), extract a pure function and add unit tests for current behavior.

## Verification (after every code change)

```sh
cargo fmt
cargo build --all-features
cargo clippy --all-targets -- -D warnings
cargo test
```

When touching feature-gated code, also build `--no-default-features --features cli` and the default `gui` set. Some referenced clippy lints are nightly-only — ask which toolchain before chasing a warning.

## Project Overview

"Stream What You Hear": captures audio from the default output device (CPAL/WASAPI loopback on Windows, ALSA/PipeWire on Linux) and streams it over HTTP to DLNA/UPnP renderers discovered via SSDP. GUI binary (FLTK, `gui` feature) and headless CLI binary (`cli` feature).

- `src/audio/` — CPAL capture, sample conversion, FLAC/WAV/MP3 encoding
- `src/server/` — HTTP streaming server
- `src/rendercontrol/` — SSDP discovery, UPnP AV/OpenHome control
- `src/ui/mainform.rs` — FLTK tabbed GUI (Audio, Network, App, Status)
- `src/utils/configuration.rs` — INI config via `serde`
- `src/bin/swyh-rs-cli.rs` — CLI, args via `lexopt`
- `src/utils/i18n.rs` — `fluent-templates`; `locales/` holds `.ftl` files

## Conventions

- OS threads + `crossbeam-channel`; do **not** introduce `tokio`/`async-std`.
- Logging via the `log` crate, not `tracing`.
- Errors via `anyhow` with `.context()`; `unwrap()` only where the invariant is obvious (after `is_some()`, `Mutex::lock()`).
- Modules: new-style `foo.rs` + `foo/` — not `foo/mod.rs`.
- Unit tests in `#[cfg(test)]` modules; benchmarks with `criterion` in `benches/`.

## Gotchas

- **CPAL `SampleRate`** is a plain `u32` alias, not a newtype — never use `.0`.
- **`lexopt` value-swallowing**: an optional-value arg only takes the "no value" branch when it's the last token; otherwise the next token is consumed as its value, even if it looks like a flag.
- **Accept-Ranges and MPD**: only emit `Accept-Ranges` on `206` and `416` responses, never on `200` — MPD loops forever on GET otherwise.
- **fluent-templates**: always `set_use_isolating(false)` in `static_loader!`, or FSI/PDI bidi chars show as garbage in FLTK labels on Windows.
