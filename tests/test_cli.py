from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from objectfile import _cli

if TYPE_CHECKING:
    from pathlib import Path


def test_cli_prints_summary(
    sample_object: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    rc = _cli.main([str(sample_object)])
    out = capsys.readouterr().out
    assert rc == 0
    assert "format:" in out
    assert "architecture:" in out
    assert "libraries (" in out
    assert "imports:" in out


def test_cli_lists_symbols(
    sample_object: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    rc = _cli.main([str(sample_object), "--symbols"])
    assert rc == 0
    assert "symbols:" in capsys.readouterr().out


def test_cli_demangle(sample_object: Path, capsys: pytest.CaptureFixture[str]) -> None:
    # pycxxfilt is in the test dependency group, so --demangle is available.
    rc = _cli.main([str(sample_object), "--symbols", "--demangle"])
    assert rc == 0
    assert "symbols:" in capsys.readouterr().out


def test_cli_demangle_without_pycxxfilt(
    sample_object: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(_cli, "_load_demangler", lambda: None)
    rc = _cli.main([str(sample_object), "--demangle"])
    assert rc == 1
    assert "pycxxfilt" in capsys.readouterr().err


def test_cli_demangle_with_version_keeps_version(
    versioned_elf: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    rc = _cli.main([str(versioned_elf), "--exports", "--demangle-with-version"])
    assert rc == 0
    assert "@@" in capsys.readouterr().out


def test_cli_demangle_drops_version(
    versioned_elf: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    rc = _cli.main([str(versioned_elf), "--exports", "--demangle"])
    assert rc == 0
    assert "@@" not in capsys.readouterr().out


def test_cli_demangle_flags_mutually_exclusive(
    sample_object: Path,
) -> None:
    with pytest.raises(SystemExit):
        _cli.main([str(sample_object), "--demangle", "--demangle-with-version"])


def test_display_falls_back_when_demangle_raises() -> None:
    def boom(_name: str) -> str:
        raise ValueError("invalid Itanium mangled name")

    # Symbols the demangler cannot parse must fall back to the raw name.
    assert _cli._display("_ZGTtNKSt11logic_error4whatEv", boom) == (
        "_ZGTtNKSt11logic_error4whatEv"
    )


def test_cli_missing_file(capsys: pytest.CaptureFixture[str]) -> None:
    rc = _cli.main(["/no/such/object/file"])
    assert rc == 1
    assert "objectfile:" in capsys.readouterr().err


def test_cli_bad_object(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    junk = tmp_path / "junk.bin"
    junk.write_bytes(b"not an object file at all")
    rc = _cli.main([str(junk)])
    assert rc == 1
    assert "failed to parse" in capsys.readouterr().err
