# Contributing to AICX

This document outlines the coding, contribution, security, and privacy standards specifically for the **AICX** repository. All developers and agents contributing to this project must follow these rules.

---

## 🛠️ Contribution Standards

* **PR Flow:** The remote `main` branch is protected. Direct pushes are blocked. All contributions must be submitted via a **Pull Request (PR)** and approved by a maintainer prior to merge.
* **Agent Handoff:** In the event of token exhaustion or agent transition, you must generate an `AGENT_HANDOFF.md` package and push your working progress to an `agent-handoff/` branch.

---

## 💻 Coding & Interface Standards

AICX is an enterprise-grade deterministic archiving and RAG context packaging engine. Code must be robust, safe, and highly performant:

### 1. Rust Coding Standards
* **Formatting:** Format all Rust files using `cargo fmt` before committing.
* **Lints:** Address all `cargo clippy` warnings. PRs with clippy warnings will be blocked.
* **Workspace Structure:**
  - `crates/aicx-core/`: Core archive model, deterministic blake3 cataloging, and validation logic.
  - `crates/aicx-cli/`: The command-line interface binary.
  - `bindings/python/`: PyO3 native bindings for Python integration.
* Keep workspace versions unified under the featured release tag (`"0.1.0-featured"`).

### 2. Python Binding Standards
* **Active Venvs:** Keep a dedicated local `.venv` active when building or running python scripts.
* **Maturin Workflow:** Compile native Python bindings using Maturin:
  ```bash
  cd bindings/python && maturin develop
  ```
* **Bypass System Packages:** Do not run `pip install --upgrade pip` on Homebrew-based Python. Pass `--break-system-packages` during global CI steps.
* **Patchelf:** Ensure `patchelf` is installed on your host system so Maturin can successfully rewrite RPATH bindings for native wheels.

### 3. CI/CD & Testing Guidelines
* **Sequential Testing:** Always run tests sequentially to avoid directory-overwriting race conditions:
  ```bash
  RUST_TEST_THREADS=1 cargo test
  ```
* **Runner Targeting:** The GitHub Actions workflow targets the `aegis-local` self-hosted runner.

---

## 🔗 The Sibling Integration Contract (AegisQR Coupling)

AICX and `AegisQR` communicate **strictly via decoupled command-line execution and metadata sidecars**:
* **Decoupled Crate Rule:** Do not introduce Cargo-level crate dependencies between AICX and AegisQR.
* **Deterministic Sidecars:** Always generate structured companion sidecars (`.sidecar.json` or `.sidecar.cbor`) detailing `archive_digest`, `manifest_digest`, `sku_index`, and `scoutai_context` risk indices.
* **Version-Locked JSON:** Mandate version-locked, stable JSON structures for `inspect`, `verify`, `digest`, and `license status` subcommands to prevent parsing breaks in parent child-processes.

---

## 🛡️ Security & Privacy Standards

* **Path-Traversal Block:** Ensure absolute blocking of `..` segments or root pathways in tar extraction.
* **Symlink Restriction:** Reject symlinks and hardlinks in packed archives to prevent host path redirection exploits.
* **Zero Telemetry:** AICX collects zero telemetry and performs zero network calls. Licensing checking (`.aqlic`) executes entirely offline.
* **Shared Config Paths:** Respect shared config directories in `/etc/aegisqr/license.aqlic` and `/etc/aegisqr/trusted_keys.d/` rotations.
