from __future__ import annotations

import importlib.util
import sys
import types


def _module_available(module_name: str) -> bool:
    try:
        return importlib.util.find_spec(module_name) is not None
    except (ModuleNotFoundError, ValueError):
        return False


def _install_lz4_stub() -> None:
    if _module_available("lz4.frame"):
        return

    frame = types.ModuleType("lz4.frame")

    def compress(data: bytes, compression_level: int = 0) -> bytes:
        return b"LZ4" + data

    def decompress(data: bytes) -> bytes:
        if not data.startswith(b"LZ4"):
            raise ValueError("invalid lz4 frame")
        return data[3:]

    frame.compress = compress  # type: ignore[attr-defined]
    frame.decompress = decompress  # type: ignore[attr-defined]

    package = types.ModuleType("lz4")
    package.frame = frame  # type: ignore[attr-defined]

    sys.modules.setdefault("lz4", package)
    sys.modules.setdefault("lz4.frame", frame)


def _install_zstandard_stub() -> None:
    if _module_available("zstandard"):
        return

    module = types.ModuleType("zstandard")

    class ZstdCompressor:
        def __init__(self, level: int = 3) -> None:
            self.level = level

        def compress(self, data: bytes) -> bytes:
            return b"ZST" + data

    class ZstdDecompressor:
        def decompress(self, data: bytes) -> bytes:
            if not data.startswith(b"ZST"):
                raise ValueError("invalid zstandard frame")
            return data[3:]

    module.ZstdCompressor = ZstdCompressor  # type: ignore[attr-defined]
    module.ZstdDecompressor = ZstdDecompressor  # type: ignore[attr-defined]

    sys.modules.setdefault("zstandard", module)


_install_lz4_stub()
_install_zstandard_stub()
