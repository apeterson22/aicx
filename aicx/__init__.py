"""AICX package root.

This package exposes high‑level functions and classes for working with
AICX archives programmatically.  Users interested in the command line
should invoke the ``aicx`` console script.
"""

from .container import pack, unpack
from .codec_selector import CodecSelector
from .compressors import COMPRESSOR_REGISTRY

__all__ = ["pack", "unpack", "CodecSelector", "COMPRESSOR_REGISTRY"]