from aicx.cli import main


def test_cli_pack_unpack_round_trip(tmp_path, monkeypatch):
    source_dir = tmp_path / "input"
    restored_dir = tmp_path / "restored"
    source_dir.mkdir()
    (source_dir / "notes.txt").write_text("hello world\n", encoding="utf-8")
    nested_dir = source_dir / "nested"
    nested_dir.mkdir()
    (nested_dir / "data.bin").write_bytes(b"\x00\x01\x02\x03\x04")

    monkeypatch.chdir(tmp_path)

    main([
        "pack",
        "--output",
        "sample.aicx",
        "--chunk-size",
        "4",
        "input",
    ])
    main([
        "unpack",
        "sample.aicx",
        "--target",
        str(restored_dir),
    ])

    assert (restored_dir / "input" / "notes.txt").read_text(encoding="utf-8") == "hello world\n"
    assert (restored_dir / "input" / "nested" / "data.bin").read_bytes() == b"\x00\x01\x02\x03\x04"
