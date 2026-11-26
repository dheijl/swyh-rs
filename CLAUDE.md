# swyh-rs — Claude Guidelines

Stream What You Hear: captures system audio (CPAL) and streams it over HTTP to
DLNA/UPnP renderers. Binaries: FLTK GUI (`gui` feature), headless CLI (`cli` feature).

## Working Rules

- **Rust care**: check ownership/borrowing before suggesting removal of `.clone()`/`.to_string()`; verify numeric types/bit widths; suggest `cargo clippy` when unsure.
- **No GUI testing**: never launch the FLTK GUI or screenshot it (no Xvfb), even mid-debugging. Make the change, verify it compiles, stop; if stuck, ask me what I see.
- **No git writes**: no `git add`/`commit`/`push` (I use GitHub Desktop). End with a suggested commit message.
- **Code review**: read files directly, no `gh`/PR workflows; one complete response, critical issues first.
- **Regression safety**: before refactoring behavior-carrying logic (CLI args, device selection, stream config), extract a pure function and unit-test current behavior.
- **Read narrowly**: search first, then read only the needed lines (offset/limit). Never read whole files over ~500 lines (e.g. `mainform.rs`, `rwstream.rs`, `swyh-rs-cli.rs`, `control.rs`, `CHANGELOG.md`) unless the task truly needs all of it.

## Verification (after every code change)

`cargo fmt` → `cargo build --all-features` → `cargo clippy --all-targets -- -D warnings` → `cargo test`.
Feature-gated code: also build `--no-default-features --features cli` and default `gui`.
Some clippy lints are nightly-only; ask which toolchain before chasing one.

## Conventions

OS threads + `crossbeam-channel` (no tokio/async-std) · `log`, not `tracing` ·
`anyhow` + `.context()`; `unwrap()` only on obvious invariants · `foo.rs` + `foo/`, not `mod.rs` ·
tests in `#[cfg(test)]`, benches with `criterion` in `benches/`.

## Read when relevant

- Module map / where code lives: `.claude/docs/architecture.md`
- Before touching CPAL sample rates, lexopt arg parsing, HTTP Range headers, or fluent/i18n: `.claude/docs/gotchas.md`
