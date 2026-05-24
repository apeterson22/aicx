Build AICX as the standalone Rust-first project.

AICX means Adaptive Intelligent Compression eXchange.

AICX is a deterministic archive and metadata engine for adaptive compression, content-aware classification, selective extraction, sidecar inspection, and future query/dedupe layers.

AICX also defines the enterprise exchange contract for repository plugins such as Artifactory and Nexus. The shared integration plan lives in `plan-enterprise.md`. AegisQR is a separate repository and should not be implemented here; it can later wrap, sign, encrypt, and transport AICX bundles.

Primary goal:
Create a production-ready Rust-first starter implementation of AICX with CLI, archive format, adaptive compression, deterministic manifests, hashing, sidecar metadata, selective extraction, tests, scenario validations, and documentation.

Python support should be delivered through PyO3 + maturin over the Rust core. The legacy Python compatibility shim under `aicx/` is retained only for compatibility.

Core product goal:
AICX should become an AI-native intelligent archive format, not just another compression tool.

It should support:
- adaptive per-chunk compression
- deterministic archives
- content-aware classification
- semantic metadata sidecar
- agent-readable inspection
- agent-facing manifest and sidecar digest helpers
- serialized inspection/report payloads include manifest and sidecar digests
- a digest-focused CLI command for scripts and agents
- a shared `ArchiveDigests` payload type for hash-only consumers
- Python digest helpers that mirror the CLI hash-only surface
- Python digest_toon helper for TOON-oriented agent consumers
- selective extraction
- future deduplication
- future content-defined chunking
- future RAG/archive query support
- future AegisQR integration
- enterprise repository exchange support via thin plugin adapters and `plan-enterprise.md`

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
plan-enterprise.md

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
- aicx.sidecar(...)
- aicx.report(...)
- aicx.digest(...)

Archive format:

Use extension:
.aicx

Magic bytes:
AICX2

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

Enterprise integration rules:
- JSON is the canonical API payload format over HTTPS.
- TOON is allowed as a model-optimized alternate projection of the same payloads.
- SSH may be used for admin access, tunnels, and host automation, but not as the primary API transport.
- Repository plugins must stay thin and delegate archive correctness to AICX and transport policy to AegisQR.
