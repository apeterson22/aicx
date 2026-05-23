from __future__ import annotations

import importlib
import sys
import types


def test_compatibility_shim_prefers_native_extension(monkeypatch):
    fake_native = types.SimpleNamespace()
    calls = []

    def pack(*args, **kwargs):
        calls.append(("pack", args, kwargs))
        return "native-pack"

    def unpack(*args, **kwargs):
        calls.append(("unpack", args, kwargs))
        return "native-unpack"

    fake_native.pack = pack
    fake_native.unpack = unpack

    monkeypatch.setitem(sys.modules, "aicx_native", fake_native)
    sys.modules.pop("aicx", None)
    sys.modules.pop("aicx.container", None)

    try:
        package = importlib.import_module("aicx")

        assert package.pack("source", "archive") == "native-pack"
        assert package.unpack("archive", "restored") == "native-unpack"
        assert calls == [
            ("pack", ("source", "archive"), {}),
            ("unpack", ("archive", "restored"), {}),
        ]
    finally:
        sys.modules.pop("aicx", None)
        sys.modules.pop("aicx.container", None)
