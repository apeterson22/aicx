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

    def extract(*args, **kwargs):
        calls.append(("extract", args, kwargs))
        return "native-extract"

    def digest(*args, **kwargs):
        calls.append(("digest", args, kwargs))
        return "native-digest"

    def digest_toon(*args, **kwargs):
        calls.append(("digest_toon", args, kwargs))
        return "native-digest-toon"

    def inspect(*args, **kwargs):
        calls.append(("inspect", args, kwargs))
        return "native-inspect"

    def inspect_toon(*args, **kwargs):
        calls.append(("inspect_toon", args, kwargs))
        return "native-inspect-toon"

    def manifest_digest(*args, **kwargs):
        calls.append(("manifest_digest", args, kwargs))
        return "native-manifest-digest"

    def report(*args, **kwargs):
        calls.append(("report", args, kwargs))
        return "native-report"

    def report_toon(*args, **kwargs):
        calls.append(("report_toon", args, kwargs))
        return "native-report-toon"

    def sidecar(*args, **kwargs):
        calls.append(("sidecar", args, kwargs))
        return "native-sidecar"

    def sidecar_toon(*args, **kwargs):
        calls.append(("sidecar_toon", args, kwargs))
        return "native-sidecar-toon"

    def sidecar_digest(*args, **kwargs):
        calls.append(("sidecar_digest", args, kwargs))
        return "native-sidecar-digest"

    def list_paths(*args, **kwargs):
        calls.append(("list_paths", args, kwargs))
        return ["native-path"]

    def verify(*args, **kwargs):
        calls.append(("verify", args, kwargs))
        return "native-verify"

    def compare_profiles(*args, **kwargs):
        calls.append(("compare_profiles", args, kwargs))
        return "native-compare"

    fake_native.pack = pack
    fake_native.unpack = unpack
    fake_native.extract = extract
    fake_native.inspect = inspect
    fake_native.inspect_toon = inspect_toon
    fake_native.digest = digest
    fake_native.digest_toon = digest_toon
    fake_native.manifest_digest = manifest_digest
    fake_native.report = report
    fake_native.report_toon = report_toon
    fake_native.sidecar = sidecar
    fake_native.sidecar_toon = sidecar_toon
    fake_native.sidecar_digest = sidecar_digest
    fake_native.list_paths = list_paths
    fake_native.verify = verify
    fake_native.compare_profiles = compare_profiles

    monkeypatch.setitem(sys.modules, "aicx_native", fake_native)
    sys.modules.pop("aicx", None)
    sys.modules.pop("aicx.container", None)

    try:
        package = importlib.import_module("aicx")

        assert "extract" in package.__all__
        assert "digest" in package.__all__
        assert "digest_toon" in package.__all__
        assert "manifest_digest" in package.__all__
        assert "sidecar_digest" in package.__all__
        assert package.pack("source", "archive") == "native-pack"
        assert package.unpack("archive", "restored") == "native-unpack"
        assert package.extract("archive", "path", "restored") == "native-extract"
        assert package.inspect("archive") == "native-inspect"
        assert package.inspect_toon("archive") == "native-inspect-toon"
        assert package.digest("archive") == "native-digest"
        assert package.digest_toon("archive") == "native-digest-toon"
        assert package.manifest_digest("archive") == "native-manifest-digest"
        assert package.report("archive") == "native-report"
        assert package.report_toon("archive") == "native-report-toon"
        assert package.sidecar("archive") == "native-sidecar"
        assert package.sidecar_toon("archive") == "native-sidecar-toon"
        assert package.sidecar_digest("archive") == "native-sidecar-digest"
        assert package.list_paths("archive") == ["native-path"]
        assert package.verify("archive") == "native-verify"
        assert package.compare_profiles("source") == "native-compare"
        assert calls == [
            ("pack", ("source", "archive"), {}),
            ("unpack", ("archive", "restored"), {}),
            ("extract", ("archive", "path", "restored"), {}),
            ("inspect", ("archive",), {}),
            ("inspect_toon", ("archive",), {}),
            ("digest", ("archive",), {}),
            ("digest_toon", ("archive",), {}),
            ("manifest_digest", ("archive",), {}),
            ("report", ("archive",), {}),
            ("report_toon", ("archive",), {}),
            ("sidecar", ("archive",), {}),
            ("sidecar_toon", ("archive",), {}),
            ("sidecar_digest", ("archive",), {}),
            ("list_paths", ("archive",), {}),
            ("verify", ("archive",), {}),
            ("compare_profiles", ("source",), {}),
        ]
    finally:
        sys.modules.pop("aicx", None)
        sys.modules.pop("aicx.container", None)
