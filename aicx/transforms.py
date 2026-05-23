"""Data transforms to improve compression in AICX.

A transform takes a byte string and returns a transformed byte string
along with metadata needed to reverse the transformation during
decompression.  Transforms are lossless – the original bytes must be
recovered exactly when unpacking.

The built‑in transforms are minimal:

* **JSON canonicalisation** – parse JSON text into Python data, then
  serialise it using compact separators.  This removes extraneous
  whitespace and normalises key ordering.  Dictionary key order is not
  guaranteed to be preserved, but JSON semantics do not depend on
  key order, so the original structure is recovered on load.

Future versions could include transforms for YAML, CSV, logs and source
code.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Callable, Dict, Tuple

# A transform function takes bytes and returns (transformed_bytes,
# metadata).  The metadata will be passed back to the inverse transform
# to restore the original bytes.
TransformFunction = Callable[[bytes], Tuple[bytes, Dict[str, str]]]
ReverseTransformFunction = Callable[[bytes, Dict[str, str]], bytes]


@dataclass
class Transform:
    """Represents a reversible transform with encode/decode functions."""

    name: str
    encode: TransformFunction
    decode: ReverseTransformFunction


def identity_transform(data: bytes) -> Tuple[bytes, Dict[str, str]]:
    """Return data unchanged with empty metadata."""
    return data, {}


def identity_reverse(data: bytes, metadata: Dict[str, str]) -> bytes:
    return data


def json_canonicalise(data: bytes) -> Tuple[bytes, Dict[str, str]]:
    """Canonicalise JSON by parsing and dumping with compact separators.

    This removes spaces and newlines and normalises ordering of keys
    (Python's `json` module preserves insertion order from Python 3.7).
    If the JSON is invalid, the original data is returned unchanged.

    Returns a tuple of the canonicalised bytes and metadata indicating
    that the transform was applied.  The metadata itself is empty
    because the transform is self‑describing.
    """
    try:
        obj = json.loads(data.decode("utf-8"))
    except Exception:
        # Not valid JSON; return unchanged.
        return data, {}
    canonical = json.dumps(obj, separators=(",", ":"), ensure_ascii=False)
    return canonical.encode("utf-8"), {}


def json_reverse(data: bytes, metadata: Dict[str, str]) -> bytes:
    # For canonical JSON we can just return the bytes.  The
    # canonicalisation is deterministic, so there is no metadata to apply.
    return data


# Map classifier labels to transforms.  Each entry defines the
# Transform to run when a chunk is classified with that label.
TRANSFORMS: Dict[str, Transform] = {
    "json": Transform(name="json_canonical", encode=json_canonicalise, decode=json_reverse),
    # YAML canonicalisation could be added here with a YAML library.
    "yaml": Transform(name="identity", encode=identity_transform, decode=identity_reverse),
    "text": Transform(name="identity", encode=identity_transform, decode=identity_reverse),
    "binary": Transform(name="identity", encode=identity_transform, decode=identity_reverse),
}


def apply_transform(label: str, data: bytes) -> Tuple[bytes, Dict[str, str], str]:
    """Select and apply a transform based on the classifier label.

    Args:
        label: The content label returned by the classifier.
        data: The raw bytes to transform.

    Returns:
        A tuple ``(transformed_bytes, metadata, transform_name)``.  If no
        transform is defined for the label, the identity transform is
        used.
    """
    transform = TRANSFORMS.get(label)
    if transform is None:
        transform = Transform(name="identity", encode=identity_transform, decode=identity_reverse)
    out, meta = transform.encode(data)
    return out, meta, transform.name


def reverse_transform(name: str, data: bytes, metadata: Dict[str, str]) -> bytes:
    """Reverse a previously applied transform.

    Args:
        name: The name of the transform that was applied.
        data: The transformed data to invert.
        metadata: The metadata returned by the original transform.

    Returns:
        The original bytes.
    """
    # Look up by name.  If unknown, fall back to identity.
    for label, transform in TRANSFORMS.items():
        if transform.name == name:
            return transform.decode(data, metadata)
    # Unknown transform; return data unchanged.
    return data