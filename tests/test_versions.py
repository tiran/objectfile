from __future__ import annotations

from typing import TYPE_CHECKING

import objectfile
from objectfile import Export, Symbol

if TYPE_CHECKING:
    from pathlib import Path


def test_version_fields_default() -> None:
    for obj in (Export("foo"), Symbol("foo")):
        assert obj.version is None
        assert obj.version_hidden is False


def test_exports_with_versions_are_distinct() -> None:
    a = Export("foo", version="V1")
    b = Export("foo", version="V2")
    assert a != b
    assert sorted((b, a), key=lambda e: e.version or "")[0].version == "V1"


def test_versioned_elf(versioned_elf: Path) -> None:
    obj = objectfile.parse_file(versioned_elf)
    exports = list(obj.exports())
    dynamic = list(obj.dynamic_symbols())
    symbols = list(obj.symbols())

    assert exports and dynamic
    # Only the dynamic symbol table carries GNU versions; .symtab does not.
    assert any(sym.version for sym in dynamic)
    assert all(sym.version is None for sym in symbols)
    assert any(exp.version for exp in exports)
    # Default-version exports (nm's "@@") have version_hidden == False.
    assert any(exp.version and not exp.version_hidden for exp in exports)
    # Version-node marker symbols (e.g. GLIBCXX_3.4) are not exports themselves.
    versions = {exp.version for exp in exports if exp.version}
    assert not [exp for exp in exports if exp.name in versions]
