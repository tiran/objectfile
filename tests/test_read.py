from __future__ import annotations

import collections.abc
from typing import TYPE_CHECKING

import pytest

import objectfile
from objectfile import (
    Architecture,
    Endianness,
    Export,
    Format,
    Import,
    ObjectFile,
    ObjectKind,
)

if TYPE_CHECKING:
    from pathlib import Path


def test_parse_sample_object(sample_object: Path) -> None:
    obj = objectfile.parse_file(sample_object)

    assert isinstance(obj, ObjectFile)
    assert isinstance(obj.format, Format)
    assert obj.format != Format.Unknown
    assert isinstance(obj.architecture, Architecture)
    assert isinstance(obj.endianness, Endianness)
    assert isinstance(obj.kind, ObjectKind)
    assert isinstance(obj.is_64, bool)


def test_parse_bytes_matches_parse_file(sample_object: Path) -> None:
    from_bytes = objectfile.parse(sample_object.read_bytes())
    from_file = objectfile.parse_file(sample_object)

    assert from_bytes.format == from_file.format
    assert from_bytes.architecture == from_file.architecture
    assert from_bytes.is_64 == from_file.is_64
    assert list(from_bytes.libraries()) == list(from_file.libraries())


def test_collections_are_iterators(sample_object: Path) -> None:
    obj = objectfile.parse_file(sample_object)

    imports = obj.imports()
    assert isinstance(imports, collections.abc.Iterator)
    assert iter(imports) is imports
    assert all(isinstance(imp, Import) for imp in obj.imports())
    assert all(isinstance(exp, Export) for exp in obj.exports())
    assert all(isinstance(lib, str) for lib in obj.libraries())


def test_object_file_is_immutable(sample_object: Path) -> None:
    obj = objectfile.parse_file(sample_object)

    # Frozen pyclass: attributes have no setter.
    with pytest.raises(AttributeError):
        obj.format = Format.Elf  # type: ignore[misc]


def test_parse_minimal_wasm() -> None:
    # Magic + version, then a custom section; no toolchain needed. Confirms the
    # opt-in "wasm" object feature is enabled.
    data = b"\x00asm\x01\x00\x00\x00" + b"\x00" + b"\x08" + b"\x04test" + b"abc"
    obj = objectfile.parse(data)
    assert obj.format == Format.Wasm
    assert obj.architecture == Architecture.Wasm32
    assert obj.is_64 is False


def test_buffer_protocol_inputs(sample_object: Path) -> None:
    data = sample_object.read_bytes()
    expected = objectfile.parse(data).format

    # parse() accepts anything supporting the buffer protocol.
    assert objectfile.parse(bytearray(data)).format == expected
    assert objectfile.parse(memoryview(data)).format == expected
