import pytest

from aicx.chunking import fixed_size_chunks


def test_fixed_size_chunks_split_bytes_into_expected_slices():
    assert list(fixed_size_chunks(b"abcdefgh", 3)) == [b"abc", b"def", b"gh"]


def test_fixed_size_chunks_rejects_non_positive_sizes():
    with pytest.raises(ValueError, match="chunk_size must be positive"):
        list(fixed_size_chunks(b"abc", 0))
