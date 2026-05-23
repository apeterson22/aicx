"""Chunking strategies for AICX.

This module defines functions that split raw bytes into chunks.  The
prototype implementation uses simple fixed‑size chunking for speed and
predictability.  In the future, more sophisticated schemes such as
content‑defined chunking (Rabin fingerprints) or semantic chunking can
be added here.
"""

from typing import Iterator


def fixed_size_chunks(data: bytes, chunk_size: int) -> Iterator[bytes]:
    """Yield consecutive slices of ``data`` with length ``chunk_size``.

    The final chunk may be shorter if the length of ``data`` is not a
    multiple of ``chunk_size``.

    Args:
        data: The input byte string to split.
        chunk_size: The desired length of each chunk in bytes.

    Yields:
        Byte slices of at most ``chunk_size`` length.
    """
    if chunk_size <= 0:
        raise ValueError("chunk_size must be positive")
    for i in range(0, len(data), chunk_size):
        yield data[i : i + chunk_size]