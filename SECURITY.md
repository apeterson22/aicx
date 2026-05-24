# Security Notes

AICX treats every archive as untrusted input.

- Validate header lengths and manifest tables before reading chunk data.
- Reject malformed offsets, lengths, non-contiguous chunk layouts, duplicate output paths, unsafe restore targets, symlinked archive output paths, symlinked extraction roots, hard-linked overwrite targets, and non-regular overwrite targets.
- Normalize archive output parents before directory creation so `..` segments cannot create the wrong directory tree.
- Canonicalize the final pack output parent before writing the archive so intermediate symlink hops cannot redirect output.
- Reject unsafe archive output filenames such as `.` and `..` before writing.
- Require UTF-8 archive output filenames so pack destinations stay portable and deterministic.
- Validate pack output filenames before any directory creation so failed writes do not leave partial directories behind.
- Stage pack and unpack output through temp files in the destination directory, then persist atomically so failed restores do not leave partial files behind.
- Restore executable hints from the manifest after validation, using a conservative mode policy on unpacked files.
- Reject filesystem-root pack and unpack destinations.
- Canonicalize the unpack target root after normalizing its path so intermediate symlink hops cannot redirect extraction.
- Refuse symlink escapes, hard-link escapes, special-file overwrite targets, and file overwrites unless explicitly enabled.
- Reject non-UTF-8 input paths before packing so archive metadata stays deterministic and machine-readable, including nested directory entries.
- Normalize preserved source-path metadata and keep directory entries archive-relative so manifests retain safe, canonicalized original-path strings instead of raw traversal input.
- Preserve canonical original-path metadata on pack and reject non-canonical original-path metadata on load. Relative single-file inputs keep their normalized relative path; absolute single-file inputs keep only the basename.
- Verify chunk and file hashes, plus manifest size totals and sidecar consistency, before writing restored output.
- Rebuild and compare the full sidecar deterministically so advisory-only metadata cannot be altered without detection.

AICX does not perform encryption or execution. Those responsibilities belong to AegisQR in its separate repository.

Enterprise plugin integrations must use HTTPS and authenticated tokens for API traffic. SSH is reserved for admin access, tunneling, or host automation and must not bypass archive validation or repository authorization.
