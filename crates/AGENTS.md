# Crates Guidelines

## Scope
This guide applies to everything under `crates/`.

## Project Structure
- Keep `aicx-core` focused on archive data structures and low-level operations.
- Keep `aicx-cli` thin: parse arguments, call into core, and render results.
- Add future crates only when they reduce coupling or isolate optional behavior.

## Commands
- Run crate tests from the repo root with `cargo test -p aicx-core` or `cargo test -p aicx-cli`.
- Use `cargo fmt` only if the Rust toolchain is installed and the change is ready to land.

## Style
- Prefer explicit error types and `Result`-based APIs.
- Keep public types serializable and stable across versions.
- Use deterministic collections such as `BTreeMap` where order matters.

## Security
- Validate all file and archive paths before filesystem writes.
- Verify chunk hashes before restoration.
- Reject malformed archive metadata early.

