"""Compatibility shim for the native AICX extension.

The legacy Python compatibility shim exists only to preserve imports
such as ``aicx.container.pack``,
``aicx.container.unpack``, and ``aicx.container.digest`` for callers
that have not moved to ``aicx_native`` yet.
"""

from __future__ import annotations

from importlib import import_module
from typing import Any


def _native() -> Any:
    try:
        return import_module("aicx_native")
    except ModuleNotFoundError as exc:
        raise ImportError(
            "aicx_native is required for AICX archive operations; install the "
            "Rust Python binding or use the Rust CLI."
        ) from exc


def pack(*args: Any, **kwargs: Any) -> Any:
    return _native().pack(*args, **kwargs)


def unpack(*args: Any, **kwargs: Any) -> Any:
    return _native().unpack(*args, **kwargs)


def extract(*args: Any, **kwargs: Any) -> Any:
    return _native().extract(*args, **kwargs)


def inspect(*args: Any, **kwargs: Any) -> Any:
    return _native().inspect(*args, **kwargs)


def inspect_toon(*args: Any, **kwargs: Any) -> Any:
    return _native().inspect_toon(*args, **kwargs)


def report(*args: Any, **kwargs: Any) -> Any:
    return _native().report(*args, **kwargs)


def report_toon(*args: Any, **kwargs: Any) -> Any:
    return _native().report_toon(*args, **kwargs)


def sidecar(*args: Any, **kwargs: Any) -> Any:
    return _native().sidecar(*args, **kwargs)


def sidecar_toon(*args: Any, **kwargs: Any) -> Any:
    return _native().sidecar_toon(*args, **kwargs)


def verify(*args: Any, **kwargs: Any) -> Any:
    return _native().verify(*args, **kwargs)


def list_paths(*args: Any, **kwargs: Any) -> Any:
    return _native().list_paths(*args, **kwargs)


def compare_profiles(*args: Any, **kwargs: Any) -> Any:
    return _native().compare_profiles(*args, **kwargs)


def digest(*args: Any, **kwargs: Any) -> Any:
    return _native().digest(*args, **kwargs)


def digest_toon(*args: Any, **kwargs: Any) -> Any:
    return _native().digest_toon(*args, **kwargs)


def manifest_digest(*args: Any, **kwargs: Any) -> Any:
    return _native().manifest_digest(*args, **kwargs)


def sidecar_digest(*args: Any, **kwargs: Any) -> Any:
    return _native().sidecar_digest(*args, **kwargs)
