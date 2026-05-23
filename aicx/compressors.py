"""Compression codec wrappers for AICX.

This module defines a simple interface for compression algorithms.  Each
compressor must implement ``compress(data: bytes) -> bytes`` and
``decompress(data: bytes) -> bytes``.  Additional metadata such as
compression level can be configured when instantiating a compressor.

Codecs included in this prototype:

* **lz4** – via `lz4.frame` for very fast compression/decompression.
* **zstd** – via `zstandard` for balanced compression; supports
  configurable levels.
* **gzip** – via Python’s standard `gzip` module for broad
  compatibility.
* **xz** – via Python’s `lzma` module (LZMA) for high compression ratios.

The compressor registry at the bottom of the file allows the
``CodecSelector`` to instantiate compressors by name.
"""

from __future__ import annotations

import gzip
import io
import lzma
from dataclasses import dataclass
from typing import Dict

try:  # pragma: no cover - exercised indirectly when optional deps exist
    import lz4.frame as lz4_frame  # type: ignore
except ModuleNotFoundError:  # pragma: no cover - fallback for native installs
    lz4_frame = None

try:  # pragma: no cover - exercised indirectly when optional deps exist
    import zstandard as zstd  # type: ignore
except ModuleNotFoundError:  # pragma: no cover - fallback for native installs
    zstd = None


class Compressor:
    """Abstract compressor interface."""

    name: str

    def compress(self, data: bytes) -> bytes:
        raise NotImplementedError

    def decompress(self, data: bytes) -> bytes:
        raise NotImplementedError


@dataclass
class LZ4Compressor(Compressor):
    """LZ4 compressor using the lz4.frame API."""

    name: str = "lz4"
    compression_level: int = 0  # 0 auto, 1 fast, 12 max

    def compress(self, data: bytes) -> bytes:
        if lz4_frame is None:
            raise RuntimeError("lz4 support is not available")
        return lz4_frame.compress(data, compression_level=self.compression_level)

    def decompress(self, data: bytes) -> bytes:
        if lz4_frame is None:
            raise RuntimeError("lz4 support is not available")
        return lz4_frame.decompress(data)


@dataclass
class ZstdCompressor(Compressor):
    """Zstandard compressor wrapper."""

    name: str = "zstd"
    level: int = 3

    def __post_init__(self) -> None:
        if zstd is None:
            raise RuntimeError("zstandard support is not available")
        self._cctx = zstd.ZstdCompressor(level=self.level)
        self._dctx = zstd.ZstdDecompressor()

    def compress(self, data: bytes) -> bytes:
        return self._cctx.compress(data)

    def decompress(self, data: bytes) -> bytes:
        return self._dctx.decompress(data)


@dataclass
class GzipCompressor(Compressor):
    """Gzip compressor for broad compatibility (DEFLATE)."""

    name: str = "gzip"
    level: int = 6

    def compress(self, data: bytes) -> bytes:
        buf = io.BytesIO()
        with gzip.GzipFile(fileobj=buf, mode="wb", compresslevel=self.level) as f:
            f.write(data)
        return buf.getvalue()

    def decompress(self, data: bytes) -> bytes:
        with gzip.GzipFile(fileobj=io.BytesIO(data), mode="rb") as f:
            return f.read()


@dataclass
class XzCompressor(Compressor):
    """XZ (LZMA) compressor for high compression ratio archives."""

    name: str = "xz"
    preset: int = 6  # default compression level

    def compress(self, data: bytes) -> bytes:
        return lzma.compress(data, preset=self.preset)

    def decompress(self, data: bytes) -> bytes:
        return lzma.decompress(data)


# Registry of compressors.  New compressors can be added here by
# implementing the Compressor interface and adding an entry to the
# registry.
COMPRESSOR_REGISTRY: Dict[str, Compressor] = {
    "gzip": GzipCompressor(level=6),
    "xz": XzCompressor(preset=6),
}

if lz4_frame is not None:
    COMPRESSOR_REGISTRY["lz4"] = LZ4Compressor()

if zstd is not None:
    COMPRESSOR_REGISTRY["zstd"] = ZstdCompressor(level=3)
