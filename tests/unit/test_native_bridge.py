from __future__ import annotations

import importlib
import sys
import types


def test_pack_prefers_native_extension(monkeypatch):
    fake_native = types.SimpleNamespace()

    calls = []

    def pack(inputs, output_path, profile, chunk_size, hash_algorithm):
        calls.append((inputs, output_path, profile, chunk_size, hash_algorithm))
        return "native-pack"

    fake_native.pack = pack
    fake_native.unpack = lambda *args, **kwargs: "native-unpack"

    monkeypatch.setitem(sys.modules, "aicx_native", fake_native)
    sys.modules.pop("aicx.container", None)

    try:
        container = importlib.import_module("aicx.container")

        result = container.pack(["alpha.txt"], "archive.aicx", chunk_size=1024)

        assert result == "native-pack"
        assert calls == [(["alpha.txt"], "archive.aicx", "balanced", 1024, "blake3")]
    finally:
        sys.modules.pop("aicx.container", None)


def test_unpack_prefers_native_extension(monkeypatch):
    fake_native = types.SimpleNamespace()

    calls = []

    def unpack(archive_path, target_dir, paths, exact, overwrite):
        calls.append((archive_path, target_dir, paths, exact, overwrite))
        return "native-unpack"

    fake_native.pack = lambda *args, **kwargs: "native-pack"
    fake_native.unpack = unpack

    monkeypatch.setitem(sys.modules, "aicx_native", fake_native)
    sys.modules.pop("aicx.container", None)

    try:
        container = importlib.import_module("aicx.container")

        result = container.unpack("archive.aicx", "restored")

        assert result == "native-unpack"
        assert calls == [("archive.aicx", "restored", None, False, False)]
    finally:
        sys.modules.pop("aicx.container", None)
