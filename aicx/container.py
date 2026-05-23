"""Container format for AICX archives.

This module defines functions to pack multiple files into a single
AICX archive and to unpack archives back into their original files.

The archive format is simple:

* The first 8 bytes are a big‑endian unsigned integer containing the
  length of the JSON header.
* The header is UTF‑8 encoded JSON.  It contains a manifest (file
  names and sizes) and a list of chunk descriptors.  Each descriptor
  records the codec used, the offsets and sizes in the compressed
  block section, the original size, the classifier label, the transform
  applied and any per‑chunk metadata returned by the transform.
* After the header come the concatenated compressed blocks.  Each
  block can be decompressed independently using the codec indicated
  in the header.

No encryption or checksums are applied.  Future versions could
incorporate BLAKE3 hashes and authenticated encryption.
"""

from __future__ import annotations

import json
import os
import struct
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Tuple

from .chunking import fixed_size_chunks
from .classifier import classify_chunk
from .compressors import COMPRESSOR_REGISTRY
from .codec_selector import CodecSelector
from .transforms import apply_transform, reverse_transform

try:  # pragma: no cover - exercised only when the native extension is installed
    import aicx_native as _native
except ImportError:  # pragma: no cover - legacy fallback path
    _native = None


@dataclass
class ChunkEntry:
    codec: str
    offset: int  # Offset of the compressed data in the data section
    compressed_size: int
    original_size: int
    transform: str
    label: str
    meta: Dict[str, str] = field(default_factory=dict)


@dataclass
class FileEntry:
    path: str
    size: int
    chunk_indices: List[int]


def _require_int(value: object, field_name: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise ValueError(f"Archive field '{field_name}' must be an integer")
    return value


def _require_non_negative_int(value: object, field_name: str) -> int:
    integer_value = _require_int(value, field_name)
    if integer_value < 0:
        raise ValueError(f"Archive field '{field_name}' must be non-negative")
    return integer_value


def _require_positive_int(value: object, field_name: str) -> int:
    integer_value = _require_int(value, field_name)
    if integer_value <= 0:
        raise ValueError(f"Archive field '{field_name}' must be positive")
    return integer_value


def _resolve_output_path(target_root: Path, raw_path: str) -> Path:
    candidate = (target_root / Path(raw_path)).resolve(strict=False)
    if candidate == target_root:
        raise ValueError(f"Archive entry path {raw_path!r} is not a file path")
    try:
        candidate.relative_to(target_root)
    except ValueError as exc:
        raise ValueError(f"Archive entry path {raw_path!r} escapes the target directory") from exc
    return candidate


def _profile_from_selector(selector: Optional[CodecSelector]) -> str:
    if selector is None:
        return "balanced"
    candidates = [candidate.lower() for candidate in getattr(selector, "candidates", [])]
    if not candidates:
        return "balanced"
    if candidates[0] == "lz4":
        return "fast"
    if candidates[0] == "xz":
        return "max"
    return "balanced"


def _gather_files(paths: Iterable[str]) -> List[str]:
    files: List[str] = []
    for p in paths:
        p = os.path.expanduser(p)
        if os.path.isdir(p):
            # Walk directory
            for root, _, filenames in os.walk(p):
                for f in filenames:
                    files.append(os.path.join(root, f))
        else:
            files.append(p)
    return files


def pack(paths: Iterable[str], output_path: str, chunk_size: int = 4 * 1024 * 1024, selector: Optional[CodecSelector] = None) -> None:
    """Pack multiple files into a single AICX archive.

    Args:
        paths: An iterable of file or directory paths to pack.  Directories
            are scanned recursively.
        output_path: The filename of the archive to create.
        chunk_size: The maximum size of each chunk in bytes.  Larger
            files will be split into multiple chunks.
        selector: Optional ``CodecSelector`` instance.  If not provided,
            the default selector will be used.
    """
    if _native is not None:
        profile = _profile_from_selector(selector)
        return _native.pack(list(paths), output_path, profile, chunk_size, "blake3")

    selector = selector or CodecSelector()
    files = _gather_files(paths)
    chunks: List[ChunkEntry] = []
    file_entries: List[FileEntry] = []
    # Data will be written sequentially after the header.  Keep track
    # of the current offset.
    data_offset = 0
    compressed_blocks: List[bytes] = []

    for file_path in files:
        rel_path = os.path.relpath(file_path, start=os.getcwd())
        with open(file_path, "rb") as f:
            data = f.read()
        file_chunk_indices: List[int] = []
        for chunk in fixed_size_chunks(data, chunk_size):
            label = classify_chunk(chunk)
            transformed, meta, transform_name = apply_transform(label, chunk)
            codec_name = selector.select(transformed)
            compressor = COMPRESSOR_REGISTRY[codec_name]
            comp_data = compressor.compress(transformed)
            entry = ChunkEntry(
                codec=codec_name,
                offset=data_offset,
                compressed_size=len(comp_data),
                original_size=len(chunk),
                transform=transform_name,
                label=label,
                meta=meta,
            )
            chunks.append(entry)
            file_chunk_indices.append(len(chunks) - 1)
            compressed_blocks.append(comp_data)
            data_offset += len(comp_data)
        file_entries.append(FileEntry(path=rel_path, size=len(data), chunk_indices=file_chunk_indices))

    # Build header structure
    header = {
        "version": 1,
        "chunk_size": chunk_size,
        "files": [
            {
                "path": fe.path,
                "size": fe.size,
                "chunks": fe.chunk_indices,
            }
            for fe in file_entries
        ],
        "chunks": [
            {
                "codec": ce.codec,
                "offset": ce.offset,
                "compressed_size": ce.compressed_size,
                "original_size": ce.original_size,
                "transform": ce.transform,
                "label": ce.label,
                "meta": ce.meta,
            }
            for ce in chunks
        ],
    }
    header_bytes = json.dumps(header, separators=(",", ":")).encode("utf-8")
    header_length = len(header_bytes)
    with open(output_path, "wb") as out:
        out.write(struct.pack(">Q", header_length))
        out.write(header_bytes)
        for block in compressed_blocks:
            out.write(block)


def unpack(archive_path: str, target_dir: str) -> None:
    """Unpack an AICX archive into a directory.

    Args:
        archive_path: Path to the `.aicx` file to unpack.
        target_dir: Directory to extract files into.  Will be created
            if it does not exist.
    """
    if _native is not None:
        return _native.unpack(archive_path, target_dir, None, False, False)

    target = Path(target_dir)
    target.mkdir(parents=True, exist_ok=True)
    target_root = target.resolve()
    with open(archive_path, "rb") as f:
        header_len_bytes = f.read(8)
        if len(header_len_bytes) < 8:
            raise ValueError("Archive is truncated or missing header length")
        (header_length,) = struct.unpack(">Q", header_len_bytes)
        f.seek(0, os.SEEK_END)
        archive_size = f.tell()
        if header_length > archive_size - 8:
            raise ValueError("Archive header length exceeds archive size")
        f.seek(8)
        header_bytes = f.read(header_length)
        if len(header_bytes) != header_length:
            raise ValueError("Archive header is truncated")
        try:
            header = json.loads(header_bytes.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise ValueError("Archive header is malformed") from exc
        if not isinstance(header, dict):
            raise ValueError("Archive header must be a JSON object")
        # Load compressed data into memory for simplicity.  For large archives
        # you would stream blocks directly from the file.
        compressed_data = f.read()

    if "chunks" not in header:
        raise ValueError("Archive header is missing the chunks table")
    if "files" not in header:
        raise ValueError("Archive header is missing the files table")

    version = _require_non_negative_int(header.get("version"), "version")
    if version != 1:
        raise ValueError(f"Unsupported archive version: {version}")
    _require_positive_int(header.get("chunk_size"), "chunk_size")

    chunks_raw = header["chunks"]
    files_raw = header["files"]
    if not isinstance(chunks_raw, list):
        raise ValueError("Archive header field 'chunks' must be a list")
    if not isinstance(files_raw, list):
        raise ValueError("Archive header field 'files' must be a list")

    chunks_info: List[Dict[str, object]] = []
    for idx, chunk_entry in enumerate(chunks_raw):
        if not isinstance(chunk_entry, dict):
            raise ValueError(f"Archive chunk {idx} must be a JSON object")
        codec = chunk_entry.get("codec")
        offset = _require_non_negative_int(chunk_entry.get("offset"), f"chunks[{idx}].offset")
        compressed_size = _require_non_negative_int(
            chunk_entry.get("compressed_size"), f"chunks[{idx}].compressed_size"
        )
        original_size = _require_non_negative_int(
            chunk_entry.get("original_size"), f"chunks[{idx}].original_size"
        )
        transform = chunk_entry.get("transform")
        label = chunk_entry.get("label")
        meta = chunk_entry.get("meta", {})
        if not isinstance(codec, str) or not codec:
            raise ValueError(f"Archive chunk {idx} must name a codec")
        if codec not in COMPRESSOR_REGISTRY:
            raise ValueError(f"Archive chunk {idx} uses unknown codec {codec!r}")
        if not isinstance(transform, str) or not transform:
            raise ValueError(f"Archive chunk {idx} must name a transform")
        if not isinstance(label, str) or not label:
            raise ValueError(f"Archive chunk {idx} must name a label")
        if not isinstance(meta, dict):
            raise ValueError(f"Archive chunk {idx} metadata must be a JSON object")
        if offset + compressed_size > len(compressed_data):
            raise ValueError(f"Archive chunk {idx} exceeds the available data")
        chunks_info.append(
            {
                "codec": codec,
                "offset": offset,
                "compressed_size": compressed_size,
                "original_size": original_size,
                "transform": transform,
                "label": label,
                "meta": meta,
            }
        )

    seen_output_paths: set[Path] = set()
    validated_files: List[Tuple[Path, List[int]]] = []
    for file_idx, file_entry in enumerate(files_raw):
        if not isinstance(file_entry, dict):
            raise ValueError(f"Archive file {file_idx} must be a JSON object")
        raw_path = file_entry.get("path")
        if not isinstance(raw_path, str) or not raw_path:
            raise ValueError(f"Archive file {file_idx} must have a non-empty path")
        out_path = _resolve_output_path(target_root, raw_path)
        if out_path in seen_output_paths:
            raise ValueError(f"Archive contains duplicate output path {raw_path!r}")
        chunk_indices_raw = file_entry.get("chunks")
        if not isinstance(chunk_indices_raw, list):
            raise ValueError(f"Archive file {file_idx} must list chunk indices")
        size = _require_non_negative_int(file_entry.get("size"), f"files[{file_idx}].size")
        chunk_indices: List[int] = []
        reconstructed_size = 0
        for position, chunk_index_value in enumerate(chunk_indices_raw):
            chunk_index = _require_non_negative_int(
                chunk_index_value, f"files[{file_idx}].chunks[{position}]"
            )
            if chunk_index >= len(chunks_info):
                raise ValueError(
                    f"Archive file {file_idx} references missing chunk {chunk_index}"
                )
            chunk_indices.append(chunk_index)
            reconstructed_size += int(chunks_info[chunk_index]["original_size"])
        if reconstructed_size != size:
            raise ValueError(
                f"Archive file {file_idx} has size {size}, but chunk data reconstructs to {reconstructed_size}"
            )
        seen_output_paths.add(out_path)
        validated_files.append((out_path, chunk_indices))

    for out_path, chunk_indices in validated_files:
        out_path.parent.mkdir(parents=True, exist_ok=True)
        # Reconstruct file content by concatenating decompressed chunks.
        parts: List[bytes] = []
        for idx in chunk_indices:
            ce = chunks_info[idx]
            codec = ce["codec"]
            offset = ce["offset"]
            compressed_size = ce["compressed_size"]
            original_size = ce["original_size"]
            transform_name = ce.get("transform", "identity")
            label = ce.get("label", "binary")
            meta = ce.get("meta", {})
            comp_data = compressed_data[offset : offset + compressed_size]
            decompressor = COMPRESSOR_REGISTRY[codec]
            transformed = decompressor.decompress(comp_data)
            chunk = reverse_transform(transform_name, transformed, meta)
            if len(chunk) != original_size:
                # This is a sanity check; mis‑sizes indicate corruption.
                raise ValueError(
                    f"Chunk {idx} decompressed to {len(chunk)} bytes, expected {original_size}"
                )
            parts.append(chunk)
        with open(out_path, "wb") as out_file:
            out_file.write(b"".join(parts))
