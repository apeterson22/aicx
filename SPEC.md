# AICX Specification Notes

## Archive Shape

AICX archives use a versioned header followed by compressed chunk data. The header stores the manifest and sidecar metadata in a deterministic binary encoding.

## Required Behaviors

- Deterministic file ordering.
- Safe relative archive paths only.
- No path traversal, absolute path restore, or symlink escape during unpack.
- Per-file and per-chunk hashes for verification.
- Sidecar inspection without full extraction.

## Extensibility

Future versions may add content-defined chunking, deduplication, richer transforms, and additional codecs without breaking the current archive contract.

