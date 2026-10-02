from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

import objectfile

if TYPE_CHECKING:
    from pathlib import Path


def test_parse_rejects_garbage() -> None:
    # ObjectFileError subclasses ValueError, so either can be caught.
    with pytest.raises(objectfile.ObjectFileError, match="failed to parse object"):
        objectfile.parse(b"this is definitely not an object file")


def test_object_file_error_is_value_error() -> None:
    assert issubclass(objectfile.ObjectFileError, ValueError)
    with pytest.raises(ValueError):
        objectfile.parse(b"")


def test_parse_file_missing_path() -> None:
    with pytest.raises(FileNotFoundError):
        objectfile.parse_file("/no/such/object/file")


def test_parse_file_empty(tmp_path: Path) -> None:
    empty = tmp_path / "empty.bin"
    empty.write_bytes(b"")
    with pytest.raises(objectfile.ObjectFileError):
        objectfile.parse_file(empty)
