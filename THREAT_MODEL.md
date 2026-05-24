# Threat Model

Primary threats addressed by AICX:

- Malicious archive structure
- Path traversal and absolute-path restore attempts
- Relative output-path normalization bugs during pack
- Intermediate symlink redirection during pack
- Special-file overwrite targets during pack
- Hard-linked overwrite targets during pack
- Unsafe archive output filenames such as `.` and `..`
- Non-UTF8 archive output filenames
- Partial directory creation on failed pack attempts
- Partial file writes from interrupted or failed restore attempts
- Filesystem-root pack and unpack destinations
- Intermediate symlink redirection during unpack
- Special-file overwrite targets during unpack
- Hard-linked overwrite targets during unpack
- Symlinked archive output paths
- Symlinked extraction roots
- Symlink escape during extraction
- Unsafe original-path metadata carrying traversal syntax, raw unnormalized paths, or other non-canonical forms
- Non-UTF8 input paths, including nested directory entries, that could otherwise be lossy-encoded into ambiguous metadata
- Absolute file inputs leaking filesystem prefixes into source metadata, and relative single-file inputs losing normalized directory context
- Decompression bombs and oversized headers
- Corrupted chunk data, manifest tampering, and non-contiguous chunk layouts
- Misleading metadata and spoofed archive contents
- Tampered advisory sidecar metadata
- Incorrect or missing executable-bit restoration
- Memory exhaustion from huge files or malformed sizes

Mitigations are centered on archive validation, safe path resolution, deterministic hashing, exact chunk table coverage, sidecar consistency checks, atomic temp-file persistence, executable-hint restoration, and explicit failure on suspicious inputs.
