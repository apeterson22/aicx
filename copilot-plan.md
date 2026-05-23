Build AICX as the standalone Rust-first project.

AICX means Adaptive Intelligent Compression eXchange.

AICX is a deterministic archive and metadata engine for adaptive compression, content-aware classification, selective extraction, sidecar inspection, and future query/dedupe layers.

AegisQR is a separate repository and should not be implemented here. AICX should only expose archive files, hashes, manifests, and sidecars that AegisQR can later wrap, encrypt, sign, and transport.

Primary goal:
Create a production-ready Rust-first starter implementation of AICX with CLI, archive format, adaptive compression, deterministic manifests, hashing, sidecar metadata, selective extraction, tests, scenario validations, and documentation.

Python support should be delivered through PyO3 + maturin over the Rust core. The legacy Python prototype under `aicx/` remains as a reference only.

Core product goal:
AICX should become an AI-native intelligent archive format, not just another compression tool.

It should support:
- adaptive per-chunk compression
- deterministic archives
- content-aware classification
- semantic metadata sidecar
- agent-readable inspection
- selective extraction
- future deduplication
- future content-defined chunking
- future RAG/archive query support
- future AegisQR integration

Repository structure:

README.md
PLAN.md
SPEC.md
SECURITY.md
THREAT_MODEL.md
Cargo.toml
crates/
  aicx-core/
    tests/
  aicx-cli/
bindings/
  python/
examples/
.github/workflows/ci.yml

Use Rust for the production core.

Recommended crates:
- clap for CLI
- serde for serialization
- serde_json for compatibility/debug manifests
- serde_cbor for compact deterministic metadata
- zstd for default balanced/high-ratio compression
- lz4_flex or lz4 for fast compression
- xz2 for xz/lzma support if practical
- flate2 for gzip compatibility
- blake3 for fast hashing
- sha2 for SHA-256 compatibility
- walkdir for deterministic file walking
- thiserror for error handling
- anyhow only at CLI boundary
- rayon for future parallel compression
- memmap2 for future large archive reading
- tempfile for tests

MVP CLI commands:

aicx pack <input> --out <archive.aicx> --profile balanced
aicx unpack <archive.aicx> --out <dir>
aicx inspect <archive.aicx>
aicx list <archive.aicx>
aicx extract <archive.aicx> <path> --out <dir>
aicx verify <archive.aicx>
aicx report <archive.aicx>
aicx sidecar <archive.aicx> --format json
aicx sidecar <archive.aicx> --format toon
aicx compare-profiles <input>

Python-compatible API should be available through the binding layer:
- aicx.pack(...)
- aicx.unpack(...)
- aicx.inspect(...)
- aicx.extract(...)
- aicx.query_metadata(...)
- aicx.report(...)

Archive format:

Use extension:
.aicx

Magic bytes:
AICX1

MVP logical archive structure:
- magic
- version
- header length
- manifest + sidecar envelope
- chunk data section

The format must be versioned and deterministic.

Archive should support:
- single file input
- directory input
- deterministic file ordering
- path normalization
- path traversal protection
- safe restore behavior
- per-file hashing
- per-chunk hashing
- manifest validation
- sidecar inspection without full extraction
- selective extraction
- crate-local tests for round trips and archive hardening

Do not include encryption in AICX MVP.
AICX may add optional integrity and signatures later, but AegisQR owns encryption, signing, QR transfer, enterprise policy, and secure capsule behavior.
