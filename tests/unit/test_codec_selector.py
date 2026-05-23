from aicx import codec_selector
from aicx.codec_selector import CodecSelector


class FakeCompressor:
    def __init__(self, compressed: bytes) -> None:
        self._compressed = compressed

    def compress(self, data: bytes) -> bytes:
        return self._compressed


def test_codec_selector_picks_smallest_compressed_result(monkeypatch):
    monkeypatch.setattr(
        codec_selector,
        "COMPRESSOR_REGISTRY",
        {
            "lz4": FakeCompressor(b"aaaa"),
            "zstd": FakeCompressor(b"aa"),
            "gzip": FakeCompressor(b"aaa"),
        },
    )

    selector = CodecSelector(candidates=["lz4", "zstd", "gzip"])

    assert selector.select(b"sample") == "zstd"


def test_codec_selector_skips_missing_candidates(monkeypatch):
    monkeypatch.setattr(
        codec_selector,
        "COMPRESSOR_REGISTRY",
        {
            "gzip": FakeCompressor(b"aaaa"),
        },
    )

    selector = CodecSelector(candidates=["missing", "gzip"])

    assert selector.select(b"sample") == "gzip"
