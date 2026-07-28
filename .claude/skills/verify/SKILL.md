---
name: verify
description: Run the full Rust verification loop and propose a commit message
---
Run in order, stopping on first failure:
1. `cargo fmt`
2. `cargo build --all-features`
3. `cargo clippy --all-targets -- -D warnings`
4. `cargo test`
5. `cargo build --no-default-features --features cli` and the GUI-only feature set

Do NOT run git add/commit/push. Print `git diff --stat` and a suggested
conventional-commit message for me to paste into GitHub Desktop.
