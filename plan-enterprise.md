# AICX ↔ AegisQR Enterprise Integration Plan

## Summary
AICX is the artifact packaging and validation layer. AegisQR is the secure courier and policy layer. Artifactory and Nexus plugins should stay thin: they expose repository actions, invoke AICX to build or inspect bundles, then hand those bundles to AegisQR for sealing, transport, approval, and release.

JSON over HTTPS is the canonical integration protocol. TOON is an optional AI-optimized projection of the same payloads for model-facing consumers. SSH is allowed for operational access, tunneling, and admin workflows, but never as a replacement for authenticated HTTPS API traffic.

## Plugin API
Base path: `/api/plugins/aicx/v1`

- `GET /capabilities`
  - Returns supported package types, checksum algorithms, compression profiles, transports, auth modes, and TOON support.
- `POST /exports`
  - Starts an async export job from repository artifacts into an AICX bundle.
- `GET /jobs/{job_id}`
  - Returns job state, progress, timestamps, errors, and next action.
- `POST /jobs/{job_id}/approve`
  - Approves a queued handoff before AegisQR sealing or repository import.
- `POST /jobs/{job_id}/cancel`
  - Cancels a queued or active job.
- `POST /imports`
  - Starts an async import job from an AegisQR-delivered capsule into the repository.
- `POST /imports/{job_id}/finalize`
  - Commits validated artifacts into the target repository.

### Transport and auth
- All API traffic uses HTTPS.
- Default auth is bearer tokens, PATs, or scoped service tokens.
- SSH is reserved for admin access, trusted tunnel endpoints, and scripted host operations.
- No anonymous, plaintext, or direct filesystem handoff is allowed.

## AICX Bundle Contract
All bundle payloads must be representable as JSON or TOON with identical semantics.

Required envelope fields:
- `bundle_id`
- `format`
- `format_version`
- `serialization_profile` (`json` or `toon`)
- `created_at`
- `created_by`
- `source_system`
- `source_repository`
- `target_system`
- `target_repository`
- `package_type`
- `artifact_count`
- `hash_algorithm`
- `compression_profile`
- `manifest_digest`
- `payload_digest`
- `sidecar_digest`
- `artifacts[]`
- `sidecar`
- `provenance`

Each artifact record must include:
- `artifact_id`
- `coordinates`
- `repository_path`
- `package_type`
- `version`
- `classifier`
- `size_bytes`
- `digests` (at minimum `sha256` and/or `blake3`)
- `labels`
- `dependencies`
- `chunk_refs`
- `restore_hint`

The bundle is not encrypted. It is a validated exchange envelope that AegisQR may seal, sign, encrypt, encode, and transport.

## AegisQR Handoff States
State ownership is split so the plugin and AegisQR do not duplicate responsibility.

- **AICX-owned:** `queued` → `collected` → `bundled` → `validated`
- **AegisQR-owned:** `sealed` → `signed` → `encrypted` → `encoded` → `transferred` → `received` → `verified` → `approved` → `decrypted` → `released` → `restored` → `completed`
- **Terminal failure states:** `failed`, `cancelled`, `expired`

Rules:
- AICX must validate manifest integrity and safe paths before handoff.
- AegisQR must verify signatures, encryption state, and approval before release.
- Import into Artifactory or Nexus only occurs after AegisQR reaches `verified` and `approved`.

## Assumptions
- JSON remains the source-of-truth wire format; TOON is a lossless model-facing projection.
- The plugin is a thin adapter over repository APIs and AICX/AegisQR contracts.
- AICX owns archive correctness; AegisQR owns transport security and policy enforcement.
- This plan should be mirrored into the AegisQR repo so Copilot agents implement the same contract from the other side.
