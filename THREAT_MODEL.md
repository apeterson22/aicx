# Threat Model

Primary threats addressed by AICX:

- Malicious archive structure
- Path traversal and absolute-path restore attempts
- Symlink escape during extraction
- Decompression bombs and oversized headers
- Corrupted chunk data and manifest tampering
- Misleading metadata and spoofed archive contents
- Memory exhaustion from huge files or malformed sizes

Mitigations are centered on archive validation, safe path resolution, deterministic hashing, and explicit failure on suspicious inputs.

