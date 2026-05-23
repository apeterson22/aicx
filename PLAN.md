# AICX Rust-First Build Plan

## Summary
AICX is the standalone Adaptive Intelligent Compression eXchange project. The production direction is a Rust-first archive engine with deterministic manifests, safe extraction, hashing, sidecars, and optional Python bindings via PyO3 + maturin.

AegisQR is a separate repository and is not implemented here. AICX only produces archives and metadata that AegisQR can later wrap, encrypt, sign, and transport.

## Implementation Direction
- Build a Rust workspace with a small core crate, a thin CLI crate, and a Python binding layer.
- Keep the legacy Python prototype as a reference, not the production architecture.
- Make the archive format versioned, deterministic, and forward compatible.
- Enforce safe path handling, hash verification, and no-overwrite extraction defaults.
- Keep the manifest and sidecar easily extensible for future chunking, dedupe, and query layers.

## Near-Term Scope
- Pack and unpack single files and directories.
- Deterministic file ordering and archive path normalization.
- Per-file and per-chunk hashing.
- Sidecar inspection, list, extract, verify, and report commands.
- Python support through the Rust core, not a separate Python implementation.

## Future Extensions
- Content-defined chunking.
- Deduplication.
- Query and metadata search.
- More codec backends.
- AegisQR integration through exported metadata, not direct coupling.

