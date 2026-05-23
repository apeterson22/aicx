"""Codec selection logic for AICX.

Given a chunk of data, the codec selector tries a subset of available
codecs and picks the one that yields the smallest output.  In a
production system you might also consider compression and decompression
speed, memory usage, availability on the target platform, or user
preferences.  Here we only consider the output size.
"""

from __future__ import annotations

import io
import time
from typing import Dict, Iterable, List, Tuple

from .compressors import COMPRESSOR_REGISTRY, Compressor


class CodecSelector:
    """Select the most appropriate compression codec for a given chunk.

    The selector tests multiple codecs by compressing a sample of the
    chunk.  It returns the name of the codec that produced the smallest
    compressed output.  If none of the candidate codecs is available
    (should not happen), a default codec is used.
    """

    def __init__(self, candidates: Iterable[str] | None = None) -> None:
        if candidates is None:
            # Default ordering – you can reorder this list to favour
            # speed over ratio or vice versa.
            candidates = ["zstd", "lz4", "xz", "gzip"]
        self.candidates: List[str] = list(candidates)

    def select(self, data: bytes) -> str:
        """Return the name of the best codec for ``data``.

        Args:
            data: The data to compress.

        Returns:
            The name of the codec that yields the smallest compressed size.
        """
        best_codec: str | None = None
        best_size: int | None = None
        # Iterate through candidate codecs and test compression.
        for name in self.candidates:
            compressor = COMPRESSOR_REGISTRY.get(name)
            if compressor is None:
                continue
            try:
                compressed = compressor.compress(data)
            except Exception:
                # If compression fails, skip this codec.
                continue
            size = len(compressed)
            if best_size is None or size < best_size:
                best_size = size
                best_codec = name
        return best_codec or self.candidates[0]