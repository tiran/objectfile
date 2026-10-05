"""ELF dynamic-section / dependency metadata (the elfdeps-style view):
soname, interpreter, rpath/runpath, link kind, symbol hash, and the
verdef/verneed version aggregates.

Concrete assertions use a committed ELF fixture under testdata/; a portable
check confirms the ELF-only fields degrade cleanly on other formats.
"""

from __future__ import annotations

from pathlib import Path

import objectfile
from objectfile import Format, LinkKind, SymbolHash

TESTDATA = Path(__file__).parent.parent / "testdata"
FIXTURE = TESTDATA / "rocm" / "hello.so"


def test_shared_library() -> None:
    obj = objectfile.parse_file(FIXTURE)
    assert obj.format == Format.Elf
    assert obj.link_kind == LinkKind.SharedLibrary
    assert obj.symbol_hash == SymbolHash.GNU
    assert obj.interpreter is None  # a shared library has no PT_INTERP
    assert obj.soname is None or isinstance(obj.soname, str)
    assert obj.rpaths() == []
    assert obj.runpaths() == []
    assert obj.provided_versions() == []

    required = obj.required_versions()
    assert isinstance(required, dict)
    assert "libc.so.6" in required  # links the C library with versioned symbols
    for library, versions in required.items():
        assert isinstance(library, str)
        assert versions == sorted(set(versions))  # sorted and deduplicated

    # verneed <-> versym: every required version is used by some dynamic symbol.
    needed = {v for versions in required.values() for v in versions}
    symbol_versions = {s.version for s in obj.dynamic_symbols() if s.version}
    assert needed <= symbol_versions


def test_fields_are_well_typed(sample_object: Path) -> None:
    obj = objectfile.parse_file(sample_object)
    # These never raise and have the right type regardless of format.
    assert isinstance(obj.link_kind, LinkKind)
    assert isinstance(obj.symbol_hash, SymbolHash)
    assert isinstance(obj.required_versions(), dict)
    assert isinstance(obj.rpaths(), list)
    assert obj.soname is None or isinstance(obj.soname, str)
    assert obj.interpreter is None or isinstance(obj.interpreter, str)
    if obj.format != Format.Elf:
        # soname / interpreter / rpaths have Mach-O analogues, but the hash and
        # the verdef/verneed/runpath fields are strictly ELF.
        assert obj.symbol_hash == SymbolHash.Unknown
        assert obj.runpaths() == []
        assert obj.provided_versions() == []
        assert obj.required_versions() == {}
