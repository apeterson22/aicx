"""Compatibility package for the native AICX extension."""

from .container import (
    compare_profiles,
    extract,
    digest,
    digest_toon,
    inspect,
    inspect_toon,
    list_paths,
    manifest_digest,
    pack,
    report,
    report_toon,
    sidecar,
    sidecar_digest,
    sidecar_toon,
    unpack,
    verify,
)

__all__ = [
    "compare_profiles",
    "extract",
    "digest",
    "digest_toon",
    "inspect",
    "inspect_toon",
    "list_paths",
    "manifest_digest",
    "pack",
    "report",
    "report_toon",
    "sidecar",
    "sidecar_digest",
    "sidecar_toon",
    "unpack",
    "verify",
]
