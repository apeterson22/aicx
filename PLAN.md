# AICX Rust-First Build Plan

## Summary
AICX is the standalone Adaptive Intelligent Compression eXchange project. The production direction is a Rust-first archive engine with deterministic manifests, safe extraction, hashing, sidecars, and optional Python bindings via PyO3 + maturin.

AICX also defines the enterprise exchange contract for repository plugins such as Artifactory and Nexus. The integration boundary is documented in `plan-enterprise.md`; AegisQR remains the secure courier and policy layer that can later wrap, sign, encrypt, and transport AICX bundles.

## Implementation Direction
- Build a Rust workspace with a small core crate, a thin CLI crate, and a Python binding layer.
- Keep the legacy Python compatibility shim as a reference, not the production architecture.
- Make the archive format versioned, deterministic, and forward compatible.
- Enforce safe path handling, hash verification, and no-overwrite extraction defaults.
- Keep enterprise plugin integrations thin and avoid reimplementing archive parsing outside `aicx-core`.
- Keep the manifest and sidecar easily extensible for future chunking, dedupe, and query layers.

## Near-Term Scope
- Pack and unpack single files and directories.
- Deterministic file ordering and archive path normalization.
- Per-file and per-chunk hashing.
- Sidecar inspection, list, extract, verify, and report commands.
- Agent-facing manifest and sidecar digest helpers for downstream verification.
- Inspection and report payloads include deterministic manifest and sidecar digests.
- A digest-focused CLI command for scripts and agents that only need hashes.
- A shared `ArchiveDigests` payload type for CLI and Python hash-only consumers.
- Python `digest` and `digest_toon` helpers that mirror the CLI hash-only surface.
- Python support through the Rust core, not a separate Python implementation.
- Repository-plugin exchange support through `plan-enterprise.md`, using JSON over HTTPS with TOON for AI-facing projections.

## Future Extensions
- Content-defined chunking.
- Deduplication.
- Query and metadata search.
- More codec backends.
- AegisQR integration through exported metadata, not direct coupling.
- Artifactory/Nexus plugin contracts and job states defined in `plan-enterprise.md`.
