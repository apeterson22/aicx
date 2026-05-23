import json
import struct

import pytest

from aicx.container import pack, unpack


def _make_source_tree(tmp_path):
    source_dir = tmp_path / "source"
    source_dir.mkdir()
    (source_dir / "alpha.txt").write_text("alpha\n", encoding="utf-8")
    (source_dir / "beta.txt").write_text("beta\n", encoding="utf-8")
    nested_dir = source_dir / "nested"
    nested_dir.mkdir()
    (nested_dir / "data.bin").write_bytes(b"\x00\x01\x02\x03")
    return source_dir


def _read_archive(archive_path):
    archive_bytes = archive_path.read_bytes()
    header_length = struct.unpack(">Q", archive_bytes[:8])[0]
    header_bytes = archive_bytes[8 : 8 + header_length]
    payload = archive_bytes[8 + header_length :]
    return json.loads(header_bytes.decode("utf-8")), payload


def _write_archive(archive_path, header, payload):
    header_bytes = json.dumps(header, separators=(",", ":")).encode("utf-8")
    archive_path.write_bytes(struct.pack(">Q", len(header_bytes)) + header_bytes + payload)


def test_pack_unpack_round_trip(tmp_path, monkeypatch):
    source_dir = _make_source_tree(tmp_path)
    archive_path = tmp_path / "archive.aicx"
    restored_dir = tmp_path / "restored"

    monkeypatch.chdir(tmp_path)

    pack([str(source_dir)], str(archive_path), chunk_size=2)
    unpack(str(archive_path), str(restored_dir))

    assert (restored_dir / "source" / "alpha.txt").read_text(encoding="utf-8") == "alpha\n"
    assert (restored_dir / "source" / "beta.txt").read_text(encoding="utf-8") == "beta\n"
    assert (restored_dir / "source" / "nested" / "data.bin").read_bytes() == b"\x00\x01\x02\x03"


def test_unpack_rejects_truncated_header(tmp_path):
    archive_path = tmp_path / "truncated.aicx"
    archive_path.write_bytes(b"\x00\x01")

    with pytest.raises(ValueError, match="missing header length"):
        unpack(str(archive_path), str(tmp_path / "restored"))


def test_unpack_rejects_header_length_past_eof(tmp_path, monkeypatch):
    source_dir = _make_source_tree(tmp_path)
    archive_path = tmp_path / "archive.aicx"
    monkeypatch.chdir(tmp_path)
    pack([str(source_dir)], str(archive_path), chunk_size=2)

    archive_bytes = archive_path.read_bytes()
    archive_path.write_bytes(struct.pack(">Q", 10_000) + archive_bytes[8:])

    with pytest.raises(ValueError, match="header length exceeds archive size"):
        unpack(str(archive_path), str(tmp_path / "restored"))


def test_unpack_rejects_malformed_header_json(tmp_path, monkeypatch):
    source_dir = _make_source_tree(tmp_path)
    archive_path = tmp_path / "archive.aicx"
    monkeypatch.chdir(tmp_path)
    pack([str(source_dir)], str(archive_path), chunk_size=2)

    _, payload = _read_archive(archive_path)
    archive_path.write_bytes(struct.pack(">Q", 8) + b"not-json" + payload)

    with pytest.raises(ValueError, match="header is malformed"):
        unpack(str(archive_path), str(tmp_path / "restored"))


@pytest.mark.parametrize("missing_key", ["chunks", "files"])
def test_unpack_rejects_missing_chunk_tables(tmp_path, monkeypatch, missing_key):
    source_dir = _make_source_tree(tmp_path)
    archive_path = tmp_path / "archive.aicx"
    monkeypatch.chdir(tmp_path)
    pack([str(source_dir)], str(archive_path), chunk_size=2)

    header, payload = _read_archive(archive_path)
    header.pop(missing_key)
    _write_archive(archive_path, header, payload)

    with pytest.raises(ValueError, match=fr"missing the {missing_key} table"):
        unpack(str(archive_path), str(tmp_path / "restored"))


@pytest.mark.parametrize("field_name, field_value", [("offset", 10_000), ("compressed_size", 10_000)])
def test_unpack_rejects_chunk_ranges_outside_payload(tmp_path, monkeypatch, field_name, field_value):
    source_dir = _make_source_tree(tmp_path)
    archive_path = tmp_path / "archive.aicx"
    monkeypatch.chdir(tmp_path)
    pack([str(source_dir)], str(archive_path), chunk_size=2)

    header, payload = _read_archive(archive_path)
    header["chunks"][0][field_name] = field_value
    _write_archive(archive_path, header, payload)

    with pytest.raises(ValueError, match="exceeds the available data"):
        unpack(str(archive_path), str(tmp_path / "restored"))


def test_unpack_rejects_duplicate_output_paths(tmp_path, monkeypatch):
    source_dir = _make_source_tree(tmp_path)
    archive_path = tmp_path / "archive.aicx"
    monkeypatch.chdir(tmp_path)
    pack([str(source_dir)], str(archive_path), chunk_size=2)

    header, payload = _read_archive(archive_path)
    header["files"][1]["path"] = header["files"][0]["path"]
    _write_archive(archive_path, header, payload)

    with pytest.raises(ValueError, match="duplicate output path"):
        unpack(str(archive_path), str(tmp_path / "restored"))


@pytest.mark.parametrize("unsafe_path", ["../escape.txt", "/escape.txt"])
def test_unpack_rejects_paths_that_escape_target_dir(tmp_path, monkeypatch, unsafe_path):
    source_dir = _make_source_tree(tmp_path)
    archive_path = tmp_path / "archive.aicx"
    monkeypatch.chdir(tmp_path)
    pack([str(source_dir)], str(archive_path), chunk_size=2)

    header, payload = _read_archive(archive_path)
    header["files"][0]["path"] = unsafe_path
    _write_archive(archive_path, header, payload)

    with pytest.raises(ValueError, match="escapes the target directory"):
        unpack(str(archive_path), str(tmp_path / "restored"))
