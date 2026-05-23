"""Command‑line interface for AICX.

This module exposes a ``main()`` function that can be used as a console
script entry point.  It provides `pack` and `unpack` subcommands to
create and extract archives.
"""

from __future__ import annotations

import argparse
import sys

from .container import pack, unpack
from .codec_selector import CodecSelector


def _parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Adaptive Intelligent Compression eXchange (AICX)"
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    # Pack command
    pack_parser = subparsers.add_parser("pack", help="Pack files into an AICX archive")
    pack_parser.add_argument(
        "paths",
        nargs="+",
        help="Files or directories to include in the archive",
    )
    pack_parser.add_argument(
        "--output",
        "-o",
        required=True,
        help="Path to the output .aicx file",
    )
    pack_parser.add_argument(
        "--chunk-size",
        type=int,
        default=4 * 1024 * 1024,
        help="Maximum size of each chunk in bytes (default: 4 MiB)",
    )
    pack_parser.add_argument(
        "--fast",
        action="store_true",
        help="Prefer speed over ratio (use LZ4)"
    )
    pack_parser.add_argument(
        "--archive",
        action="store_true",
        help="Prefer high compression ratio (use XZ)",
    )
    # Unpack command
    unpack_parser = subparsers.add_parser(
        "unpack", help="Extract files from an AICX archive"
    )
    unpack_parser.add_argument(
        "archive",
        help="Path to the .aicx archive to extract",
    )
    unpack_parser.add_argument(
        "--target",
        "-t",
        default=".",
        help="Directory to extract files into (default: current directory)",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    args = _parse_args(argv or sys.argv[1:])
    if args.command == "pack":
        # Select candidate codecs based on user flags
        if args.fast:
            selector = CodecSelector(candidates=["lz4", "zstd", "gzip"])
        elif args.archive:
            selector = CodecSelector(candidates=["xz", "zstd", "lz4", "gzip"])
        else:
            selector = CodecSelector()  # default ordering
        pack(args.paths, args.output, chunk_size=args.chunk_size, selector=selector)
    elif args.command == "unpack":
        unpack(args.archive, args.target)
    else:
        raise RuntimeError(f"Unknown command: {args.command}")


if __name__ == "__main__":
    main()