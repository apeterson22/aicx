# Copilot Instructions

Read the most specific `AGENTS.md` before editing code in `crates/`, `bindings/`, or `aicx/`.

## Build and test commands

- Run the full Rust workspace tests from the repo root with `cargo test --workspace`.
- Check Rust formatting with `cargo fmt --all -- --check`.
- Run the Rust linter with `cargo clippy --workspace --all-targets -- -D warnings`.
- Run Rust tests for one crate with `cargo test -p aicx-core` or `cargo test -p aicx-cli`.
- Run a single Rust test with `cargo test -p aicx-core pack_and_unpack_round_trip` or `cargo test -p aicx-cli parses_digest_subcommand`.
- Smoke-test the CLI with `cargo run -p aicx-cli -- --help`.
- Build the Python extension locally with `cd bindings/python && maturin develop`.
- Build the Python wheel the same way CI does with `cd bindings/python && maturin build --release`.
- Run the Python shim tests from the repo root with `python3 -m pytest`.
- Run a single Python test with `python3 -m pytest tests/unit/test_native_bridge.py::test_compatibility_shim_prefers_native_extension`.

## High-level architecture

- `crates/aicx-core/` is the source of truth for archive behavior. `src/archive.rs` orchestrates pack, inspect, verify, report, digest, list, compare, and extract by composing the chunking, classification, compression, transform, pathing, sidecar, and model modules.
- The packed file format is an `ArchiveEnvelope` plus payload data: the manifest and sidecar live in the envelope, and pack/write plus read/validate logic stays centralized in `aicx-core/src/archive.rs`.
- The pack path is: gather source files -> normalize archive paths -> classify content -> chunk -> apply transforms -> choose a codec -> write the archive -> build sidecar metadata and deterministic digests.
- The read path is: read archive -> validate manifest, chunk layout, hashes, and sidecar consistency -> expose metadata through `inspect` / `report` / `digest` or restore files through `extract_archive`.
- `aicx-core/src/sidecar.rs` derives agent-facing hints from the manifest, including file type counts, codec/transform usage, entrypoint candidates, dependency hints, extraction maps, and AegisQR integration hints. `build_sidecar()` is the canonical deterministic sidecar builder, and validation expects complete sidecars that exactly match the rebuilt form.
- `crates/aicx-cli/` is intentionally thin: it parses Clap subcommands and delegates directly to `aicx_core`.
- `bindings/python/src/lib.rs` is also a thin layer over `aicx_core`, exposing the same archive operations plus TOON renderers and digest helpers through the `aicx_native` module.
- The top-level `aicx/` package is a compatibility shim only. It forwards legacy imports such as `aicx.container.pack` to `aicx_native` and should not regain archive logic.
- `inspect`, `report`, `sidecar`, and `digest` are the main machine-facing metadata surfaces. `inspect` and `report` include deterministic `manifest_digest` and `sidecar_digest`, while `digest` is the hash-only surface for automation.

## Key conventions

- Keep security-sensitive archive behavior in `aicx-core`; the CLI, Python bindings, and any future UI should stay thin and reuse core logic instead of reimplementing archive parsing, validation, or extraction.
- Preserve deterministic outputs. The core uses sorted archive paths and deterministic collections like `BTreeMap`, and tests assert deterministic manifests and digest values.
- Treat archive inputs and extraction targets as hostile. Preserve path normalization, traversal rejection, symlink checks, hash verification, and no-overwrite defaults unless a change explicitly requires different behavior.
- Prefer the Rust-first Python path: new Python-facing features belong in `bindings/python/` and the legacy `aicx/` package should only forward to `aicx_native`.
- Keep `PLAN.md`, `copilot-plan.md`, and `plan-enterprise.md` aligned with architecture or public-contract changes; those files are treated as active design references for the project and AegisQR integration.
- For hash-only or agent-facing workflows, prefer `digest` / `digest_toon` over fuller metadata surfaces when only deterministic manifest and sidecar hashes are needed.
