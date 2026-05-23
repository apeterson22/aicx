"""Compatibility shim for the native AICX extension.

The legacy Python prototype has been retired. This module exists only
to preserve imports such as ``aicx.container.pack`` and
``aicx.container.unpack`` for callers that have not moved to
``aicx_native`` yet.
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
