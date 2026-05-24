# Repository Guidelines

## Scope
This guide applies to the entire `aicx/` repository root. Read the more specific guides in `crates/`, `bindings/`, and `aicx/` before editing those trees.

## Project Structure
- `crates/aicx-core/` owns archive models, chunking, classification, transforms, hashing, validation, and pack/unpack logic.
- `crates/aicx-cli/` owns the Rust CLI and command wiring.
- `bindings/python/` owns the PyO3 + maturin Python bindings.
- `aicx/` is a minimal Python compatibility shim that forwards to `aicx_native`.
- `PLAN.md`, `copilot-plan.md`, and `plan-enterprise.md` are the planning sources of truth for implementation direction.
- `inspect`, `report`, and `digest` are the agent-facing metadata surfaces; they include deterministic manifest and sidecar digests.
- The Python bindings expose `extract`, `digest`, and `digest_toon` alongside `inspect`, `report`, `sidecar`, and the individual digest helpers; prefer them for hash-only agent workflows and TOON-friendly consumers.

## Commands
- Run Rust checks from the repo root with `cargo test`.
- Check Rust formatting with `cargo fmt --all -- --check`.
- Run the Rust linter with `cargo clippy --workspace --all-targets -- -D warnings`.
- Build the CLI with `cargo run -p aicx-cli -- --help`.
- Use `cargo run -p aicx-cli -- digest archive.aicx` for hash-only machine-readable output.
- Develop Python bindings with `cd bindings/python && maturin develop`.
- Keep root docs and metadata in sync with the current crate layout before merging.
- Keep CodeQL and Dependabot workflows aligned with the active crate and binding set.
- Keep enterprise plugin and AegisQR contract docs aligned with the current API shape.

## Style
- Favor small, composable crates and explicit module boundaries.
- Keep archive formats deterministic and versioned.
- Preserve safe path handling, hash verification, and no-overwrite defaults.

## Security
- Treat all archive inputs as hostile.
- Treat archive outputs as no-overwrite by default.
- Do not weaken validation, path normalization, or hash checks for convenience.
- Keep any UI or downstream consumer thin and backed by `aicx-core` rather than reimplementing archive logic.

## Agent Workflow
- Work from the repo root inside `aicx/`.
- Do not overwrite unrelated edits from other agents or users.
- Update plan docs when architecture or public behavior changes.
