from __future__ import annotations

from typing import TYPE_CHECKING

import objectfile
from objectfile import Export, Symbol

if TYPE_CHECKING:
    from pathlib import Path


def test_export_version_fields_default() -> None:
    exp = Export("foo")
    assert exp.version is None
    assert exp.version_hidden is False


def test_symbol_version_fields_default() -> None:
    sym = Symbol("foo")
    assert sym.version is None
    assert sym.version_hidden is False


def test_elf_dynamic_symbols_versioned(versioned_elf: Path) -> None:
    dynamic = list(objectfile.parse_file(versioned_elf).dynamic_symbols())
    assert dynamic
    assert any(sym.version for sym in dynamic)


def test_elf_full_symbol_table_unversioned(versioned_elf: Path) -> None:
    # Only the dynamic symbol table carries GNU versions; .symtab does not.
    symbols = list(objectfile.parse_file(versioned_elf).symbols())
    assert all(sym.version is None for sym in symbols)


def test_exports_with_versions_are_distinct() -> None:
    a = Export("foo", version="V1")
    b = Export("foo", version="V2")
    assert a != b
    assert sorted((b, a), key=lambda e: e.version or "")[0].version == "V1"


def test_elf_versions_populated(versioned_elf: Path) -> None:
    exports = list(objectfile.parse_file(versioned_elf).exports())
    assert exports
    # A versioned ELF library exposes GNU symbol versions.
    assert any(exp.version for exp in exports)


def test_elf_version_markers_filtered(versioned_elf: Path) -> None:
    exports = list(objectfile.parse_file(versioned_elf).exports())
    versions = {exp.version for exp in exports if exp.version}
    # The version-node marker symbols (named after a version, e.g. GLIBCXX_3.4)
    # must not appear as exports themselves.
    assert not [exp for exp in exports if exp.name in versions]


def test_elf_has_default_versions(versioned_elf: Path) -> None:
    exports = list(objectfile.parse_file(versioned_elf).exports())
    # Default-version exports (nm's "@@") have version_hidden == False.
    assert any(exp.version and not exp.version_hidden for exp in exports)
