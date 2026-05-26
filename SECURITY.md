# Security Policy: AICX

This document outlines the core security practices, threat assumptions, and path validation safety rules for the **AICX** deterministic archiving and semantic packaging engine.

---

## 🛡️ Secure Defaults

AICX enforces strict safety constraints to insulate system hosts and AI agents from adversarial archive structures:

* **Untrusted Input Assumption:** Every archive is treated as potentially hostile input.
* **Deterministic Hashing:** All archive contents are strictly cataloged using deterministic **Blake3** hashing.
* **Path-Traversal Block:** Unsafe parent directory escapes (`..` injections) or filesystem root pathways are strictly blocked at the library core. Unpack directories are canonicalized and isolated.
* **Redirection Protection:** Symlinks and hardlinks are completely rejected during packing and extraction to prevent host path-redirection exploits.
* **Atomic Restores:** File and directory extractions are staged through temporary files in the destination directory, then persisted atomically so failed restores do not leave partial or corrupt states.
* **Validation Prior to Restore:** Chunk hashes, manifest totals, and sidecar structures are fully validated *prior* to writing any restored output.

---

## ⚡ Threat Assumptions

Our design operates under the following threat model constraints:
1. **Adversarial Ingestion:** Attackers may inject malicious symbolic links or path-escapes inside catalog directories, trying to overwrite critical system configurations.
2. **Context Alteration:** Advisory-only metadata could be modified to deceive autonomous AI agents. AICX resolves this by rebuilding and verifying sidecars deterministically.
3. **Host Security:** Unpacking or inspecting archives must never execute binary payloads. Executable modes are quarantined or staged under conservative local policies.

---

## 🔒 Privacy & Offline Commitment

AICX is committed to **100% user privacy**:
* **Zero Telemetry:** The tool contains no analytics, telemetry scripts, or network-bound call-homes.
* **Offline Licensing:** License status checks and `.aqlic` validations execute entirely locally against offline verification keys. Shared config locations `/etc/aegisqr/` are used to sync license states with AegisQR.

---

## 🐛 Vulnerability Disclosure

If you identify a security issue, please do not open a public issue. Report it privately to the maintainers or utilize GitHub's Private Vulnerability Reporting features to coordinate a fix.
