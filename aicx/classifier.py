"""Heuristic chunk classification for AICX.

The classifier inspects a block of bytes and attempts to determine its
content type.  The result of classification informs which transform and
codec are selected.  This module currently implements very simple
heuristics for detecting JSON, YAML, text and binary data.  It does
not perform a full structural parse – the goal is to keep the
classification cheap so that it can run on every chunk.
"""

from __future__ import annotations

import json
from typing import Literal


# Define a type for our labels.  As the classifier grows more complex,
# additional types can be added (e.g. "log", "source_code", "csv").
ChunkLabel = Literal["json", "yaml", "text", "binary"]


def classify_chunk(data: bytes) -> ChunkLabel:
    """Classify a chunk into a coarse content type.

    The classification follows these rules:

    * If the data can be decoded as UTF‑8 and is valid JSON after
      stripping whitespace, label it ``json``.
    * If the data can be decoded as UTF‑8 and contains colon/space
      patterns typical of YAML, label it ``yaml``.
    * If the data can be decoded as UTF‑8 and contains printable
      characters and newlines, label it ``text``.
    * Otherwise label it ``binary``.

    Args:
        data: The raw bytes to classify.

    Returns:
        One of ``json``, ``yaml``, ``text`` or ``binary``.
    """
    # Binary heuristics: if the chunk has high entropy or contains
    # NUL bytes, it's likely not text.
    if b"\x00" in data:
        return "binary"
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return "binary"
    stripped = text.strip()
    # Try JSON – it must start with { or [ and parse successfully.
    if stripped and stripped[0] in "{[":
        try:
            json.loads(text)
        except Exception:
            pass
        else:
            return "json"
    # Heuristic for YAML: presence of ':' followed by space on many lines.
    if ": " in text and ("\n" in text or "\r" in text):
        # Count colons relative to newline breaks.
        colon_count = text.count(": ")
        line_count = text.count("\n") + 1
        if colon_count / line_count > 0.3:
            return "yaml"
    # If it decodes and has printable characters, treat it as plain text.
    # Note: this is a simple check; in practice you may want to filter
    # control characters more aggressively.
    if stripped:
        return "text"
    return "binary"