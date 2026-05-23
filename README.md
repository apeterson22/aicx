# AICX

AICX, Adaptive Intelligent Compression eXchange, is a Rust-first archive format and tooling stack for deterministic packing, safe restoration, and agent-readable metadata.

## Layout

- `crates/aicx-core/` contains the archive model, hashing, classification, compression, validation, and pack/unpack logic.
- `crates/aicx-cli/` contains the command-line interface.
- `bindings/python/` contains the PyO3 + maturin Python binding layer.
- `aicx/` is the legacy Python prototype kept as a reference implementation while the Rust core matures.

## Development

```sh
cd aicx
cargo test
```

For Python bindings:

```sh
cd aicx/bindings/python
maturin develop
```

## Compatibility

AegisQR is a separate repository. AICX only produces deterministic archives, hashes, manifests, and sidecars that AegisQR can later consume and wrap.

