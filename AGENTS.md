# Repository Guidelines

## Scope
This guide applies to the entire `aicx/` repository root. Read the more specific guides in `crates/`, `bindings/`, and `aicx/` before editing those trees.

## Project Structure
- `crates/aicx-core/` owns archive models, chunking, classification, transforms, hashing, validation, and pack/unpack logic.
- `crates/aicx-cli/` owns the Rust CLI and command wiring.
- `bindings/python/` owns the PyO3 + maturin Python bindings.
- `aicx/` is the legacy Python compatibility layer and reference implementation; it should prefer `aicx_native` when available.
- `PLAN.md` and `copilot-plan.md` are the planning sources of truth for implementation direction.

## Commands
- Run Rust checks from the repo root with `cargo test`.
- Build the CLI with `cargo run -p aicx-cli -- --help`.
- Develop Python bindings with `cd bindings/python && maturin develop`.
- Keep root docs and metadata in sync with the current crate layout before merging.

## Style
- Favor small, composable crates and explicit module boundaries.
- Keep archive formats deterministic and versioned.
- Preserve safe path handling, hash verification, and no-overwrite defaults.

## Security
- Treat all archive inputs as hostile.
- Do not weaken validation, path normalization, or hash checks for convenience.
- Keep AegisQR implementation out of this repository; only preserve integration boundaries.

## Agent Workflow
- Work from the repo root inside `aicx/`.
- Do not overwrite unrelated edits from other agents or users.
- Update plan docs when architecture or public behavior changes.
