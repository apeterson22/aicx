# AICX

AICX, Adaptive Intelligent Compression eXchange, is a Rust-first archive engine for deterministic packing, safe unpacking, and machine-readable metadata. It provides:

- a Rust core in `crates/aicx-core/`
- a CLI in `crates/aicx-cli/`
- Python bindings in `bindings/python/`

The top-level `aicx/` package is legacy compatibility code. New work should use the Rust crates or the native Python binding module.

## When to Use AICX

Use AICX when you need to:

- package files into a reproducible archive
- inspect or verify an archive before extraction
- restore files with path validation and no-overwrite defaults
- emit sidecar metadata for downstream tooling or agent pipelines
- call the archive engine from Rust or Python
- feed a downstream workflow such as [AegisQR](https://github.com/apeterson22/AegisQR) with deterministic archives and metadata

Do not use AICX as an encryption layer. AegisQR is a separate repository that can consume AICX archives, manifests, and sidecars.

## Where To Use It

- **CLI automation:** local shells, scripts, and CI jobs that need archive creation or extraction.
- **Rust projects:** application code that wants direct access to the archive engine via `aicx-core`.
- **Python applications:** code under `bindings/python/` that imports `aicx_native`.
- **Legacy compatibility only:** the root `aicx/` Python package, which now prefers `aicx_native` when available.

## Quick Start

From the repository root:

```sh
cargo test --workspace
cargo run -p aicx-cli -- --help
```

Build the Python extension locally:

```sh
cd bindings/python
maturin develop
```

## CLI Usage

Pack inputs into an archive:

```sh
cargo run -p aicx-cli -- pack ./data ./docs -o archive.aicx --profile balanced --chunk-size 4194304 --hash blake3
```

Unpack safely into a target directory:

```sh
cargo run -p aicx-cli -- unpack archive.aicx --out restored
```

Inspect, verify, and review metadata:

```sh
cargo run -p aicx-cli -- inspect archive.aicx
cargo run -p aicx-cli -- verify archive.aicx
cargo run -p aicx-cli -- sidecar archive.aicx
cargo run -p aicx-cli -- list archive.aicx
```

Use `extract` to restore a single path and `compare-profiles` to compare archive profiles across inputs.

## Python Usage

After `maturin develop`, import the native module:

```python
import json
from aicx_native import pack, unpack, inspect, verify, list_paths

manifest = json.loads(pack(["./data"], "archive.aicx", "balanced", 4_194_304, "blake3"))
summary = json.loads(inspect("archive.aicx"))
paths = list_paths("archive.aicx")
```

The native module is the supported Python interface. The legacy `aicx` package remains for compatibility and will prefer `aicx_native` when installed.

## Repository Layout

- `crates/aicx-core/`: archive model, validation, hashing, safe paths, and pack/unpack logic.
- `crates/aicx-cli/`: command-line entry points.
- `bindings/python/`: PyO3/maturin bindings for Python callers.
- `aicx/`: legacy Python reference and compatibility layer.
- `.github/workflows/ci.yml`: CI that runs `cargo test --workspace` and builds the Python bindings.
- Any future UI should be a thin Rust frontend over `aicx-core`; do not duplicate archive validation or extraction logic.

## Safety Notes

- Treat archive inputs as untrusted.
- Keep extraction inside a controlled directory.
- Do not disable hash or path validation for convenience.
- Prefer the Rust core for new features and security-sensitive behavior.
